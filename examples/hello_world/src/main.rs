use metrics::{counter, gauge, histogram};
use std::time::Duration;
use tokio::time::interval;

const ADDR: &str = "127.0.0.1:3002";

#[tokio::main]
async fn main() {
    println!(
        "Starting Enya hello_world on http://{ADDR}\n\
         - Metrics are emitted every second for two fake endpoints.\n\
         - Preview the aggregated data via /api/metrics/preview (the example also logs it)."
    );

    spawn_example_metrics();
    spawn_preview_logger();

    enya::serve(ADDR).await;
}

fn spawn_example_metrics() {
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(1));
        let endpoints = ["/api/demo", "/api/metrics"];

        loop {
            tick.tick().await;
            for endpoint in &endpoints {
                counter!("hello_world.requests", "endpoint" => *endpoint).increment(1);
                gauge!("hello_world.inflight", "endpoint" => *endpoint)
                    .set(fastrand::i32(0..5) as f64);
                histogram!("hello_world.latency_ms", "endpoint" => *endpoint)
                    .record(fastrand::f32() * 100.0);
            }
        }
    });
}

fn spawn_preview_logger() {
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let url = format!(
            "http://{ADDR}/api/metrics/preview?metric=hello_world.requests&group_by=endpoint"
        );
        let mut tick = interval(Duration::from_secs(5));

        loop {
            tick.tick().await;
            match client.get(&url).send().await {
                Ok(resp) => match resp.json::<serde_json::Value>().await {
                    Ok(json) => println!("[metrics preview] {json}"),
                    Err(err) => eprintln!("Failed to decode metrics preview: {err}"),
                },
                Err(err) => eprintln!("Failed to fetch metrics preview: {err}"),
            }
        }
    });
}
