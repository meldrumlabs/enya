use super::format;
use crate::Result;

/// Parse a file spec: `"name=path"` or bare `"path"` (table name from filename).
fn parse_file_spec(spec: &str) -> Result<(String, String)> {
    if let Some((name, path)) = spec.split_once('=') {
        Ok((name.to_string(), path.to_string()))
    } else {
        let path = std::path::Path::new(spec);
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("could not derive table name from file path")?;
        Ok((name.to_string(), spec.to_string()))
    }
}

pub fn query(expression: &str, files: &[String], limit: Option<usize>, json: bool) -> Result {
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("failed to create async runtime: {e}"))?;

    rt.block_on(async {
        let session = enya_datafusion::Session::new();

        for file_spec in files {
            let (name, path) = parse_file_spec(file_spec)?;
            session
                .register_file(&name, &path)
                .await
                .map_err(|e| format!("failed to register '{name}' from '{path}': {e}"))?;
        }

        // Apply LIMIT if not already in the SQL
        let sql = if let Some(lim) = limit {
            let upper = expression.to_uppercase();
            if upper.contains("LIMIT") {
                expression.to_string()
            } else {
                format!("{expression} LIMIT {lim}")
            }
        } else {
            expression.to_string()
        };

        let batches = session
            .execute_collect(&sql)
            .await
            .map_err(|e| format!("query failed: {e}"))?;

        if json {
            format::print_sql_json(&batches)?;
        } else {
            format::print_sql_table(&batches)?;
        }

        Ok(())
    })
}
