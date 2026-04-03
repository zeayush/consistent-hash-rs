use consistent_hash_rs::{BoundedLoadRing, ConsistentHashRing, JumpHashRing};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

static NODE_COUNTS: &[usize] = &[3, 10, 50, 100];

// ── Add benchmarks ────────────────────────────────────────────────────────────

fn bench_add(c: &mut Criterion) {
    let mut group = c.benchmark_group("add");
    for &n in NODE_COUNTS {
        group.bench_with_input(BenchmarkId::new("vnodes", n), &n, |b, &n| {
            b.iter(|| {
                let ring = ConsistentHashRing::new(100);
                for i in 0..n {
                    ring.add(&format!("node{i}"), 1);
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("jump", n), &n, |b, &n| {
            b.iter(|| {
                let ring = JumpHashRing::new();
                for i in 0..n {
                    ring.add(&format!("node{i}"), 1);
                }
            });
        });
        group.bench_with_input(BenchmarkId::new("bounded", n), &n, |b, &n| {
            b.iter(|| {
                let ring = BoundedLoadRing::new(100, 1.25);
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
    for &n in NODE_COUNTS {
        let vring = ConsistentHashRing::new(100);
        for i in 0..n {
            vring.add(&format!("node{i}"), 1);
        }
        group.bench_with_input(BenchmarkId::new("vnodes", n), &n, |b, _| {
            b.iter(|| vring.get("bench-key"));
        });

        let jring = JumpHashRing::new();
        for i in 0..n {
            jring.add(&format!("node{i}"), 1);
        }
        group.bench_with_input(BenchmarkId::new("jump", n), &n, |b, _| {
            b.iter(|| jring.get("bench-key"));
        });

        let bring = BoundedLoadRing::new(100, 1.25);
        for i in 0..n {
            bring.add(&format!("node{i}"), 1);
        }
        group.bench_with_input(BenchmarkId::new("bounded", n), &n, |b, _| {
            b.iter(|| {
                if let Some(node) = bring.get("bench-key") {
                    bring.done(&node);
                }
            });
        });
    }
    group.finish();
}

// ── GetN (replica placement) benchmark ───────────────────────────────────────

fn bench_get_n(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_n");
    for &n in NODE_COUNTS {
        let ring = ConsistentHashRing::new(100);
        for i in 0..n {
            ring.add(&format!("node{i}"), 1);
        }
        let k = (n / 3).max(1);
        group.bench_with_input(BenchmarkId::new("vnodes", n), &n, |b, _| {
            b.iter(|| ring.get_n("bench-key", k));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_add, bench_get, bench_get_n);
criterion_main!(benches);
