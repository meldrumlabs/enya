<p align="center">
  <img width="300" height="300" src="assets/logo.png">
</p>

![ci](https://github.com/meldrumlabs/enya/actions/workflows/ci.yml/badge.svg)
[![unsafe forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/uwheel/uwheel/blob/main/LICENSE-APACHE)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/uwheel/uwheel/blob/main/LICENSE-MIT)

# Enya

Your trusted companion for building and running data systems.

```rust
// Vision - Plug and play observability for your application.
#[tokio::main]
async fn main() -> Result<()> {

    // Integrates with metrics-rs and tracing
    enya::init();

    Ok(())
}
```

- Metrics compatible with Prometheus (metrics-rs)
- Logs compatible with Opentelemetry (tracing-subscriber)
- Memory profiling (rust-jemalloc-pprof)
- CPU Flamegraph Profiling (?)
- Git-awareness - Track things over commits
  - suitable for statefulsets +

**Other SDKs**

- DataFusion Customized  (More customized tracking helping teams)
  - Enable enya feature in DataFusion and make it integrate
- Deterministic Simulation Testing (Visualization)
  - egui graphs or step visualizer
- AI Agents ???

## Feature Flags

- `cpu`
  - Enables CPU profiling using pprof
- `jemalloc`
  - Enables memory profiling using pprof through rust-jemalloc-pprof
  - NOTE: this features assumes you are using tikv-jemallocator in your project

## Acknowledgements

- Enya takes inspiration from rerun.io for its egui-based UI.
- Enya's metrics store is inspired by [talna](https://github.com/marvin-j97/talna), a time-series LSM storage made by [marvin-j97](https://github.com/marvin-j97), and uses [SlateDB](https://github.com/slatedb/slatedb) for object storage.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
