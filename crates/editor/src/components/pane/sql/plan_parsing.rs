//! Demo data generation for SQL pane visualizations.
//!
//! This module contains demo data generators for testing plan visualizations,
//! diff views, and schema comparisons. Parsing functions are provided by the
//! `enya_datafusion` crate.

use std::time::Duration;

use enya_datafusion::{OperatorMetrics, PlanNode};
use rustc_hash::FxHashMap;

// Re-export parse_plan_text for use by pane.rs
pub use enya_datafusion::parse_plan_text;

// =============================================================================
// Demo Data Generators
// =============================================================================

/// Create a demo query plan for testing the visualization.
pub fn create_demo_plan() -> PlanNode {
    PlanNode {
        operator: "ProjectionExec".to_string(),
        description: "user_id, name, total_orders, last_order_date".to_string(),
        properties: FxHashMap::default(),
        metrics: Some(OperatorMetrics {
            output_rows: 1000,
            elapsed_time: Duration::from_millis(5),
            memory_bytes: 32768,
            spill_count: 0,
            spill_bytes: 0,
        }),
        children: vec![PlanNode {
            operator: "SortExec".to_string(),
            description: "total_orders DESC".to_string(),
            properties: FxHashMap::default(),
            metrics: Some(OperatorMetrics {
                output_rows: 1000,
                elapsed_time: Duration::from_millis(25),
                memory_bytes: 65536,
                spill_count: 0,
                spill_bytes: 0,
            }),
            children: vec![PlanNode {
                operator: "HashAggregateExec".to_string(),
                description: "group_by=[user_id], aggr=[COUNT(*), MAX(order_date)]".to_string(),
                properties: FxHashMap::default(),
                metrics: Some(OperatorMetrics {
                    output_rows: 1000,
                    elapsed_time: Duration::from_millis(45),
                    memory_bytes: 131072,
                    spill_count: 0,
                    spill_bytes: 0,
                }),
                children: vec![PlanNode {
                    operator: "HashJoinExec".to_string(),
                    description: "users.id = orders.user_id, type=Inner".to_string(),
                    properties: FxHashMap::default(),
                    metrics: Some(OperatorMetrics {
                        output_rows: 50000,
                        elapsed_time: Duration::from_millis(120),
                        memory_bytes: 524288,
                        spill_count: 0,
                        spill_bytes: 0,
                    }),
                    children: vec![
                        PlanNode {
                            operator: "ParquetExec".to_string(),
                            description: "users.parquet, projection=[id, name]".to_string(),
                            properties: FxHashMap::default(),
                            metrics: Some(OperatorMetrics {
                                output_rows: 10000,
                                elapsed_time: Duration::from_millis(85),
                                memory_bytes: 262144,
                                spill_count: 0,
                                spill_bytes: 0,
                            }),
                            children: vec![],
                        },
                        PlanNode {
                            operator: "FilterExec".to_string(),
                            description: "order_date >= '2024-01-01'".to_string(),
                            properties: FxHashMap::default(),
                            metrics: Some(OperatorMetrics {
                                output_rows: 50000,
                                elapsed_time: Duration::from_millis(15),
                                memory_bytes: 8192,
                                spill_count: 0,
                                spill_bytes: 0,
                            }),
                            children: vec![PlanNode {
                                operator: "ParquetExec".to_string(),
                                description: "orders.parquet, projection=[user_id, order_date]"
                                    .to_string(),
                                properties: FxHashMap::default(),
                                metrics: Some(OperatorMetrics {
                                    output_rows: 100000,
                                    elapsed_time: Duration::from_millis(150),
                                    memory_bytes: 1048576,
                                    spill_count: 0,
                                    spill_bytes: 0,
                                }),
                                children: vec![],
                            }],
                        },
                    ],
                }],
            }],
        }],
    }
}

