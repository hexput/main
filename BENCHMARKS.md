# Hexput Performance Benchmarks

This document contains comprehensive performance benchmarks for the Hexput runtime and RPC system.

**Test Environment:**
- OS: Linux
- Date: January 1, 2026
- Rust: Release mode (optimized)
- CPU: System dependent

---

## Runtime Benchmarks

### Parse Performance

| Benchmark | Time | Description |
|-----------|------|-------------|
| `parse_simple` | ~1.17 µs | Parse simple arithmetic expression |

### Non-Cached Execution (Parse + Execute)

These benchmarks include both parsing and execution time, representing the cost of running code without AST caching:

| Benchmark | Time | Description |
|-----------|------|-------------|
| `simple_arithmetic` | ~1.73 µs | Parse + execute simple addition |
| `loop_sum` | ~6.23 µs | Parse + execute loop summing 10 numbers |
| `complex_nested` | ~6.73 µs | Parse + execute nested object/array operations |
| `with_callbacks` | ~6.22 µs | Parse + execute with callback functions |

### Cached Execution (Execute Only)

These benchmarks use pre-parsed AST, showing the raw execution performance:

| Benchmark | Time | Speedup vs Non-Cached |
|-----------|------|----------------------|
| `simple_arithmetic` | ~244 ns | **~7.1x faster** |
| `loop_sum` | ~2.30 µs | **~2.7x faster** |
| `complex_nested` | ~1.05 µs | **~6.4x faster** |
| `with_callbacks` | ~1.09 µs | **~5.7x faster** |

### Parse vs Execute Breakdown

Analysis of time spent in parsing vs execution:

| Operation | Time | Percentage |
|-----------|------|------------|
| Parse only | ~3.50 µs | ~54% of total |
| Execute only | ~2.27 µs | ~36% of total |
| Parse + Execute | ~6.53 µs | 100% |

**Key Insight:** Parsing accounts for more than half the execution time, making AST caching highly beneficial.

### Execution Scaling

Performance comparison showing how cached vs non-cached execution scales with script complexity:

| Array Size | Cached Time | Non-Cached Time | Speedup |
|------------|-------------|-----------------|---------|
| 10 items | ~2.27 µs | ~6.14 µs | **2.7x** |
| 50 items | ~12.2 µs | ~17.8 µs | **1.5x** |
| 100 items | ~19.7 µs | ~36.9 µs | **1.9x** |

**Analysis:**
- Small scripts: 2-3x speedup from caching
- Medium scripts: 1.5-2x speedup from caching
- The speedup varies based on the ratio of parsing time to execution time
- Longer-running scripts benefit less from AST caching (execution dominates)

---

## Transport Benchmarks

### WebSocket Transport

WebSocket benchmarks measure end-to-end RPC performance over TCP:

| Benchmark | Typical Time | Description |
|-----------|--------------|-------------|
| `ws_basic_execution` | ~5-10 ms | Simple script execution via WebSocket |
| `ws_cached_execution` | ~8-15 ms | Code registration + cached execution |
| `ws_function_registration` | ~3-5 ms | Register a remote function |
| `ws_complex_execution` | ~8-12 ms | Complex nested operations via WebSocket |
| `ws_remote_function_call` | ~10-15 ms | Register function + call it remotely |

**Note:** WebSocket benchmarks include:
- Network overhead (TCP connection, handshake)
- JSON serialization/deserialization
- Message framing
- Server-side processing

### Remote Function Call Overhead

The `ws_remote_function_call` benchmark measures the complete workflow:
1. Register a remote function in a context
2. Execute a script that calls the registered function
3. Handle the RPC round-trip when function is not locally defined

This represents the typical pattern for extending the language with host-provided functions.

### Unix Domain Socket Transport (Linux/macOS)

Unix socket benchmarks show local IPC performance:

| Benchmark | Typical Time | Speedup vs WebSocket |
|-----------|--------------|---------------------|
| `unix_basic_execution` | ~3-6 ms | **~1.5-2x faster** |
| `unix_cached_execution` | ~5-10 ms | **~1.5x faster** |
| `unix_complex_execution` | ~5-8 ms | **~1.5x faster** |
| `unix_remote_function_call` | ~6-10 ms | **~1.5-2x faster** |

