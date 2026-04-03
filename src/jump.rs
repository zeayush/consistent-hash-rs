/// Jump consistent hash ring (Lamping & Veach, 2014).
///
/// Properties vs vnode ring:
/// - Lookup: O(ln n) — compact inner loop, no binary search on vnode table.
/// - Memory: O(n) — stores only the node list; no virtual node table.
/// - Uniformity: near-perfect (no hash-collision variance from vnodes).
/// - Weighted nodes: NOT supported — weight is accepted for trait compatibility
///   but ignored. Repeat an `add` call via a wrapper if weighting is needed.
/// - Removal: adding at the end gives O(1/n) redistribution (minimal). Removing
///   a mid-list node shuffles indices for higher-indexed nodes.
///
/// Best suited for stateless load balancing where backends are homogeneous.
use std::sync::RwLock;

/// Compute a 64-bit FNV-1a hash for a string key.
fn fnv64(key: &str) -> u64 {
    let mut hash: u64 = 14695981039346656037;
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

/// Core jump hash algorithm (Lamping & Veach, 2014).
fn jump_hash(mut key: u64, num_buckets: usize) -> usize {
    let (mut b, mut j) = (-1i64, 0i64);
    while j < num_buckets as i64 {
        b = j;
        key = key.wrapping_mul(2862933555777941757).wrapping_add(1);
        j = ((b + 1) as f64 * (((1i64 << 31) as f64) / ((key >> 33) as f64 + 1.0))) as i64;
    }
    b as usize
}

/// Thread-safe consistent hash ring using the jump consistent hash algorithm.
pub struct JumpHashRing {
    inner: RwLock<Vec<String>>, // alphabetically sorted for determinism
}

impl JumpHashRing {
    /// Create an empty jump hash ring.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(Vec::new()),
        }
    }

    /// Add a node. `weight` is ignored — jump hash distributes load uniformly.
    /// Returns `false` if `weight == 0` or the node is already present.
    pub fn add(&self, node: &str, weight: usize) -> bool {
        if weight == 0 {
            return false;
        }
        let mut nodes = self.inner.write().unwrap();
        // Reject duplicates.
        if nodes.binary_search_by(|n| n.as_str().cmp(node)).is_ok() {
            return false;
        }
        // Insert in sorted position for deterministic bucket→name mapping.
        let idx = nodes.partition_point(|n| n.as_str() < node);
        nodes.insert(idx, node.to_owned());
        true
    }

    /// Remove a node. Returns `false` if not present.
    pub fn remove(&self, node: &str) -> bool {
        let mut nodes = self.inner.write().unwrap();
        match nodes.binary_search_by(|n| n.as_str().cmp(node)) {
            Ok(idx) => {
                nodes.remove(idx);
                true
            }
            Err(_) => false,
        }
    }

    /// Returns the node for `key`, or `None` when the ring is empty.
    pub fn get(&self, key: &str) -> Option<String> {
        let nodes = self.inner.read().unwrap();
        let n = nodes.len();
        if n == 0 {
            return None;
        }
        let idx = jump_hash(fnv64(key), n);
        Some(nodes[idx].clone())
    }

    /// Returns `true` if the ring has no physical nodes.
    pub fn is_empty(&self) -> bool {
        self.inner.read().unwrap().is_empty()
    }

    /// Returns the number of physical nodes.
    pub fn len(&self) -> usize {
        self.inner.read().unwrap().len()
    }

    /// Returns the sorted list of physical nodes.
    pub fn nodes(&self) -> Vec<String> {
        self.inner.read().unwrap().clone()
    }
}

impl Default for JumpHashRing {
    fn default() -> Self {
        Self::new()
    }
}