/// Create demo profile diff plans (left=staging slower, right=production faster).
pub fn create_profile_diff_demo() -> (PlanNode, PlanNode) {
    // Staging: slower - full table scans, no partition pruning, memory pressure
    let left_plan = PlanNode {
        operator: "GlobalLimitExec".to_string(),
        description: "skip=0, fetch=100".to_string(),
        properties: FxHashMap::default(),
        metrics: Some(OperatorMetrics {
            output_rows: 100,
            elapsed_time: Duration::from_millis(2),
            memory_bytes: 4096,
            spill_count: 0,
            spill_bytes: 0,
        }),
        children: vec![PlanNode {
            operator: "SortExec".to_string(),
            description: "revenue DESC".to_string(),
            properties: FxHashMap::default(),
            metrics: Some(OperatorMetrics {
                output_rows: 5000,
                elapsed_time: Duration::from_millis(180),
                memory_bytes: 2097152,
                spill_count: 2,
                spill_bytes: 1048576,
            }),
            children: vec![PlanNode {
                operator: "HashAggregateExec".to_string(),
                description: "group_by=[name], aggr=[COUNT(*), SUM(total)]".to_string(),
                properties: FxHashMap::default(),
                metrics: Some(OperatorMetrics {
                    output_rows: 5000,
                    elapsed_time: Duration::from_millis(320),
                    memory_bytes: 4194304,
                    spill_count: 1,
                    spill_bytes: 524288,
                }),
                children: vec![PlanNode {
                    operator: "HashJoinExec".to_string(),
                    description: "orders.customer_id = customers.id".to_string(),
                    properties: FxHashMap::default(),
                    metrics: Some(OperatorMetrics {
                        output_rows: 250000,
                        elapsed_time: Duration::from_millis(420),
                        memory_bytes: 8388608,
                        spill_count: 3,
                        spill_bytes: 2097152,
                    }),
                    children: vec![
                        PlanNode {
                            operator: "FilterExec".to_string(),
                            description: "status = 'completed' AND created_at > '2024-01-01'"
                                .to_string(),
                            properties: FxHashMap::default(),
                            metrics: Some(OperatorMetrics {
                                output_rows: 250000,
                                elapsed_time: Duration::from_millis(85),
                                memory_bytes: 65536,
                                spill_count: 0,
                                spill_bytes: 0,
                            }),
                            children: vec![PlanNode {
                                operator: "ParquetExec".to_string(),
                                description: "orders.parquet (full scan, no partition pruning)"
                                    .to_string(),
                                properties: FxHashMap::default(),
                                metrics: Some(OperatorMetrics {
                                    output_rows: 1000000,
                                    elapsed_time: Duration::from_millis(850),
                                    memory_bytes: 16777216,
                                    spill_count: 0,
                                    spill_bytes: 0,
                                }),
                                children: vec![],
                            }],
                        },
                        PlanNode {
                            operator: "ParquetExec".to_string(),
                            description: "customers.parquet".to_string(),
                            properties: FxHashMap::default(),
                            metrics: Some(OperatorMetrics {
                                output_rows: 50000,
                                elapsed_time: Duration::from_millis(45),
                                memory_bytes: 2097152,
                                spill_count: 0,
                                spill_bytes: 0,
                            }),
                            children: vec![],
                        },
                    ],
                }],
            }],
        }],
    };

    // Production: faster - partition pruning, indexed lookups, optimized memory
    let right_plan = PlanNode {
        operator: "GlobalLimitExec".to_string(),
        description: "skip=0, fetch=100".to_string(),
        properties: FxHashMap::default(),
        metrics: Some(OperatorMetrics {
            output_rows: 100,
            elapsed_time: Duration::from_millis(1),
            memory_bytes: 4096,
            spill_count: 0,
            spill_bytes: 0,
        }),
        children: vec![PlanNode {
            operator: "SortExec".to_string(),
            description: "revenue DESC".to_string(),
            properties: FxHashMap::default(),
            metrics: Some(OperatorMetrics {
                output_rows: 5000,
                elapsed_time: Duration::from_millis(45),
                memory_bytes: 524288,
                spill_count: 0,
                spill_bytes: 0,
            }),
            children: vec![PlanNode {
                operator: "HashAggregateExec".to_string(),
                description: "group_by=[name], aggr=[COUNT(*), SUM(total)]".to_string(),
                properties: FxHashMap::default(),
                metrics: Some(OperatorMetrics {
                    output_rows: 5000,
                    elapsed_time: Duration::from_millis(78),
                    memory_bytes: 1048576,
                    spill_count: 0,
                    spill_bytes: 0,
                }),
                children: vec![PlanNode {
                    operator: "HashJoinExec".to_string(),
                    description: "orders.customer_id = customers.id".to_string(),
                    properties: FxHashMap::default(),
                    metrics: Some(OperatorMetrics {
                        output_rows: 45000,
                        elapsed_time: Duration::from_millis(65),
                        memory_bytes: 2097152,
                        spill_count: 0,
                        spill_bytes: 0,
                    }),
                    children: vec![
                        PlanNode {
                            operator: "FilterExec".to_string(),
                            description: "status = 'completed' AND created_at > '2024-01-01'"
                                .to_string(),
                            properties: FxHashMap::default(),
                            metrics: Some(OperatorMetrics {
                                output_rows: 45000,
                                elapsed_time: Duration::from_millis(12),
                                memory_bytes: 32768,
                                spill_count: 0,
                                spill_bytes: 0,
                            }),
                            children: vec![PlanNode {
                                operator: "ParquetExec".to_string(),
                                description: "orders.parquet (partition pruning: 2024-*)"
                                    .to_string(),
                                properties: FxHashMap::default(),
                                metrics: Some(OperatorMetrics {
                                    output_rows: 150000,
                                    elapsed_time: Duration::from_millis(95),
                                    memory_bytes: 4194304,
                                    spill_count: 0,
                                    spill_bytes: 0,
                                }),
                                children: vec![],
                            }],
                        },
                        PlanNode {
                            operator: "ParquetExec".to_string(),
                            description: "customers.parquet".to_string(),
                            properties: FxHashMap::default(),
                            metrics: Some(OperatorMetrics {
                                output_rows: 50000,
                                elapsed_time: Duration::from_millis(42),
                                memory_bytes: 2097152,
                                spill_count: 0,
                                spill_bytes: 0,
                            }),
                            children: vec![],
                        },
                    ],
                }],
            }],
        }],
    };

    (left_plan, right_plan)
}

