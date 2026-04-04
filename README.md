# consistent-hash-rs

[![CI](https://github.com/zeayush/consistent-hash-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/zeayush/consistent-hash-rs/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-green)

Consistent hashing in Rust — virtual-node ring with weighted nodes, O(log n)
lookup, automatic rebalancing, and thread-safe reads.

Part of a distributed systems portfolio implementing every system from **Alex
Xu's System Design Interview (Vol. 1 & 2)**. This covers **Chapter 5 —
Design Consistent Hashing**.

---

## What is Consistent Hashing?

With a naive hash ring (`server = hash(key) % N`), adding or removing one
server remaps almost every key. Consistent hashing fixes this: both servers
and keys are hashed onto the same ring, so only `keys / N` keys need to move
when the topology changes.

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

  Physical nodes : A, B, C
  Virtual nodes  : replicas × weight per node  (CRC-32 hashed)
```

- **Add:** hash `replicas × weight` vnode IDs → insert into sorted `Vec<u32>`
- **Remove:** strip the node's vnodes → rebuild sorted vec
- **Lookup:** `CRC-32(key)` → `binary_search` → first vnode hash ≥ key → physical node

---

## Quick Start

```toml
[dependencies]
consistent-hash-rs = { git = "https://github.com/zeayush/consistent-hash-rs" }
```

```rust
use consistent_hash_rs::ConsistentHashRing;

let ring = ConsistentHashRing::new(150); // 150 virtual nodes per unit weight

ring.add("db1", 2); // db1 gets 2x the ring space of db2
ring.add("db2", 1);

let node = ring.get("user:42"); // Some("db1") or Some("db2")

ring.remove("db1");             // only the affected key slice migrates
```

---

## API

```rust
impl ConsistentHashRing {
    pub fn new(replicas: usize) -> Self;
    pub fn add(&self, node: &str, weight: usize) -> bool;
    pub fn remove(&self, node: &str) -> bool;
    pub fn get(&self, key: &str) -> Option<String>;
    pub fn nodes(&self) -> Vec<String>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

`add` returns `false` for duplicates or zero weight. `remove` returns `false`
if the node is not present. Both are safe to call concurrently.

---

## Benchmarks

Run on Apple M1 · Rust stable · Criterion 0.5

```sh
cargo bench
```

| Operation | 10 nodes | 100 nodes | 1,000 nodes |
|---|---|---|---|
| `get` (O(log n)) | ~73 ns | ~81 ns | ~91 ns* |
| `add` (build ring) | ~263 µs | ~9.3 ms | ~820 ms |

\* `get` measured at 10,000 nodes; binary-search growth is sub-linear so
1,000 and 10,000 nodes produce virtually the same latency (~91 ns).

`get` scales O(log n) — binary search on the sorted vnode array.
`add` cost is dominated by `replicas × N` CRC-32 computations and the re-sort.

---

## Key Design Decisions

| Decision | Rationale |
|---|---|
| **`crc32fast`** | SIMD/hardware-accelerated CRC-32; deterministic across runs; no crypto overhead |
| **`RwLock<Inner>`** | Concurrent readers never block each other — critical for read-heavy routing |
| **`Vec<u32>` + `binary_search`** | `binary_search` returns `Err(insertion_point)` which is directly the successor index |
| **`sorted_keys.dedup()`** | Hash collisions between vnodes are silent; newer node wins, duplicate dropped |

---

## Tests

```sh
cargo test
```

10 integration tests: empty ring, single-node routing, add/remove idempotency,
remove nonexistent, zero-weight rejection, weighted proportional distribution,
key stability on node join, wrap-around, zero replicas panic, concurrent reads
and writes.

---