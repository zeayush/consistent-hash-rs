mod ring;
mod jump;
mod bounded;

pub use ring::ConsistentHashRing;
pub use jump::JumpHashRing;
pub use bounded::BoundedLoadRing;

/// Common interface for all consistent hash implementations.
///
/// Both `ConsistentHashRing` and `JumpHashRing` implement this trait, so they
/// can be used interchangeably behind a `Box<dyn HashRouter>` or `Arc<dyn HashRouter>`.
///
/// Note: `BoundedLoadRing` also implements `HashRouter` but its `get` increments
/// an in-flight counter — callers must call `done` after each request.
pub trait HashRouter: Send + Sync {
    fn add(&self, node: &str, weight: usize) -> bool;
    fn remove(&self, node: &str) -> bool;
    fn get(&self, key: &str) -> Option<String>;
    fn nodes(&self) -> Vec<String>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

impl HashRouter for ConsistentHashRing {
    fn add(&self, node: &str, weight: usize) -> bool { self.add(node, weight) }
    fn remove(&self, node: &str) -> bool { self.remove(node) }
    fn get(&self, key: &str) -> Option<String> { self.get(key) }
    fn nodes(&self) -> Vec<String> { self.nodes() }
    fn len(&self) -> usize { self.len() }
    fn is_empty(&self) -> bool { self.is_empty() }
}

impl HashRouter for JumpHashRing {
    fn add(&self, node: &str, weight: usize) -> bool { self.add(node, weight) }
    fn remove(&self, node: &str) -> bool { self.remove(node) }
    fn get(&self, key: &str) -> Option<String> { self.get(key) }
    fn nodes(&self) -> Vec<String> { self.nodes() }
    fn len(&self) -> usize { self.len() }
    fn is_empty(&self) -> bool { self.is_empty() }
}

impl HashRouter for BoundedLoadRing {
    fn add(&self, node: &str, weight: usize) -> bool { self.add(node, weight) }
    fn remove(&self, node: &str) -> bool { self.remove(node) }
    fn get(&self, key: &str) -> Option<String> { self.get(key) }
    fn nodes(&self) -> Vec<String> { self.nodes() }
    fn len(&self) -> usize { self.len() }
    fn is_empty(&self) -> bool { self.is_empty() }
}
