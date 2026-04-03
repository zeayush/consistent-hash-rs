/// Consistent hashing with bounded loads (Mirrokni, Thorup, Zadimoghaddam; Google, 2017).
///
/// Wraps `ConsistentHashRing` and enforces a per-node load ceiling:
///
/// ```text
/// ceil(β × totalLoad / numNodes)
/// ```
///
/// For each incoming request the ring is walked in order starting at the key's
/// position. The first node below its ceiling is chosen. This bounds any single
/// node to at most β times its fair share of in-flight requests.
///
/// # Usage
///
/// ```rust
/// use consistent_hash_rs::BoundedLoadRing;
/// let ring = BoundedLoadRing::new(100, 1.25);
/// ring.add("db1", 1);
/// ring.add("db2", 1);
///
/// if let Some(node) = ring.get("user:42") {
///     // … handle request …
///     ring.done(&node);   // MUST be called when request finishes
/// }
/// ```
use crate::ring::ConsistentHashRing;
use std::collections::HashMap;
use std::sync::Mutex;

struct LoadState {
    loads: HashMap<String, i64>,
    total: i64,
}

/// Thread-safe consistent hash ring with bounded loads.
pub struct BoundedLoadRing {
    ring: ConsistentHashRing,
    state: Mutex<LoadState>,
    beta: f64,
}

impl BoundedLoadRing {
    /// Create a bounded-load ring.
    ///
    /// `replicas` controls virtual-node count (same as `ConsistentHashRing::new`).  
    /// `beta` is the overload factor (must be `> 1.0`). A value of `1.25` allows
    /// each node to absorb up to 25 % more than its fair share before the ring
    /// promotes requests to the next candidate.
    ///
    /// # Panics
    /// Panics if `replicas == 0` or `beta <= 1.0`.
    pub fn new(replicas: usize, beta: f64) -> Self {
        assert!(beta > 1.0, "beta must be > 1.0");
        Self {
            ring: ConsistentHashRing::new(replicas),
            state: Mutex::new(LoadState {
                loads: HashMap::new(),
                total: 0,
            }),
            beta,
        }
    }

    /// Add a physical node. Delegates to the inner ring.
    pub fn add(&self, node: &str, weight: usize) -> bool {
        let ok = self.ring.add(node, weight);
        if ok {
            let mut s = self.state.lock().unwrap();
            s.loads.entry(node.to_owned()).or_insert(0);
        }
        ok
    }

    /// Remove a node and clean up its load accounting.
    pub fn remove(&self, node: &str) -> bool {
        let ok = self.ring.remove(node);
        if ok {
            let mut s = self.state.lock().unwrap();
            if let Some(load) = s.loads.remove(node) {
                s.total = (s.total - load).max(0);
            }
        }
        ok
    }

    /// Return the node for `key` under the bounded-load policy and increment
    /// its in-flight load counter.
    ///
    /// Returns `None` only when the ring is empty. When all nodes exceed their
    /// ceiling (highly unlikely in practice) the ring-primary candidate is
    /// returned as a fallback to prevent starvation.
    ///
    /// **Callers must call `done` once per successful `get`.**
    pub fn get(&self, key: &str) -> Option<String> {
        let num_nodes = self.ring.len();
        if num_nodes == 0 {
            return None;
        }
        let candidates = self.ring.get_n(key, num_nodes);
        if candidates.is_empty() {
            return None;
        }
        let num_nodes = candidates.len();

        let mut s = self.state.lock().unwrap();

        // threshold = ceil(β × totalLoad / n), minimum 1 so startup routing works.
        let threshold = ((self.beta * s.total as f64 / num_nodes as f64).ceil() as i64).max(1);

        for node in &candidates {
            let load = *s.loads.get(node).unwrap_or(&0);
            if load < threshold {
                *s.loads.entry(node.clone()).or_insert(0) += 1;
                s.total += 1;
                return Some(node.clone());
            }
        }

        // Fallback: ring-primary candidate.
        let node = candidates[0].clone();
        *s.loads.entry(node.clone()).or_insert(0) += 1;
        s.total += 1;
        Some(node)
    }

    /// Decrement the in-flight load counter for `node`.
    /// Must be called once for each successful `get` call when the request completes.
    pub fn done(&self, node: &str) {
        let mut s = self.state.lock().unwrap();
        if let Some(load) = s.loads.get_mut(node) {
            if *load > 0 {
                *load -= 1;
                s.total -= 1;
            }
        }
    }

    /// Returns a snapshot of current in-flight loads per node.
    pub fn loads(&self) -> HashMap<String, i64> {
        self.state.lock().unwrap().loads.clone()
    }

    /// Returns `true` if the ring has no physical nodes.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Returns the number of physical nodes.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Returns the list of physical nodes.
    pub fn nodes(&self) -> Vec<String> {
        self.ring.nodes()
    }
}