**Advantages of Unix Sockets:**
- Lower latency (no TCP overhead)
- Better for local IPC
- Filesystem-based access control
- No network stack involvement

---

## Key Performance Insights

### 1. AST Caching is Highly Effective

- **7x speedup** for simple scripts
- **2-6x speedup** for typical workloads
- Parsing represents 50-60% of total time
- **Recommendation:** Always use code registration + cached execution for repeated scripts

### 2. Execution Performance Characteristics

- Simple arithmetic: ~244 ns (cached)
- Loop over 10 items: ~2.3 µs (cached)
- Complex nested operations: ~1 µs (cached)
- Callback invocations: minimal overhead (~1 µs for 5 calls)

### 3. Transport Layer Overhead

- WebSocket: 5-10 ms round-trip for simple operations
- Unix Socket: 3-6 ms round-trip (1.5-2x faster than WebSocket)
- Remote function call: Adds 2-5 ms overhead for registration + RPC
- Transport choice matters more for many small requests
- Batching or long-running scripts amortize transport overhead

### 4. Remote Function Call Performance

- Function registration: ~3-5 ms (WebSocket), ~2-3 ms (Unix)
- Complete register + call workflow: ~10-15 ms (WebSocket), ~6-10 ms (Unix)
- RPC overhead per remote call: ~5-8 ms
- **Recommendation:** Minimize remote function calls; batch operations when possible

### 4. Scalability Characteristics

- Linear scaling with script complexity
- Parallel execution possible (separate contexts)
- No shared state between contexts = good concurrency
- Transport layer handles multiple concurrent clients efficiently

---

## Best Practices Based on Benchmarks

### For Performance-Critical Applications

1. **Use Code Registration**
   - Register scripts once, execute many times
   - Saves ~60% execution time by avoiding re-parsing

2. **Choose Appropriate Transport**
   - Use Unix sockets for local IPC (1.5-2x faster)
   - Use WebSocket for remote connections or browser clients
   - Consider Named Pipes on Windows

3. **Batch Operations**
   - Transport overhead (3-10ms) dominates for small scripts
   - Group related operations when possible
   - Use global variables to parameterize cached code
   - Minimize remote function calls by doing more work in scripts

4. **Remote Function Design**
   - Keep remote functions coarse-grained
   - Prefer passing data over making multiple calls
   - Consider caching function results on the client side
   - Use local callbacks for fine-grained operations

4. **Script Optimization**
   - Simple operations are very fast (<1 µs)
   - Loops scale linearly (~200 ns per iteration)
   - Callbacks have minimal overhead

### For Development

1. **Non-Cached Execution is Fine**
   - ~6 µs total time for typical scripts
   - Acceptable for development/testing
   - Simpler workflow (no registration needed)

2. **Transport Choice**
   - WebSocket is easier to debug
   - Works across networks
   - Good default choice

---

## Benchmark Methodology

### Runtime Benchmarks
- **Tool:** Criterion.rs
- **Iterations:** 100+ per benchmark (with warm-up)
- **Method:** Statistical analysis with outlier detection
- **Variance:** Reported in detailed output

### Transport Benchmarks
- **Setup:** Full server + client round-trip
- **Method:** Async Tokio runtime
- **Measurement:** End-to-end time (send + process + receive)
- **Network:** Localhost only (minimal network effects)

---

## Running Benchmarks

### Runtime Benchmarks
```bash
cargo bench --bench runtime_bench
```

### Transport Benchmarks
```bash
cargo bench --bench transport_bench
```

### All Benchmarks
```bash
cargo bench
```

---

## Future Improvements

Potential optimizations identified through benchmarking:

1. **Parser Optimization**
   - Consider zero-copy parsing
   - Optimize token scanning
   - Reduce allocations during parsing

2. **VM Optimizations**
   - Add bytecode compilation
   - Implement JIT for hot paths
   - Optimize value cloning

3. **Transport Optimizations**
   - Binary protocol (vs JSON)
   - Message batching
   - Connection pooling

4. **Caching Strategies**
   - LRU cache for parsed code
   - Shared AST across contexts
   - Pre-compilation of common patterns

---

*Last updated: January 1, 2026*
