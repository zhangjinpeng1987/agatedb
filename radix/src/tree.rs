// https://db.in.tum.de/~leis/papers/ART.pdf Adaptive Radix Tree

use super::node::Node;

pub struct Tree {
    root: AtomicPtr<Node>,
}

