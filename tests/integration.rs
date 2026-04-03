use consistent_hash_rs::{BoundedLoadRing, ConsistentHashRing, HashRouter, JumpHashRing};
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

// ── get_n tests ───────────────────────────────────────────────────────────────

#[test]
fn get_n_returns_distinct_ordered_candidates() {
    let ring = ConsistentHashRing::new(100);
    ring.add("A", 1);
    ring.add("B", 1);
    ring.add("C", 1);

    let candidates = ring.get_n("some-key", 3);
    assert_eq!(candidates.len(), 3);
    let mut seen = std::collections::HashSet::new();
    for c in &candidates {
        assert!(seen.insert(c.clone()), "duplicate candidate: {c}");
    }
}

#[test]
fn get_n_clamps_to_node_count() {
    let ring = ConsistentHashRing::new(100);
    ring.add("A", 1);
    ring.add("B", 1);
    // Request more than available — should return all 2 distinct nodes.
    let candidates = ring.get_n("key", 10);
    assert_eq!(candidates.len(), 2);
}

#[test]
fn get_n_empty_ring_returns_empty() {
    let ring = ConsistentHashRing::new(100);
    assert!(ring.get_n("key", 3).is_empty());
}

// ── JumpHashRing tests ────────────────────────────────────────────────────────

#[test]
fn jump_empty_ring_returns_none() {
    let ring = JumpHashRing::new();
    assert!(ring.is_empty());
    assert!(ring.get("key").is_none());
}

#[test]
fn jump_single_node_always_selected() {
    let ring = JumpHashRing::new();
    ring.add("node-a", 1);
    for i in 0..200 {
        assert_eq!(ring.get(&format!("key-{i}")).unwrap(), "node-a");
    }
}

#[test]
fn jump_duplicate_add_is_noop() {
    let ring = JumpHashRing::new();
    assert!(ring.add("node-a", 1));
    assert!(!ring.add("node-a", 1));
    assert_eq!(ring.len(), 1);
}

#[test]
fn jump_remove_nonexistent_is_noop() {
    let ring = JumpHashRing::new();
    assert!(!ring.remove("ghost"));
}

#[test]
fn jump_zero_weight_rejected() {
    let ring = JumpHashRing::new();
    assert!(!ring.add("node-a", 0));
    assert!(ring.is_empty());
}

#[test]
fn jump_uniform_distribution() {
    let ring = JumpHashRing::new();
    ring.add("A", 1);
    ring.add("B", 1);
    ring.add("C", 1);

    let mut counts = HashMap::new();
    for i in 0..9000 {
        let node = ring.get(&format!("key-{i}")).unwrap();
        *counts.entry(node).or_insert(0usize) += 1;
    }
    let expected = 3000.0f64;
    for node in ["A", "B", "C"] {
        let got = *counts.get(node).unwrap_or(&0) as f64;
        assert!(
            got >= expected * 0.85 && got <= expected * 1.15,
            "node {node}: expected ~{expected}, got {got}"
        );
    }
}

#[test]
fn jump_remove_and_reroute() {
    let ring = JumpHashRing::new();
    ring.add("A", 1);
    ring.add("B", 1);
    ring.remove("B");
    for i in 0..100 {
        assert_eq!(ring.get(&format!("key-{i}")).unwrap(), "A");
    }
}

#[test]
fn jump_stability_on_add() {
    let ring = JumpHashRing::new();
    ring.add("A", 1);
    ring.add("B", 1);

    let keys: Vec<String> = (0..5000).map(|i| format!("k-{i}")).collect();
    let before: Vec<String> = keys.iter().map(|k| ring.get(k).unwrap()).collect();

    ring.add("C", 1);
    let moved = keys
        .iter()
        .zip(before.iter())
        .filter(|(k, b)| ring.get(k).unwrap() != **b)
        .count();

    let pct = moved as f64 / keys.len() as f64 * 100.0;
    assert!(pct < 55.0, "too many keys migrated: {moved}/{} ({pct:.1}%)", keys.len());
}

// ── HashRouter trait tests ────────────────────────────────────────────────────

#[test]
fn hash_router_trait_is_object_safe() {
    // Verify all three types work behind a trait object.
    let routers: Vec<Box<dyn HashRouter>> = vec![
        Box::new(ConsistentHashRing::new(100)),
        Box::new(JumpHashRing::new()),
        Box::new(BoundedLoadRing::new(100, 1.25)),
    ];
    for router in &routers {
        router.add("node-a", 1);
        assert_eq!(router.len(), 1);
        assert!(router.get("key").is_some());
    }
}

// ── BoundedLoadRing tests ─────────────────────────────────────────────────────

#[test]
fn bounded_empty_ring_returns_none() {
    let ring = BoundedLoadRing::new(100, 1.25);
    assert!(ring.is_empty());
    assert!(ring.get("key").is_none());
}

#[test]
fn bounded_single_node() {
    let ring = BoundedLoadRing::new(100, 1.25);
    ring.add("node-a", 1);
    for i in 0..10 {
        let node = ring.get(&format!("key-{i}")).unwrap();
        assert_eq!(node, "node-a");
        ring.done(&node);
    }
}

#[test]
fn bounded_zero_weight_rejected() {
    let ring = BoundedLoadRing::new(100, 1.25);
    assert!(!ring.add("x", 0));
    assert!(ring.is_empty());
}

#[test]
#[should_panic(expected = "beta must be > 1.0")]
fn bounded_bad_beta_panics() {
    let _ = BoundedLoadRing::new(100, 1.0);
}

#[test]
fn bounded_done_releases_load() {
    let ring = BoundedLoadRing::new(100, 1.25);
    ring.add("A", 1);
    ring.add("B", 1);

    let mut assigned = Vec::new();
    for i in 0..20 {
        let node = ring.get(&format!("key-{i}")).unwrap();
        assigned.push(node);
    }
    for node in &assigned {
        ring.done(node);
    }
    for (_node, load) in ring.loads() {
        assert_eq!(load, 0, "expected zero load after all Done calls");
    }
}

#[test]
fn bounded_remove_node() {
    let ring = BoundedLoadRing::new(100, 1.25);
    ring.add("A", 1);
    ring.add("B", 1);
    assert!(ring.remove("B"));
    assert_eq!(ring.len(), 1);
    for i in 0..10 {
        let node = ring.get(&format!("key-{i}")).unwrap();
        assert_eq!(node, "A");
        ring.done(&node);
    }
}

#[test]
fn bounded_remove_nonexistent_is_noop() {
    let ring = BoundedLoadRing::new(100, 1.25);
    assert!(!ring.remove("ghost"));
}
