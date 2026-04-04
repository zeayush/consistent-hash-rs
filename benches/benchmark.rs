use consistent_hash_rs::ConsistentHashRing;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

static ADD_NODE_COUNTS: &[usize] = &[10, 100, 1000];
static GET_NODE_COUNTS: &[usize] = &[10, 100, 10000];

// ── Add benchmarks ────────────────────────────────────────────────────────────

fn bench_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("add");
    group.sample_size(10);
    for &n in ADD_NODE_COUNTS {
        group.bench_with_input(BenchmarkId::new("ring", n), &n, |b, &n| {
            b.iter(|| {
                let ring = ConsistentHashRing::new(150);
                for i in 0..n {
                    ring.add(&format!("node{i}"), 1);
                }
            });
        });
    }
    group.finish();
}

// ── Get benchmarks ────────────────────────────────────────────────────────────

fn bench_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("get");
    for &n in GET_NODE_COUNTS {
        let ring = ConsistentHashRing::new(150);
        for i in 0..n {
            ring.add(&format!("node{i}"), 1);
        }
        group.bench_with_input(BenchmarkId::new("ring", n), &n, |b, _| {
            b.iter(|| ring.get("bench-key"));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_add, bench_get);
criterion_main!(benches);
