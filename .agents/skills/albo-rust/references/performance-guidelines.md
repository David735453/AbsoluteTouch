# Performance Guidelines

## M-HOTPATH

**Category:** Identify, Profile, Optimize the Hot Path Early

Early in development, identify performance-critical code and create benchmarks.

### Actions

- Identify hot paths and create benchmarks
- Regularly run profiler for CPU and allocation insights
- Document performance-sensitive areas

### Recommended Tools

- **Benchmarking**: criterion, divan
- **Profiling**: Intel VTune, Superluminal (Windows)

### Cargo.toml Setup

```toml
[profile.bench]
debug = 1  # Enable debug symbols for profiling
```

### Common Performance Issues

| Issue | Impact | Solution |
|-------|--------|----------|
| String re-allocations | 15-50% slowdown | Pre-allocate, use `with_capacity` |
| Short-lived allocations | Memory pressure | Bump allocation, arena |
| Cloning large data | CPU + memory | Use references, `Cow`, `Arc` |
| Repeated re-hashing | CPU | Cache hash values |
| Default hasher | CPU | Use `ahash` or `rustc-hash` |

### Benchmark Example

```rust
use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_process(c: &mut Criterion) {
    let data = generate_test_data();

    c.bench_function("process_data", |b| {
        b.iter(|| process_data(&data))
    });
}

criterion_group!(benches, benchmark_process);
criterion_main!(benches);
```

---

## M-THROUGHPUT

**Category:** Optimize for Throughput, Avoid Empty Cycles

Optimize libraries for throughput measured in items per CPU cycle.

### Do

- Partition work into reasonable chunks
- Let threads/tasks handle work independently
- Sleep/yield when no work exists
- Design APIs for batched operations
- Exploit CPU cache and locality

### Don't

- Hot spin for individual items
- Process items individually when batching possible
- Use work stealing for individual items

### Principle

Shared state overhead should be less than recomputation cost.

### Batching Example

```rust
// Bad: Process one at a time
for item in items {
    channel.send(item).await;
}

// Good: Batch operations
let batch: Vec<_> = items.iter().take(BATCH_SIZE).collect();
channel.send_batch(&batch).await;
```

---

## M-YIELD-POINTS

**Category:** Long-Running Tasks Should Have Yield Points

Long-running computations must contain `yield_now().await` points.

### I/O-Bound Tasks

```rust
async fn process_items(items: &[Item]) {
    for item in items {
        // Awaiting I/O provides natural yield point
        read_item(item).await;
    }
}
```

### CPU-Bound Tasks

```rust
use tokio::task::yield_now;

async fn decompress_items(zip_file: &File) {
    let items = zip_file.read().await;
    for item in items {
        decompress(item);  // CPU-bound work
        yield_now().await; // Cooperative yielding
    }
}
```

### Batched Yielding

```rust
async fn process_many(items: &[Item]) {
    const BATCH_SIZE: usize = 100;

    for (i, item) in items.iter().enumerate() {
        process(item);

        // Yield every BATCH_SIZE items
        if i % BATCH_SIZE == 0 {
            yield_now().await;
        }
    }
}
```

### Recommendation

Yield every 10-100μs of CPU-bound work to balance switching overhead against fairness.

### Timing Consideration

```rust
use std::time::Instant;

async fn cpu_intensive_work(data: &[Data]) {
    let mut last_yield = Instant::now();

    for item in data {
        process(item);

        // Yield if more than 50μs elapsed
        if last_yield.elapsed().as_micros() > 50 {
            yield_now().await;
            last_yield = Instant::now();
        }
    }
}
```
