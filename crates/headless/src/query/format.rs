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
