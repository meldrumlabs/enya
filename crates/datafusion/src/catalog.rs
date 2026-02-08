//! Catalog management for DataFusion tables and schemas.

use datafusion::prelude::SessionContext;
use rustc_hash::FxHashMap;

use crate::types::{ColumnInfo, TableInfo, TableSource};

/// Manages catalog state and provides table/schema information.
#[derive(Debug)]
pub struct Catalog {
    /// Cached table information.
    tables: FxHashMap<String, TableInfo>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}

impl Catalog {
    /// Create a new empty catalog.
    pub fn new() -> Self {
        Self {
            tables: FxHashMap::default(),
        }
    }

    /// Refresh catalog from a SessionContext.
    pub fn refresh(&mut self, ctx: &SessionContext) {
        self.tables.clear();

        // Iterate through all catalogs, schemas, and tables
        for catalog_name in ctx.catalog_names() {
            if let Some(catalog) = ctx.catalog(&catalog_name) {
                for schema_name in catalog.schema_names() {
                    if let Some(schema) = catalog.schema(&schema_name) {
                        for table_name in schema.table_names() {
                            if let Some(table) = futures::executor::block_on(async {
                                schema.table(&table_name).await.ok().flatten()
                            }) {
                                let arrow_schema = table.schema();
                                let columns = arrow_schema
                                    .fields()
                                    .iter()
                                    .map(|f| ColumnInfo {
                                        name: f.name().clone(),
                                        data_type: f.data_type().to_string(),
                                        nullable: f.is_nullable(),
                                    })
                                    .collect();

                                let full_name = if schema_name == "public" {
                                    table_name.clone()
                                } else {
                                    format!("{schema_name}.{table_name}")
                                };

                                self.tables.insert(
                                    full_name,
                                    TableInfo {
                                        name: table_name,
                                        schema: schema_name.clone(),
                                        catalog: catalog_name.clone(),
                                        columns,
                                        row_count: None,
                                        source: TableSource::Memory,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Get all registered tables.
    pub fn tables(&self) -> impl Iterator<Item = &TableInfo> {
        self.tables.values()
    }

    /// Get a table by name.
    pub fn get_table(&self, name: &str) -> Option<&TableInfo> {
        self.tables.get(name)
    }

    /// Get all table names.
    pub fn table_names(&self) -> Vec<&str> {
        self.tables.keys().map(|s| s.as_str()).collect()
    }

    /// Search tables by name pattern.
    pub fn search_tables(&self, pattern: &str) -> Vec<&TableInfo> {
        let pattern = pattern.to_lowercase();
        self.tables
            .values()
            .filter(|t| t.name.to_lowercase().contains(&pattern))
            .collect()
    }

    /// Record that a table was registered from a file.
    pub fn record_file_table(
        &mut self,
        name: String,
        path: String,
        format: crate::types::FileFormat,
        columns: Vec<ColumnInfo>,
    ) {
        self.tables.insert(
            name.clone(),
            TableInfo {
                name,
                schema: "public".to_string(),
                catalog: "datafusion".to_string(),
                columns,
                row_count: None,
                source: if path.starts_with("s3://")
                    || path.starts_with("gs://")
                    || path.starts_with("az://")
                {
                    TableSource::ObjectStore { url: path, format }
                } else {
                    TableSource::LocalFile { path, format }
                },
            },
        );
    }

    /// Remove a table from the catalog.
    pub fn remove_table(&mut self, name: &str) {
        self.tables.remove(name);
    }

    /// Clear all tables.
    pub fn clear(&mut self) {
        self.tables.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_new() {
        let catalog = Catalog::new();
        assert_eq!(catalog.table_names().len(), 0);
    }

    #[test]
    fn test_catalog_search() {
        let mut catalog = Catalog::new();
        catalog.record_file_table(
            "events".to_string(),
            "/data/events.parquet".to_string(),
            crate::types::FileFormat::Parquet,
            vec![],
        );
        catalog.record_file_table(
            "users".to_string(),
            "/data/users.parquet".to_string(),
            crate::types::FileFormat::Parquet,
            vec![],
        );

        let results = catalog.search_tables("event");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "events");
    }
}
