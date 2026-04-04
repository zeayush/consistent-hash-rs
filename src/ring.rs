use crc32fast::Hasher;
use std::collections::HashMap;
use std::sync::RwLock;

/// Internal mutable state of the ring, protected by an `RwLock`.
struct Inner {
    /// Sorted virtual-node hashes for binary-search lookups.
    sorted_keys: Vec<u32>,
    /// Maps a virtual-node hash → physical node name.
    hash_map: HashMap<u32, String>,
    /// Maps physical node name → weight (needed to recompute vnodes on removal).
    nodes: HashMap<String, usize>,
}

/// A thread-safe consistent hash ring.
///
/// Each physical node is mapped to `replicas * weight` virtual nodes on the
/// ring.  Key lookups binary-search the sorted vnode list and return the
/// first node whose hash is ≥ the key hash (wrapping to index 0 when the
/// key overshoots).
pub struct ConsistentHashRing {
    inner: RwLock<Inner>,
    /// Base number of virtual nodes per unit weight.
    replicas: usize,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Compute a deterministic CRC-32 hash for a virtual-node identifier.
fn vnode_hash(node: &str, idx: usize) -> u32 {
    let mut h = Hasher::new();
    h.update(node.as_bytes());
    h.update(b"#");
    h.update(idx.to_string().as_bytes());
    h.finalize()
}

/// Compute a CRC-32 hash for an arbitrary key.
fn key_hash(key: &str) -> u32 {
    let mut h = Hasher::new();
    h.update(key.as_bytes());
    h.finalize()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

impl ConsistentHashRing {
    /// Create an empty ring with the given base replica count.
    ///
    /// # Panics
    /// Panics if `replicas` is 0.
    pub fn new(replicas: usize) -> Self {
        assert!(replicas > 0, "replicas must be > 0");
        Self {
            inner: RwLock::new(Inner {
                sorted_keys: Vec::new(),
                hash_map: HashMap::new(),
                nodes: HashMap::new(),
            }),
            replicas,
        }
    }

    /// Returns `true` if the ring has no physical nodes.
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.read().unwrap();
        inner.nodes.is_empty()
    }

    /// Returns the number of physical nodes currently in the ring.
    pub fn len(&self) -> usize {
        let inner = self.inner.read().unwrap();
        inner.nodes.len()
    }

    // -- mutators -----------------------------------------------------------

    /// Add a physical node with the given weight.
    ///
    /// The node receives `replicas * weight` virtual nodes on the ring.
    /// If the node already exists it is first removed, then re-added with
    /// the new weight (useful for re-weighting).
    ///
    /// Returns `false` (no-op) when `weight` is 0.
    pub fn add(&self, node: &str, weight: usize) -> bool {
        if weight == 0 {
            return false;
        }

        let mut inner = self.inner.write().unwrap();

        // If node already present, strip old vnodes first.
        if inner.nodes.contains_key(node) {
            Self::remove_node_locked(&mut inner, node, self.replicas);
        }

        let vnode_count = self.replicas * weight;
        for i in 0..vnode_count {
            let hash = vnode_hash(node, i);
            // On collision the newer node silently wins (simple strategy).
            inner.hash_map.insert(hash, node.to_owned());
            inner.sorted_keys.push(hash);
        }

        inner.nodes.insert(node.to_owned(), weight);
        inner.sorted_keys.sort_unstable();
        inner.sorted_keys.dedup(); // drop duplicate hashes after sort
        true
    }

    /// Remove a physical node and all its virtual nodes from the ring.
    ///
    /// Returns `true` if the node was present and removed.
    pub fn remove(&self, node: &str) -> bool {
        let mut inner = self.inner.write().unwrap();
        if !inner.nodes.contains_key(node) {
            return false;
        }
        Self::remove_node_locked(&mut inner, node, self.replicas);
        true
    }

    /// Internal removal helper; caller must already hold write lock.
    fn remove_node_locked(inner: &mut Inner, node: &str, replicas: usize) {
        let weight = match inner.nodes.remove(node) {
            Some(w) => w,
            None => return,
        };

        let vnode_count = replicas * weight;
        for i in 0..vnode_count {
            let hash = vnode_hash(node, i);
            // Only remove if this hash still belongs to the node we're
            // deleting (another node may have collided and overwritten it).
            if inner.hash_map.get(&hash).map(|n| n.as_str()) == Some(node) {
                inner.hash_map.remove(&hash);
            }
        }

        // Rebuild sorted_keys from the current hash_map.
        inner.sorted_keys = inner.hash_map.keys().copied().collect();
        inner.sorted_keys.sort_unstable();
    }

    // -- lookups ------------------------------------------------------------

    /// Return the physical node responsible for `key`, or `None` if the
    /// ring is empty.
    pub fn get(&self, key: &str) -> Option<String> {
        let inner = self.inner.read().unwrap();
        if inner.sorted_keys.is_empty() {
            return None;
        }

        let hash = key_hash(key);

        // Find the first vnode hash >= key hash.
        let idx = match inner.sorted_keys.binary_search(&hash) {
            Ok(i) => i,
            Err(i) => {
                if i >= inner.sorted_keys.len() {
                    0 // wrap around to the first node
                } else {
                    i
                }
            }
        };

        inner.hash_map.get(&inner.sorted_keys[idx]).cloned()
    }

    /// Return the list of physical nodes currently in the ring.
    pub fn nodes(&self) -> Vec<String> {
        let inner = self.inner.read().unwrap();
        inner.nodes.keys().cloned().collect()
    }
}
