use consistent_hash_rs::ConsistentHashRing;
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_add_node(c: &mut Criterion) {
    c.bench_function("add_node", |b| {
        b.iter(|| {
            let ring = ConsistentHashRing::new(100);
            ring.add("node1", 1);
            ring.add("node2", 1);
            ring.add("node3", 1);
        });
    });
}

fn bench_get_node(c: &mut Criterion) {
    let ring = ConsistentHashRing::new(100);
    ring.add("node1", 1);
    ring.add("node2", 1);
    ring.add("node3", 1);

    c.bench_function("get_node", |b| {
        b.iter(|| {
            ring.get("some-key");
        });
    });
}

criterion_group!(benches, bench_add_node, bench_get_node);
criterion_main!(benches);
