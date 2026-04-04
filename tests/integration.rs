use consistent_hash_rs::ConsistentHashRing;
use std::collections::HashMap;

#[test]
fn empty_ring_returns_none() {
    let ring = ConsistentHashRing::new(100);
    assert!(ring.is_empty());
    assert_eq!(ring.len(), 0);
    assert!(ring.get("any-key").is_none());
}

#[test]
fn single_node_always_selected() {
    let ring = ConsistentHashRing::new(100);
    ring.add("node-a", 1);
    assert_eq!(ring.len(), 1);

    for i in 0..200 {
        let key = format!("key-{i}");
        assert_eq!(ring.get(&key).unwrap(), "node-a");
    }
}

#[test]
fn add_then_remove_restores_state() {
    let ring = ConsistentHashRing::new(100);
    ring.add("node-a", 1);
    ring.add("node-b", 1);
    assert_eq!(ring.len(), 2);

    // Capture assignments with both nodes.
    let keys: Vec<String> = (0..500).map(|i| format!("key-{i}")).collect();
    let before: Vec<Option<String>> = keys.iter().map(|k| ring.get(k)).collect();

    // Remove node-b.
    assert!(ring.remove("node-b"));
    assert_eq!(ring.len(), 1);

    // All keys must now land on node-a.
    for k in &keys {
        assert_eq!(ring.get(k).unwrap(), "node-a");
    }

    // Re-add node-b with same weight → assignments must match the original snapshot.
    ring.add("node-b", 1);
    let after: Vec<Option<String>> = keys.iter().map(|k| ring.get(k)).collect();
    assert_eq!(before, after, "re-adding same node should restore routing");
}

#[test]
fn remove_nonexistent_is_noop() {
    let ring = ConsistentHashRing::new(50);
    assert!(!ring.remove("ghost"));
}

#[test]
fn zero_weight_is_rejected() {
    let ring = ConsistentHashRing::new(50);
    assert!(!ring.add("node-a", 0));
    assert!(ring.is_empty());
}

#[test]
fn weighted_distribution_is_roughly_proportional() {
    let ring = ConsistentHashRing::new(200);
    ring.add("heavy", 3);
    ring.add("light", 1);

    let mut counts: HashMap<String, usize> = HashMap::new();
    let total = 10_000;
    for i in 0..total {
        let key = format!("item-{i}");
        *counts.entry(ring.get(&key).unwrap()).or_default() += 1;
    }

    let heavy = *counts.get("heavy").unwrap_or(&0) as f64;
    let light = *counts.get("light").unwrap_or(&0) as f64;
    let ratio = heavy / light;

    // Expect ratio close to 3:1 (allow 1.5–5.0 for hash variance).
    assert!(
        (1.5..=5.0).contains(&ratio),
        "expected ratio ~3.0, got {ratio:.2} (heavy={heavy}, light={light})"
    );
}

#[test]
fn stability_after_adding_new_node() {
    // After adding a third node, the majority of existing key assignments
    // should remain unchanged (only ~1/N of keys should migrate).
    let ring = ConsistentHashRing::new(150);
    ring.add("A", 1);
    ring.add("B", 1);

    let keys: Vec<String> = (0..5000).map(|i| format!("k-{i}")).collect();
    let before: Vec<String> = keys.iter().map(|k| ring.get(k).unwrap()).collect();

    ring.add("C", 1);
    let after: Vec<String> = keys.iter().map(|k| ring.get(k).unwrap()).collect();

    let moved: usize = before
        .iter()
        .zip(after.iter())
        .filter(|(b, a)| b != a)
        .count();

    let moved_pct = (moved as f64) / (keys.len() as f64) * 100.0;
    // Ideal migration is ~33%. Allow up to 55% for hash variance.
    assert!(
        moved_pct < 55.0,
        "too many keys migrated: {moved}/{} ({moved_pct:.1}%)",
        keys.len()
    );
}

#[test]
fn wrap_around_works() {
    // Ensures that a key whose hash exceeds all vnode hashes still maps
    // to a node (wraps to index 0).  We cannot engineer the exact hash
    // but the single-node test already covers this implicitly; this test
    // adds a deterministic sanity check with two nodes.
    let ring = ConsistentHashRing::new(3); // few vnodes to make wrap-around likely
    ring.add("alpha", 1);
    ring.add("beta", 1);

    // Every key must resolve to some node.
    for i in 0..1000 {
        assert!(ring.get(&format!("wrap-{i}")).is_some());
    }
}

#[test]
#[should_panic(expected = "replicas must be > 0")]
fn zero_replicas_panics() {
    let _ = ConsistentHashRing::new(0);
}

#[test]
fn concurrent_reads_and_writes() {
    use std::sync::Arc;
    use std::thread;

    let ring = Arc::new(ConsistentHashRing::new(100));

    // Writer thread
    let ring_w = Arc::clone(&ring);
    let writer = thread::spawn(move || {
        for i in 0..50 {
            ring_w.add(&format!("node-{i}"), 1);
        }
        for i in 0..25 {
            ring_w.remove(&format!("node-{i}"));
        }
    });

    // Reader threads
    let readers: Vec<_> = (0..4)
        .map(|t| {
            let ring_r = Arc::clone(&ring);
            thread::spawn(move || {
                for i in 0..500 {
                    let _ = ring_r.get(&format!("t{t}-key-{i}"));
                }
            })
        })
        .collect();

    writer.join().unwrap();
    for r in readers {
        r.join().unwrap();
    }

    // After all writers finish, ring should have 25 nodes left.
    assert_eq!(ring.len(), 25);
}