/// Create demo data diff result (staging vs production users table).
pub fn create_diff_demo() -> super::types::DiffQueryResult {
    use enya_datafusion::arrow::array::{Int32Array, StringArray};
    use enya_datafusion::arrow::datatypes::{DataType, Field, Schema};
    use enya_datafusion::arrow::record_batch::RecordBatch;
    use std::sync::Arc;

    use super::types::{DiffQueryResult, DiffStats, DiffType};

    // Create schema for demo data - realistic users table
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int32, false),
        Field::new("email", DataType::Utf8, true),
        Field::new("name", DataType::Utf8, true),
        Field::new("role", DataType::Utf8, true),
    ]));

    // STAGING environment - includes test users and some synced production users
    let left_batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3, 100, 101, 102])),
            Arc::new(StringArray::from(vec![
                "alice@acme.com",
                "bob@acme.com",
                "carol@acme.com",
                "test@staging.local",
                "qa@staging.local",
                "demo@staging.local",
            ])),
            Arc::new(StringArray::from(vec![
                "Alice Chen",
                "Bob Smith",
                "Carol Jones",
                "Test User",
                "QA Engineer",
                "Demo Account",
            ])),
            Arc::new(StringArray::from(vec![
                "admin", "editor", "viewer", "admin", "editor", "viewer",
            ])),
        ],
    )
    .unwrap();

    // PRODUCTION environment - real users only
    let right_batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Int32Array::from(vec![1, 2, 3, 4, 5, 6])),
            Arc::new(StringArray::from(vec![
                "alice@acme.com",
                "bob@acme.com",
                "carol@acme.com",
                "dave@acme.com",
                "emma@acme.com",
                "frank@acme.com",
            ])),
            Arc::new(StringArray::from(vec![
                "Alice Chen",
                "Bob Smith",
                "Carol Jones",
                "Dave Wilson",
                "Emma Brown",
                "Frank Garcia",
            ])),
            Arc::new(StringArray::from(vec![
                "admin", "editor", "viewer", "editor", "viewer", "viewer",
            ])),
        ],
    )
    .unwrap();

    DiffQueryResult {
        left_name: "staging".to_string(),
        right_name: "production".to_string(),
        left_schema: Some(schema.clone()),
        left_batches: vec![left_batch],
        left_error: None,
        right_schema: Some(schema),
        right_batches: vec![right_batch],
        right_error: None,
        schemas_match: true,
        diff_stats: Some(DiffStats {
            matching: 3,
            left_only: 3,
            right_only: 3,
            different: 0,
        }),
        left_plan: None,
        right_plan: None,
        diff_type: DiffType::Data,
        schema_diff: None,
    }
}

