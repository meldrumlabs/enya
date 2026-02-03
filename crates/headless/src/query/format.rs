use super::discovery::{MetricInfo, SeriesEntry};
use super::promql::PromData;
use super::time::format_timestamp;
use crate::Result;

/// Format PromQL metric labels as `{key="value", ...}`.
fn format_metric_labels(metric: &serde_json::Map<String, serde_json::Value>) -> String {
    let pairs: Vec<String> = metric
        .iter()
        .filter(|(k, _)| k.as_str() != "__name__")
        .map(|(k, v)| {
            let val = v.as_str().unwrap_or("");
            format!("{k}=\"{val}\"")
        })
        .collect();

    if pairs.is_empty() {
        "{}".to_string()
    } else {
        format!("{{{}}}", pairs.join(", "))
    }
}

/// Print PromQL results as a human-readable table.
pub fn print_promql_table(data: &PromData, limit: Option<usize>) -> Result {
    if data.result.is_empty() {
        println!("(empty result)");
        return Ok(());
    }

    let rows: Vec<(String, String, String)> = data
        .result
        .iter()
        .flat_map(|series| {
            let labels = format_metric_labels(&series.metric);
            series
                .values
                .iter()
                .map(move |(ts, val)| (labels.clone(), format_timestamp(*ts), val.clone()))
        })
        .take(limit.unwrap_or(usize::MAX))
        .collect();

    let w0 = rows.iter().map(|r| r.0.len()).max().unwrap_or(6).max(6);
    let w1 = rows.iter().map(|r| r.1.len()).max().unwrap_or(9).max(9);

    println!("{:<w0$}  {:<w1$}  VALUE", "METRIC", "TIMESTAMP");
    for row in &rows {
        println!("{:<w0$}  {:<w1$}  {}", row.0, row.1, row.2);
    }

    println!("\n{} samples from {} series", rows.len(), data.result.len());
    Ok(())
}

/// Print PromQL results as JSON.
pub fn print_promql_json(data: &PromData, limit: Option<usize>) -> Result {
    let series: Vec<serde_json::Value> = data
        .result
        .iter()
        .map(|r| {
            let values: Vec<serde_json::Value> = r
                .values
                .iter()
                .take(limit.unwrap_or(usize::MAX))
                .map(|(ts, val)| serde_json::json!({"timestamp": ts, "value": val}))
                .collect();
            serde_json::json!({
                "metric": r.metric,
                "values": values,
            })
        })
        .collect();

    println!(
        "{}",
        serde_json::json!({
            "result_type": data.result_type,
            "series": series,
            "series_count": data.result.len(),
        })
    );
    Ok(())
}

// -- Discovery output formatters -----------------------------------------------

/// Print a list of strings as a single-column table.
pub fn print_string_list(header: &str, items: &[String]) -> Result {
    if items.is_empty() {
        println!("(empty result)");
        return Ok(());
    }
    println!("{header}");
    for item in items {
        println!("{item}");
    }
    println!("\n{} items", items.len());
    Ok(())
}

/// Print a list of strings as JSON: `{"key": [...], "count": N}`.
pub fn print_string_list_json(key: &str, items: &[String]) -> Result {
    println!(
        "{}",
        serde_json::json!({
            key: items,
            "count": items.len(),
        })
    );
    Ok(())
}

/// Print metric metadata as a table with METRIC, TYPE, and HELP columns.
pub fn print_metric_info_table(infos: &[MetricInfo]) -> Result {
    if infos.is_empty() {
        println!("(no metadata found)");
        return Ok(());
    }

    let w0 = infos
        .iter()
        .map(|i| i.metric.len())
        .max()
        .unwrap_or(6)
        .max(6);
    let w1 = infos
        .iter()
        .map(|i| i.metric_type.len())
        .max()
        .unwrap_or(4)
        .max(4);

    println!("{:<w0$}  {:<w1$}  HELP", "METRIC", "TYPE");
    for info in infos {
        println!(
            "{:<w0$}  {:<w1$}  {}",
            info.metric, info.metric_type, info.help
        );
    }
    println!("\n{} metrics", infos.len());
    Ok(())
}

