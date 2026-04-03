# consistent-hash-rs

[![CI](https://github.com/zeayush/consistent-hash-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/zeayush/consistent-hash-rs/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-green)

Production-grade consistent hashing library for Rust — implements three distinct
algorithms behind a single `HashRouter` trait, tested with the full integration
suite, and benchmarked across algorithm and node-count dimensions with Criterion.

Part of a distributed systems portfolio implementing every system from **Alex
Xu's System Design Interview (Vol. 1 & 2)**. This covers **Chapter 5 —
Design Consistent Hashing**.

---

## What is Consistent Hashing?

In a naive hash ring (`server = hash(key) % N`), adding or removing one server
re-maps almost every key — catastrophic for caches and databases. Consistent
hashing places both servers and keys on a virtual ring so that only
`keys / N` keys move when the cluster changes.

This library adds two more algorithms on top of the baseline:

- **Jump Hash** — O(ln n) lookup, O(n) memory, near-perfect uniformity. No
  vnode table.
- **Bounded Load** — wraps the vnode ring and caps each node at `β × fair
  share` of in-flight requests, eliminating hot spots under skewed key
  distributions.

---

## How it Works

```
          0 ─────────────────────────────── 2³²-1
          │                                   │
     ┌────▼────┐                         ┌────▼────┐
     │ vnode A │ ◄──── key hashes ──────►│ vnode C │
     └────┬────┘    land on nearest      └────┬────┘
          │         vnode clockwise           │
     ┌────▼────┐                         ┌────▼────┐
     │ vnode B │◄────────────────────────│ vnode A │
     └─────────┘                         └─────────┘

  Physical nodes: A, B, C
  Virtual nodes : replicas × weight per node (CRC-32 hashed)
```

**Add node:** insert `replicas × weight` virtual nodes → sort → done.  
**Remove node:** strip its virtual nodes → rebuild sorted vec → done.  
**Lookup:** CRC-32(key) → `binary_search` → first vnode hash ≥ key → physical node.

---

## Algorithms

| | VNode Ring | Jump Hash | Bounded Load |
|---|---|---|---|
| **Lookup** | O(log n) | O(ln n) | O(log n) + load scan |
| **Memory** | O(replicas × n) | O(n) | O(replicas × n) |
| **Uniformity** | Good (hash variance) | Near-perfect | Good |
| **Weighted nodes** | ✅ | ❌ | ✅ |
| **Hot-spot prevention** | ❌ | ❌ | ✅ |
| **Best for** | General routing | Homogeneous backends | Stateful request routing |

---

## Benchmarks

Run on Apple M1 · Rust stable · Criterion 0.5

```sh
cargo bench
```

### Get (single key lookup)

| Algorithm | 3 nodes | 10 nodes | 50 nodes | 100 nodes |
|---|---|---|---|---|
| VNode Ring | ~18 ns | ~22 ns | ~26 ns | ~28 ns |
| Jump Hash | ~9 ns | ~11 ns | ~14 ns | ~16 ns |
| Bounded Load | ~180 ns | ~550 ns | ~5 µs | ~10 µs |

### Add (build ring with N nodes)

| Algorithm | 3 nodes | 10 nodes | 50 nodes | 100 nodes |
|---|---|---|---|---|
| VNode Ring | ~30 µs | ~250 µs | ~6 ms | ~25 ms |
| Jump Hash | ~200 ns | ~700 ns | ~4 µs | ~8 µs |

> Jump Hash add is ~3000× faster — it stores only a sorted `Vec<String>`
> vs `replicas × N` CRC-32'd virtual nodes.

---

## Quick Start

Add to `Cargo.toml`:

```toml
[dependencies]
consistent-hash-rs = { git = "https://github.com/zeayush/consistent-hash-rs" }
```

```rust
use consistent_hash_rs::{ConsistentHashRing, JumpHashRing, BoundedLoadRing, HashRouter};

// --- VNode Ring (general purpose) ---
let ring = ConsistentHashRing::new(150); // 150 virtual nodes per unit weight
ring.add("db1", 2);                      // db1 gets 2× the load of db2
ring.add("db2", 1);
let node = ring.get("user:42");          // deterministic routing

// --- Jump Hash (uniform, stateless) ---
let jring = JumpHashRing::new();
jring.add("cache1", 1);
jring.add("cache2", 1);
let node = jring.get("session:abc");

// --- Bounded Load (hot-spot elimination) ---
let bring = BoundedLoadRing::new(150, 1.25);  // β = 1.25
bring.add("api1", 1);
bring.add("api2", 1);
if let Some(node) = bring.get("req:xyz") {   // increments in-flight counter
    // ... handle request ...
    bring.done(&node);                         // MUST be called when done
}

// --- Swap algorithms via the HashRouter trait ---
let router: Box<dyn HashRouter> = Box::new(ring);
```

---

## API

```rust
pub trait HashRouter: Send + Sync {
    fn add(&self, node: &str, weight: usize) -> bool;
    fn remove(&self, node: &str) -> bool;
    fn get(&self, key: &str) -> Option<String>;
    fn nodes(&self) -> Vec<String>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

// VNode Ring only
impl ConsistentHashRing {
    pub fn get_n(&self, key: &str, n: usize) -> Vec<String>;
}

// Bounded Load only
impl BoundedLoadRing {
    pub fn done(&self, node: &str);
    pub fn loads(&self) -> HashMap<String, i64>;
}
```

---

## Key Design Decisions

| Decision | Rationale |
|---|---|
| **`crc32fast`** | Uses SIMD/hardware-accelerated CRC-32; deterministic across runs; no crypto overhead needed for routing |
| **`RwLock<Inner>`** | Multiple concurrent readers never block each other — critical for read-heavy routing workloads |
| **`Vec<u32>` + `binary_search`** | Stack-friendly sorted array; `binary_search` returns `Err(insertion_point)` which doubles as the successor index |
| **`sorted_keys.dedup()`** | Hash collisions between vnodes are silent; the newer node wins, duplicate position is dropped |
| **Sorted `Vec<String>` in Jump Hash** | Deterministic bucket→name mapping even after add/remove; `partition_point` for O(log n) inserts |
| **`(β × totalLoad / n).ceil().max(1)`** | Threshold of 1 prevents starvation at startup when `totalLoad == 0` |

---

## Running Tests

```sh
cargo test
```

29 tests covering: empty ring, single-node routing, add/remove idempotency,
zero-weight rejection, weighted proportional distribution, key stability on node
join, wrap-around, concurrent reads and writes, jump hash uniformity, bounded load
ceiling, `get_n` replica placement, and `HashRouter` trait object safety.

---

## Portfolio

This is one implementation in a series covering every system from Alex Xu's
*System Design Interview* Vol. 1 & 2.

| # | System | Rust | Go |
|---|---|---|---|
| 1 | Consistent Hashing | **this repo** | [consistent-hashing-go](https://github.com/zeayush/consistent-hashing-go) |
| … | *more coming* | | |

