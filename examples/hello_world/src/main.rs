#[tokio::main]
async fn main() {
    // Integrates with metrics-rs and tracing opentelemetry.
    enya::serve("0.0.0.0:3002").await;
}