/// Print metric metadata as JSON.
pub fn print_metric_info_json(infos: &[MetricInfo]) -> Result {
    let items: Vec<serde_json::Value> = infos
        .iter()
        .map(|i| {
            serde_json::json!({
                "metric": i.metric,
                "type": i.metric_type,
                "help": i.help,
                "unit": i.unit,
            })
        })
        .collect();
    println!(
        "{}",
        serde_json::json!({
            "metrics": items,
            "count": items.len(),
        })
    );
    Ok(())
}

/// Print series entries as a table.
pub fn print_series_table(entries: &[SeriesEntry]) -> Result {
    if entries.is_empty() {
        println!("(no matching series)");
        return Ok(());
    }

    println!("SERIES");
    for entry in entries {
        let mut pairs: Vec<String> = entry
            .labels
            .iter()
            .map(|(k, v)| format!("{k}=\"{v}\""))
            .collect();
        pairs.sort();
        println!("{{{}}}", pairs.join(", "));
    }
    println!("\n{} series", entries.len());
    Ok(())
}

/// Print series entries as JSON.
pub fn print_series_json(entries: &[SeriesEntry]) -> Result {
    let items: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| serde_json::json!(e.labels))
        .collect();
    println!(
        "{}",
        serde_json::json!({
            "series": items,
            "count": items.len(),
        })
    );
    Ok(())
}

/// Print SQL results as a human-readable table.
#[cfg(feature = "sql")]
pub fn print_sql_table(batches: &[enya_datafusion::arrow::array::RecordBatch]) -> Result {
    if batches.is_empty() {
        println!("(empty result)");
        return Ok(());
    }

    let schema = batches[0].schema();
    let columns: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();

    let mut rows: Vec<Vec<String>> = Vec::new();
    for batch in batches {
        for row_idx in 0..batch.num_rows() {
            let row: Vec<String> = (0..batch.num_columns())
                .map(|col_idx| enya_datafusion::format_array_value(batch.column(col_idx), row_idx))
                .collect();
            rows.push(row);
        }
    }

    // Compute column widths
    let mut widths: Vec<usize> = columns.iter().map(|c| c.len()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(cell.len());
            }
        }
    }

    // Header
    let header: String = columns
        .iter()
        .enumerate()
        .map(|(i, col)| format!("{:<width$}", col, width = widths[i]))
        .collect::<Vec<_>>()
        .join("  ");
    println!("{header}");

    // Separator
    let sep: String = widths
        .iter()
        .map(|w| "-".repeat(*w))
        .collect::<Vec<_>>()
        .join("  ");
    println!("{sep}");

    // Rows
    for row in &rows {
        let line: String = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                format!(
                    "{:<width$}",
                    cell,
                    width = widths.get(i).copied().unwrap_or(0)
                )
            })
            .collect::<Vec<_>>()
            .join("  ");
        println!("{line}");
    }

    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    println!("\n{total_rows} rows, {} columns", columns.len());
    Ok(())
}

/// Print SQL results as JSON.
#[cfg(feature = "sql")]
pub fn print_sql_json(batches: &[enya_datafusion::arrow::array::RecordBatch]) -> Result {
    if batches.is_empty() {
        println!(
            "{}",
            serde_json::json!({"columns": [], "rows": [], "row_count": 0})
        );
        return Ok(());
    }

    let schema = batches[0].schema();
    let columns: Vec<serde_json::Value> = schema
        .fields()
        .iter()
        .map(|f| {
            serde_json::json!({
                "name": f.name(),
                "type": f.data_type().to_string(),
            })
        })
        .collect();

    let mut rows: Vec<serde_json::Value> = Vec::new();
    for batch in batches {
        for row_idx in 0..batch.num_rows() {
            let mut obj = serde_json::Map::new();
            for (col_idx, field) in schema.fields().iter().enumerate() {
                let value = enya_datafusion::format_array_value(batch.column(col_idx), row_idx);
                obj.insert(field.name().clone(), serde_json::Value::String(value));
            }
            rows.push(serde_json::Value::Object(obj));
        }
    }

    let total_rows = rows.len();
    println!(
        "{}",
        serde_json::json!({
            "columns": columns,
            "rows": rows,
            "row_count": total_rows,
        })
    );
    Ok(())
}
