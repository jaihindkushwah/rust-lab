# Mercury Ultra-Low Latency Rust HFT Engine

Mercury is a high-frequency trading (HFT) simulation engine built in Rust, designed for microsecond-optimized processing. It parses market data, maintains order book states, evaluates strategy rules, runs pre-trade risk checks, and dispatches orders to an exchange simulator in a multi-threaded, pipelined architecture.

## Architecture

The system uses a **3-thread pipeline** connected by lock-free Single-Producer Single-Consumer (SPSC) ring buffers, with threads pinned to dedicated CPU cores via core affinity:

```
[Ingress Thread] --(RingBuffer1: IngressPacket)--> [Parser & Order Book Thread] --(RingBuffer2: BookUpdate)--> [Strategy & Risk Thread]
```

1. **Ingress Thread (Thread 1)**: Simulated network packet receiver, copying raw messages into a pre-allocated stack packet array and stamping the ingress CPU Time Stamp Counter (TSC) timestamp.
2. **Parser & Order Book Thread (Thread 2)**: Parses FIX messages in-place, updates the cache-aligned order book, and extracts the best bid and ask prices.
3. **Strategy & Risk Thread (Thread 3)**: Evaluates the market opportunity using fixed-point skewed market-making logic and executes pre-trade risk checks.

---

## Latency Optimization Techniques

- **Core Affinity & Isolation**: Critical threads are pinned to dedicated CPU cores to eliminate context switching overhead.
- **Lock-Free Communication**: Multi-threaded message passing is handled using custom Single-Producer Single-Consumer (SPSC) ring buffers.
- **Cache-Line Alignment**: Data structures like the `OrderBook` are aligned to 64 bytes (`#[repr(align(64))]`) to fit CPU cache lines and prevent false sharing.
- **Zero Allocations in the Hot Path**: All message buffers, price levels, and packets are pre-allocated at startup. No heap allocation occurs during loop execution.
- **Single-Pass Parser**: The FIX parser scans data in a single pass and converts tags directly to integers (`55`, `44`, etc.) rather than performing slice comparisons.
- **Fixed-Point Arithmetic**: The strategy uses fixed-point math with a scaling factor of `10,000` (4 decimal places) and bit-shifting to avoid floating-point operations.

---

## Real-Time Pre-Trade Risk Controls

The **PreTradeRiskEngine** executes checks in nanoseconds before order dispatch:
1. **Global Software Kill Switch**: Instantly rejects all orders if triggered.
2. **Order Size Limit**: Rejects orders exceeding `1,000` shares.
3. **Position Limit**: Rejects orders that would cause the net position to exceed `10,000` shares.
4. **Notional Exposure Limit**: Rejects orders exceeding a cumulative exposure of **$5M** (`50_000_000_000` scaled).
5. **Price Reasonability**: Rejects orders where the target price deviates by more than **2% (200 bps)** from the current mid-price.
6. **Duplicate Order Detection**: Rejects identical orders (same side, price, qty) sent within **1 second** using the calibrated TSC frequency.

---

## How to Run

Since the engine relies on low-level Linux/POSIX APIs for affinity and timing, compilation and execution should be done in a WSL (Linux) environment.

### Prerequisites
Ensure Rust and Cargo are installed in your WSL environment.

### Run Unit Tests
To verify all unit tests (order book sorting, parser validation, SPSC FIFO):
```bash
cargo test
```

### Run Latency & Throughput Benchmark
To compile with maximum optimizations and run the benchmark pipeline (1,000,000 messages) and Chaos Engineering tests:
```bash
cargo run --release
```
