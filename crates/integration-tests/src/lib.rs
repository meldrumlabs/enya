//! Integration tests for the Enya metrics store and REST API.
//!
//! Tests the complete flow from data ingestion through to REST API queries.

#[cfg(test)]
mod tests {
    use axum_test::TestServer;
    use enya::testing::{Core, build_router};
    use enya_metrics_store::{Database, MetricName, MetricsStore, object_store};
    use object_store::memory::InMemory;
    use std::sync::Arc;
    use std::time::Duration;

    /// Creates a test server with a fresh in-memory metrics store.
    async fn test_server() -> (TestServer, MetricsStore) {
        let object_store = Arc::new(InMemory::new());
        let db = Database::builder()
            .with_flush_interval(Duration::from_millis(10))
            .open(object_store, "/")
            .await
            .expect("database");
        let metrics_store = MetricsStore::new(db, None, None);
        let build_info = enya_build_info::build_info!();
        let core = Core::new(build_info, metrics_store.clone());
        let app = build_router(core);
        let server = TestServer::new(app).expect("test server");
        (server, metrics_store)
    }

    fn metric(name: &str) -> MetricName<'_> {
        MetricName::try_from(name).expect("valid metric name")
    }

    // =========================================================================
    // Health endpoint tests
    // =========================================================================

    #[tokio::test]
    async fn health_endpoint_returns_success() {
        let (server, _store) = test_server().await;

        let response = server.get("/api/health").await;
        response.assert_status_ok();

        let body: serde_json::Value = response.json();
        assert_eq!(body["msg"], "Enya is up");
    }

    // =========================================================================
    // Data ingestion tests
    // =========================================================================

    #[tokio::test]
    async fn ingested_data_is_queryable() {
        let (server, store) = test_server().await;

        // Ingest data directly via the store
        let m = metric("request.count");
        let tags = [("service", "api"), ("env", "prod")];
        store.ingest(m, 100.0, &tags).await.expect("ingest");

        // Query via REST API
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "request.count")
            .add_query_param("group_by", "service")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["group"], "api");
        assert_eq!(groups[0]["buckets"][0]["value"].as_f64().unwrap(), 100.0);
    }

    #[tokio::test]
    async fn multiple_series_ingestion() {
        let (server, store) = test_server().await;

        let m = metric("cpu.usage");
        store
            .ingest(m, 25.0, &[("host", "server1")])
            .await
            .expect("ingest");
        store
            .ingest(m, 50.0, &[("host", "server2")])
            .await
            .expect("ingest");
        store
            .ingest(m, 75.0, &[("host", "server3")])
            .await
            .expect("ingest");

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "cpu.usage")
            .add_query_param("group_by", "host")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");
        assert_eq!(groups.len(), 3);
    }

    // =========================================================================
    // Filter expression tests
    // =========================================================================

    #[tokio::test]
    async fn filter_exact_match() {
        let (server, store) = test_server().await;

        let m = metric("http.requests");
        store
            .ingest(m, 10.0, &[("env", "prod"), ("service", "api")])
            .await
            .expect("ingest");
        store
            .ingest(m, 20.0, &[("env", "staging"), ("service", "api")])
            .await
            .expect("ingest");
        store
            .ingest(m, 30.0, &[("env", "prod"), ("service", "web")])
            .await
            .expect("ingest");

        // Filter for prod only
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "http.requests")
            .add_query_param("group_by", "service")
            .add_query_param("filter", "env:prod")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");

        // Should have 2 groups: api (10.0) and web (30.0)
        assert_eq!(groups.len(), 2);
        for group in groups {
            let name = group["group"].as_str().unwrap();
            let value = group["buckets"][0]["value"].as_f64().unwrap();
            match name {
                "api" => assert_eq!(value, 10.0),
                "web" => assert_eq!(value, 30.0),
                _ => panic!("unexpected group: {name}"),
            }
        }
    }

    #[tokio::test]
    async fn filter_and_expression() {
        let (server, store) = test_server().await;

        let m = metric("db.queries");
        store
            .ingest(m, 5.0, &[("env", "prod"), ("region", "us")])
            .await
            .expect("ingest");
        store
            .ingest(m, 10.0, &[("env", "prod"), ("region", "eu")])
            .await
            .expect("ingest");
        store
            .ingest(m, 15.0, &[("env", "staging"), ("region", "us")])
            .await
            .expect("ingest");

        // Filter for prod AND us
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "db.queries")
            .add_query_param("group_by", "region")
            .add_query_param("filter", "env:prod AND region:us")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0]["group"], "us");
        assert_eq!(groups[0]["buckets"][0]["value"].as_f64().unwrap(), 5.0);
    }

    #[tokio::test]
    async fn filter_or_expression() {
        let (server, store) = test_server().await;

        let m = metric("cache.hits");
        store
            .ingest(m, 100.0, &[("region", "us")])
            .await
            .expect("ingest");
        store
            .ingest(m, 200.0, &[("region", "eu")])
            .await
            .expect("ingest");
        store
            .ingest(m, 50.0, &[("region", "asia")])
            .await
            .expect("ingest");

        // Filter for us OR eu
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "cache.hits")
            .add_query_param("group_by", "region")
            .add_query_param("filter", "region:us OR region:eu")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");

        assert_eq!(groups.len(), 2);
    }

    #[tokio::test]
    async fn filter_not_expression() {
        let (server, store) = test_server().await;

        let m = metric("error.count");
        store
            .ingest(m, 5.0, &[("level", "warn")])
            .await
            .expect("ingest");
        store
            .ingest(m, 10.0, &[("level", "error")])
            .await
            .expect("ingest");
        store
            .ingest(m, 1.0, &[("level", "info")])
            .await
            .expect("ingest");

        // Filter for NOT error
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "error.count")
            .add_query_param("group_by", "level")
            .add_query_param("filter", "!level:error")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");

        // Should exclude "error", leaving warn and info
        assert_eq!(groups.len(), 2);
        for group in groups {
            let name = group["group"].as_str().unwrap();
            assert_ne!(name, "error");
        }
    }

    #[tokio::test]
    async fn filter_wildcard_expression() {
        let (server, store) = test_server().await;

        let m = metric("app.events");
        store
            .ingest(m, 10.0, &[("service", "db_primary")])
            .await
            .expect("ingest");
        store
            .ingest(m, 20.0, &[("service", "db_replica")])
            .await
            .expect("ingest");
        store
            .ingest(m, 30.0, &[("service", "api_gateway")])
            .await
            .expect("ingest");

        // Filter for services starting with db_
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "app.events")
            .add_query_param("group_by", "service")
            .add_query_param("filter", "service:db_*")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");

        assert_eq!(groups.len(), 2);
        for group in groups {
            let name = group["group"].as_str().unwrap();
            assert!(name.starts_with("db_"), "expected db_* but got {name}");
        }
    }

    #[tokio::test]
    async fn filter_complex_nested_expression() {
        let (server, store) = test_server().await;

        let m = metric("api.latency");
        store
            .ingest(m, 10.0, &[("env", "prod"), ("region", "us")])
            .await
            .expect("ingest");
        store
            .ingest(m, 20.0, &[("env", "prod"), ("region", "eu")])
            .await
            .expect("ingest");
        store
            .ingest(m, 30.0, &[("env", "staging"), ("region", "us")])
            .await
            .expect("ingest");
        store
            .ingest(m, 40.0, &[("env", "staging"), ("region", "eu")])
            .await
            .expect("ingest");

        // Complex: (env:prod OR env:staging) AND region:us
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "api.latency")
            .add_query_param("group_by", "env")
            .add_query_param("filter", "(env:prod OR env:staging) AND region:us")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");

        // Should have prod (10.0) and staging (30.0) both in us region
        assert_eq!(groups.len(), 2);
        for group in groups {
            let name = group["group"].as_str().unwrap();
            let value = group["buckets"][0]["value"].as_f64().unwrap();
            match name {
                "prod" => assert_eq!(value, 10.0),
                "staging" => assert_eq!(value, 30.0),
                _ => panic!("unexpected group: {name}"),
            }
        }
    }

    #[tokio::test]
    async fn filter_all_star() {
        let (server, store) = test_server().await;

        let m = metric("all.metrics");
        store.ingest(m, 1.0, &[("tag", "a")]).await.expect("ingest");
        store.ingest(m, 2.0, &[("tag", "b")]).await.expect("ingest");

        // Explicit * filter
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "all.metrics")
            .add_query_param("group_by", "tag")
            .add_query_param("filter", "*")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");
        assert_eq!(groups.len(), 2);
    }

    // =========================================================================
    // Aggregation type tests
    // =========================================================================

    #[tokio::test]
    async fn aggregation_sum() {
        let (server, store) = test_server().await;

        let m = metric("counter.total");
        let db = store.database();
        // Write multiple values to same series
        db.write_at(m, 1000, 10.0, &[("svc", "a")]).await.unwrap();
        db.write_at(m, 2000, 20.0, &[("svc", "a")]).await.unwrap();
        db.write_at(m, 3000, 30.0, &[("svc", "a")]).await.unwrap();

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "counter.total")
            .add_query_param("group_by", "svc")
            .add_query_param("agg", "sum")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "sum");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 60.0); // 10 + 20 + 30
    }

    #[tokio::test]
    async fn aggregation_avg() {
        let (server, store) = test_server().await;

        let m = metric("latency.avg");
        let db = store.database();
        db.write_at(m, 1000, 10.0, &[("svc", "a")]).await.unwrap();
        db.write_at(m, 2000, 20.0, &[("svc", "a")]).await.unwrap();
        db.write_at(m, 3000, 30.0, &[("svc", "a")]).await.unwrap();
        db.write_at(m, 4000, 40.0, &[("svc", "a")]).await.unwrap();

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "latency.avg")
            .add_query_param("group_by", "svc")
            .add_query_param("agg", "avg")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "avg");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 25.0); // (10 + 20 + 30 + 40) / 4
    }

    #[tokio::test]
    async fn aggregation_min() {
        let (server, store) = test_server().await;

        let m = metric("temp.min");
        let db = store.database();
        db.write_at(m, 1000, 15.0, &[("sensor", "s1")])
            .await
            .unwrap();
        db.write_at(m, 2000, 5.0, &[("sensor", "s1")])
            .await
            .unwrap();
        db.write_at(m, 3000, 25.0, &[("sensor", "s1")])
            .await
            .unwrap();

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "temp.min")
            .add_query_param("group_by", "sensor")
            .add_query_param("agg", "min")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "min");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 5.0);
    }

    #[tokio::test]
    async fn aggregation_max() {
        let (server, store) = test_server().await;

        let m = metric("temp.max");
        let db = store.database();
        db.write_at(m, 1000, 15.0, &[("sensor", "s1")])
            .await
            .unwrap();
        db.write_at(m, 2000, 35.0, &[("sensor", "s1")])
            .await
            .unwrap();
        db.write_at(m, 3000, 25.0, &[("sensor", "s1")])
            .await
            .unwrap();

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "temp.max")
            .add_query_param("group_by", "sensor")
            .add_query_param("agg", "max")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "max");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 35.0);
    }

    #[tokio::test]
    async fn aggregation_count() {
        let (server, store) = test_server().await;

        let m = metric("events.count");
        let db = store.database();
        db.write_at(m, 1000, 1.0, &[("type", "click")])
            .await
            .unwrap();
        db.write_at(m, 2000, 1.0, &[("type", "click")])
            .await
            .unwrap();
        db.write_at(m, 3000, 1.0, &[("type", "click")])
            .await
            .unwrap();
        db.write_at(m, 4000, 1.0, &[("type", "click")])
            .await
            .unwrap();
        db.write_at(m, 5000, 1.0, &[("type", "click")])
            .await
            .unwrap();

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "events.count")
            .add_query_param("group_by", "type")
            .add_query_param("agg", "count")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["agg"], "count");
        let value = body["groups"][0]["buckets"][0]["value"].as_f64().unwrap();
        assert_eq!(value, 5.0);
    }

    // =========================================================================
    // Time range and granularity tests
    // =========================================================================

    #[tokio::test]
    async fn query_with_granularity() {
        let (server, store) = test_server().await;

        let m = metric("requests.per.second");
        let db = store.database();
        // Write data at different timestamps spanning multiple buckets
        // Using 1 second granularity (1_000_000_000 ns)
        let base: u128 = 1_000_000_000_000; // 1000 seconds in ns
        db.write_at(m, base, 1.0, &[("svc", "a")]).await.unwrap();
        db.write_at(m, base + 500_000_000, 2.0, &[("svc", "a")])
            .await
            .unwrap(); // same second
        db.write_at(m, base + 1_000_000_000, 3.0, &[("svc", "a")])
            .await
            .unwrap(); // next second
        db.write_at(m, base + 1_500_000_000, 4.0, &[("svc", "a")])
            .await
            .unwrap(); // same second

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "requests.per.second")
            .add_query_param("group_by", "svc")
            .add_query_param("granularity", "1s")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        // With 1s granularity, we should have 2 buckets
        let buckets = body["groups"][0]["buckets"].as_array().expect("buckets");
        assert_eq!(buckets.len(), 2);
    }

    // =========================================================================
    // Preview endpoint tests
    // =========================================================================

    #[tokio::test]
    async fn preview_endpoint_works() {
        let (server, store) = test_server().await;

        let m = metric("preview.metric");
        store
            .ingest(m, 42.0, &[("tag", "value")])
            .await
            .expect("ingest");

        let response = server
            .get("/api/metrics/preview")
            .add_query_param("metric", "preview.metric")
            .add_query_param("group_by", "tag")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        assert_eq!(body["metric"], "preview.metric");
        assert_eq!(body["group_by"], "tag");
    }

    // =========================================================================
    // Error handling tests
    // =========================================================================

    #[tokio::test]
    async fn invalid_metric_name_returns_error() {
        let (server, _store) = test_server().await;

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "INVALID-NAME!")
            .add_query_param("group_by", "host")
            .await;

        response.assert_status_bad_request();
    }

    #[tokio::test]
    async fn invalid_granularity_returns_error() {
        let (server, _store) = test_server().await;

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "valid.metric")
            .add_query_param("group_by", "host")
            .add_query_param("granularity", "invalid")
            .await;

        response.assert_status_bad_request();
    }

    #[tokio::test]
    async fn empty_result_for_nonexistent_metric() {
        let (server, _store) = test_server().await;

        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "does.not.exist")
            .add_query_param("group_by", "host")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");
        assert!(groups.is_empty());
    }

    #[tokio::test]
    async fn filter_no_match_returns_empty() {
        let (server, store) = test_server().await;

        let m = metric("some.metric");
        store
            .ingest(m, 100.0, &[("env", "prod")])
            .await
            .expect("ingest");

        // Filter that matches nothing
        let response = server
            .get("/api/metrics")
            .add_query_param("metric", "some.metric")
            .add_query_param("group_by", "env")
            .add_query_param("filter", "env:nonexistent")
            .await;

        response.assert_status_ok();
        let body: serde_json::Value = response.json();
        let groups = body["groups"].as_array().expect("groups");
        assert!(groups.is_empty());
    }
}
