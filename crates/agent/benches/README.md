# Agent Benchmarks

## Serialization: JSON vs bitcode

Benchmarks comparing JSON and bitcode serialization for the `/api/metrics/query` endpoint responses.

### Run Benchmarks

```bash
cargo bench -p enya-agent --bench serialization
```

### Results

Tested on Apple M3 Max. Results will vary by hardware.

#### Payload Sizes

| Payload | JSON | bitcode | Compression |
|---------|------|---------|-------------|
| 5 groups × 60 buckets | 20,975 B | 7,932 B | **2.64x smaller** |
| 10 groups × 60 buckets | 41,905 B | 15,765 B | **2.66x smaller** |
| 20 groups × 60 buckets | 84,509 B | 31,447 B | **2.69x smaller** |
| 10 groups × 1440 buckets (24h) | 1.07 MB | 374 KB | **2.86x smaller** |

#### Serialization Throughput (Server Side)

| Payload | JSON | bitcode | Speedup |
|---------|------|---------|---------|
| 5g × 60b | ~575 MiB/s | 5.9 GiB/s | **~10x** |
| 10g × 60b | ~605 MiB/s | 7.5 GiB/s | **~12x** |
| 20g × 60b | ~600 MiB/s | 8.6 GiB/s | **~14x** |
| 10g × 1440b | ~655 MiB/s | 7.7 GiB/s | **~12x** |

#### Deserialization Throughput (Editor Side)

| Payload | JSON | bitcode | Speedup |
|---------|------|---------|---------|
| 5g × 60b | ~346 MiB/s | 16.8 GiB/s | **~50x** |
| 10g × 60b | ~350 MiB/s | 20.1 GiB/s | **~57x** |
| 20g × 60b | ~356 MiB/s | 21.7 GiB/s | **~61x** |
| 10g × 1440b | ~346 MiB/s | 29.2 GiB/s | **~84x** |

### Key Takeaways

- **Deserialization is 50-84x faster** with bitcode - critical for editor responsiveness
- **Larger payloads see bigger gains** - 24-hour queries show 84x faster deserialization
- **Payloads are ~2.7x smaller** - less network bandwidth, faster transfers
- For a 1MB response (typical 24-hour view):
  - bitcode deserializes in **~30µs**
  - JSON deserializes in **~3ms**

### Usage

The `/api/metrics/query` endpoint supports content negotiation via the `Accept` header:

```bash
# JSON (default)
curl "http://localhost:9797/api/metrics/query?metric=cpu.usage&query=sum(*)"

# bitcode binary (fastest)
curl -H "Accept: application/x-bitcode" \
  "http://localhost:9797/api/metrics/query?metric=cpu.usage&query=sum(*)"
```

### Why bitcode?

bitcode is ideal for Rust-to-Rust communication:

1. **Rust-only ecosystem** - Both agent and editor are Rust (native or WASM)
2. **WASM compatible** - Works in browser-based editor builds
3. **Zero-copy friendly** - Minimal allocations during deserialization
4. **Schema evolution** - `Option<T>` fields allow adding new fields without breaking clients

Trade-offs:
- Not human-readable (keep JSON fallback for debugging)
- Requires `bitcode::Encode`/`Decode` derives on types