/// Create demo schema diff result (staging vs production users table schema).
pub fn create_schema_diff_demo() -> super::types::DiffQueryResult {
    use super::types::{
        ColumnDiffStatus, DiffQueryResult, DiffType, SchemaDiffColumn, SchemaDiffResult,
    };

    let schema_diff = SchemaDiffResult {
        table_name: "users".to_string(),
        columns: vec![
            // Matching columns
            SchemaDiffColumn {
                name: "id".to_string(),
                left_type: Some("INT".to_string()),
                left_nullable: Some(false),
                right_type: Some("INT".to_string()),
                right_nullable: Some(false),
                status: ColumnDiffStatus::Matching,
            },
            SchemaDiffColumn {
                name: "email".to_string(),
                left_type: Some("VARCHAR(255)".to_string()),
                left_nullable: Some(false),
                right_type: Some("VARCHAR(255)".to_string()),
                right_nullable: Some(false),
                status: ColumnDiffStatus::Matching,
            },
            SchemaDiffColumn {
                name: "name".to_string(),
                left_type: Some("VARCHAR(100)".to_string()),
                left_nullable: Some(true),
                right_type: Some("VARCHAR(100)".to_string()),
                right_nullable: Some(true),
                status: ColumnDiffStatus::Matching,
            },
            // Changed column - type difference
            SchemaDiffColumn {
                name: "status".to_string(),
                left_type: Some("VARCHAR(20)".to_string()),
                left_nullable: Some(true),
                right_type: Some("INT".to_string()),
                right_nullable: Some(false),
                status: ColumnDiffStatus::Changed,
            },
            // Left-only columns
            SchemaDiffColumn {
                name: "test_flag".to_string(),
                left_type: Some("BOOLEAN".to_string()),
                left_nullable: Some(true),
                right_type: None,
                right_nullable: None,
                status: ColumnDiffStatus::LeftOnly,
            },
            SchemaDiffColumn {
                name: "debug_info".to_string(),
                left_type: Some("TEXT".to_string()),
                left_nullable: Some(true),
                right_type: None,
                right_nullable: None,
                status: ColumnDiffStatus::LeftOnly,
            },
            // Right-only columns
            SchemaDiffColumn {
                name: "created_at".to_string(),
                left_type: None,
                left_nullable: None,
                right_type: Some("TIMESTAMP".to_string()),
                right_nullable: Some(false),
                status: ColumnDiffStatus::RightOnly,
            },
            SchemaDiffColumn {
                name: "updated_at".to_string(),
                left_type: None,
                left_nullable: None,
                right_type: Some("TIMESTAMP".to_string()),
                right_nullable: Some(true),
                status: ColumnDiffStatus::RightOnly,
            },
            SchemaDiffColumn {
                name: "deleted_at".to_string(),
                left_type: None,
                left_nullable: None,
                right_type: Some("TIMESTAMP".to_string()),
                right_nullable: Some(true),
                status: ColumnDiffStatus::RightOnly,
            },
        ],
        matching: 3,
        left_only: 2,
        right_only: 3,
        changed: 1,
    };

    DiffQueryResult {
        left_name: "staging".to_string(),
        right_name: "production".to_string(),
        left_schema: None,
        left_batches: Vec::new(),
        left_error: None,
        right_schema: None,
        right_batches: Vec::new(),
        right_error: None,
        schemas_match: false,
        diff_stats: None,
        left_plan: None,
        right_plan: None,
        diff_type: DiffType::Schema,
        schema_diff: Some(schema_diff),
    }
}

#[cfg(test)]
mod tests {
    use enya_datafusion::{
        parse_metric_bytes, parse_metric_duration, parse_metric_usize, parse_plan_text,
    };

    #[test]
    fn test_parse_metric_usize() {
        assert_eq!(parse_metric_usize("rows=100, time=5ms", "rows="), Some(100));
        assert_eq!(
            parse_metric_usize("output_rows=12345", "output_rows="),
            Some(12345)
        );
        assert_eq!(parse_metric_usize("no match", "rows="), None);
    }

    #[test]
    fn test_parse_metric_duration() {
        let d = parse_metric_duration("time=5.2ms", "time=").unwrap();
        assert!(d.as_micros() > 5000 && d.as_micros() < 5300);

        let d = parse_metric_duration("elapsed_compute=52.06µs", "elapsed_compute=").unwrap();
        assert!(d.as_micros() > 50 && d.as_micros() < 55);
    }

    #[test]
    fn test_parse_metric_bytes() {
        assert_eq!(parse_metric_bytes("mem=1024 B", "mem="), Some(1024));
        assert_eq!(
            parse_metric_bytes("output_bytes=1.5 KB", "output_bytes="),
            Some(1536)
        );
        assert_eq!(parse_metric_bytes("mem=1 MB", "mem="), Some(1048576));
    }

    #[test]
    fn test_parse_plan_text() {
        let plan_text = r#"
ProjectionExec: col1, col2
  FilterExec: x > 10
    TableScan: my_table
"#;
        let plan = parse_plan_text(plan_text);
        assert_eq!(plan.operator, "ProjectionExec");
        assert_eq!(plan.children.len(), 1);
        assert_eq!(plan.children[0].operator, "FilterExec");
        assert_eq!(plan.children[0].children[0].operator, "TableScan");
    }
}
