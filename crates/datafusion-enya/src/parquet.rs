//! Parquet metadata extraction from DataFusion execution plans.
//!
//! This module provides utilities to inspect execution plans and extract
//! Parquet-specific metadata such as file paths, sizes, and schema information.

use datafusion::datasource::listing::PartitionedFile;
use datafusion::datasource::physical_plan::FileScanConfig;
use datafusion::datasource::physical_plan::ParquetSource;
use datafusion::datasource::source::DataSourceExec;
use datafusion::physical_plan::ExecutionPlan;
use std::sync::Arc;

/// Metadata extracted from a Parquet scan operation.
#[derive(Debug, Clone)]
pub struct ParquetScanMetadata {
    /// List of files being scanned with their sizes.
    pub files: Vec<ParquetFileInfo>,
    /// Number of columns in the file schema.
    pub schema_column_count: usize,
    /// Total size of all files in bytes.
    pub total_file_size_bytes: u64,
    /// Total number of files being scanned.
    pub file_count: usize,
    /// Derived table name from common path prefix.
    pub table_name: Option<String>,
}

/// Information about a single Parquet file.
#[derive(Debug, Clone)]
pub struct ParquetFileInfo {
    /// The file path.
    pub path: String,
    /// File size in bytes.
    pub size_bytes: u64,
}

impl ParquetScanMetadata {
    /// Extract Parquet metadata from an execution plan tree.
    ///
    /// This walks the plan tree looking for DataSourceExec nodes that use
    /// ParquetSource, and extracts file and schema information.
    pub fn from_plan(plan: &Arc<dyn ExecutionPlan>) -> Vec<Self> {
        let mut results = Vec::new();
        Self::collect_from_plan(plan, &mut results);
        results
    }

    fn collect_from_plan(plan: &Arc<dyn ExecutionPlan>, results: &mut Vec<Self>) {
        // Try to extract ParquetSource metadata from this node
        if let Some(metadata) = Self::try_extract_from_node(plan.as_ref()) {
            results.push(metadata);
        }

        // Recurse into children
        for child in plan.children() {
            Self::collect_from_plan(child, results);
        }
    }

    fn try_extract_from_node(plan: &dyn ExecutionPlan) -> Option<Self> {
        // Check if this is a DataSourceExec
        let data_source_exec = plan.as_any().downcast_ref::<DataSourceExec>()?;

        // Get the data source and try to downcast to FileScanConfig
        let data_source = data_source_exec.data_source();
        let file_scan_config = data_source.as_any().downcast_ref::<FileScanConfig>()?;

        // Check if the file source is ParquetSource
        let _parquet_source = file_scan_config
            .file_source
            .as_any()
            .downcast_ref::<ParquetSource>()?;

        // Extract file information
        let files: Vec<ParquetFileInfo> = file_scan_config
            .file_groups
            .iter()
            .flat_map(|group| group.iter())
            .map(|file: &PartitionedFile| ParquetFileInfo {
                path: file.object_meta.location.to_string(),
                size_bytes: file.object_meta.size,
            })
            .collect();

        let total_file_size_bytes = files.iter().map(|f| f.size_bytes).sum();
        let file_count = files.len();
        let schema_column_count = file_scan_config.file_schema.fields().len();

        // Derive table name from common path prefix or first file's parent directory
        let table_name = Self::derive_table_name(&files);

        Some(Self {
            files,
            schema_column_count,
            total_file_size_bytes,
            file_count,
            table_name,
        })
    }

    /// Derive a table name from file paths.
    ///
    /// Strategy:
    /// 1. If there's only one file, use its parent directory name
    /// 2. If there are multiple files, find common path prefix and use its last component
    /// 3. Falls back to the first file's parent directory
    fn derive_table_name(files: &[ParquetFileInfo]) -> Option<String> {
        if files.is_empty() {
            return None;
        }

        // Get paths without leading slashes for consistent handling
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();

        if paths.len() == 1 {
            // Single file: use parent directory name
            return Self::extract_parent_dir_name(paths[0]);
        }

        // Multiple files: find common prefix
        let common_prefix = Self::find_common_path_prefix(&paths);
        if let Some(prefix) = common_prefix {
            // Use the last directory component of the common prefix
            if let Some(name) = Self::extract_last_path_component(&prefix) {
                return Some(name);
            }
        }

        // Fallback: use first file's parent directory
        Self::extract_parent_dir_name(paths[0])
    }

