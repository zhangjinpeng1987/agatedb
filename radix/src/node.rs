// https://db.in.tum.de/~leis/papers/ART.pdf Adaptive Radix Tree Node

use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicU8, AtomicU64, AtomicPtr, Ordering};
use std::sync::Arc;


pub enum NodeType {
    NODE4 = 0,
    NODE16 = 1,
    NODE48 = 2,
    NODE256 = 3,
}

pub struct BaseNode {
    node_type: NodeType,

}

pub trait Node {
    fn find_child(&self, partial_key: u8) -> Option<Arc<Box<dyn NodeTrait>>>;
    fn set_child(&mut self, parial_key: u8, child: Arc<Box<dyn NodeTrait>>);
    fn grow(&self) -> Box<dyn Node>;
    fn is_full(&self) -> bool;
}