    /// Extract the parent directory name from a path.
    fn extract_parent_dir_name(path: &str) -> Option<String> {
        // Remove trailing filename
        let path = path.trim_end_matches(|c| c != '/');
        let path = path.trim_end_matches('/');
        Self::extract_last_path_component(path)
    }

    /// Extract the last component of a path (directory or file name).
    fn extract_last_path_component(path: &str) -> Option<String> {
        path.rsplit('/')
            .find(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    /// Find the common directory prefix among multiple paths.
    fn find_common_path_prefix(paths: &[&str]) -> Option<String> {
        if paths.is_empty() {
            return None;
        }

        let first = paths[0];

        // Split first path into directory components
        let first_parts: Vec<&str> = first.split('/').filter(|s| !s.is_empty()).collect();

        let mut common_depth = first_parts.len();

        for path in paths.iter().skip(1) {
            let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let mut matching = 0;
            for (a, b) in first_parts.iter().zip(parts.iter()) {
                if a == b {
                    matching += 1;
                } else {
                    break;
                }
            }
            common_depth = common_depth.min(matching);
        }

        // Exclude the last component (which might be a filename or varying directory)
        // We want the common parent directory
        if common_depth > 0 {
            // Return path up to common_depth - 1 (parent of the varying part)
            let prefix_parts: Vec<&str> = first_parts.iter().take(common_depth).copied().collect();
            Some(prefix_parts.join("/"))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parquet_file_info_debug() {
        let info = ParquetFileInfo {
            path: "test.parquet".to_string(),
            size_bytes: 1024,
        };
        let debug_str = format!("{info:?}");
        assert!(debug_str.contains("test.parquet"));
        assert!(debug_str.contains("1024"));
    }

    #[test]
    fn test_parquet_scan_metadata_empty() {
        let metadata = ParquetScanMetadata {
            files: vec![],
            schema_column_count: 5,
            total_file_size_bytes: 0,
            file_count: 0,
            table_name: None,
        };
        assert_eq!(metadata.file_count, 0);
        assert_eq!(metadata.total_file_size_bytes, 0);
    }

    #[test]
    fn test_derive_table_name_single_file() {
        let files = vec![ParquetFileInfo {
            path: "s3://bucket/data/my_table/part-0000.parquet".to_string(),
            size_bytes: 1024,
        }];
        let name = ParquetScanMetadata::derive_table_name(&files);
        assert_eq!(name, Some("my_table".to_string()));
    }

    #[test]
    fn test_derive_table_name_multiple_files_same_dir() {
        let files = vec![
            ParquetFileInfo {
                path: "s3://bucket/data/orders/part-0000.parquet".to_string(),
                size_bytes: 1024,
            },
            ParquetFileInfo {
                path: "s3://bucket/data/orders/part-0001.parquet".to_string(),
                size_bytes: 2048,
            },
        ];
        let name = ParquetScanMetadata::derive_table_name(&files);
        // Common prefix is "bucket/data/orders", last component is "orders"
        assert_eq!(name, Some("orders".to_string()));
    }

    #[test]
    fn test_derive_table_name_empty() {
        let files: Vec<ParquetFileInfo> = vec![];
        let name = ParquetScanMetadata::derive_table_name(&files);
        assert_eq!(name, None);
    }

    #[test]
    fn test_extract_parent_dir_name() {
        assert_eq!(
            ParquetScanMetadata::extract_parent_dir_name("/a/b/c/file.parquet"),
            Some("c".to_string())
        );
        assert_eq!(
            ParquetScanMetadata::extract_parent_dir_name("bucket/table/file.parquet"),
            Some("table".to_string())
        );
    }
}
