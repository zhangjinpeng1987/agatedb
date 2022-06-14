use std::sync::atomic::{Atomic32, AtomicU8, AtomicPtr};
use node::Node;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Node4 {
    pub(crate) keys: AtomicU32,
    pub(crate) children_addr: [AtomicPtr<Node>; 4],
    pub(crate) children_cnt: AtomicU8,
}

impl Node4 {
    pub new() -> Self {
        Self {
            keys: 0,
            children_addr: [ptr::null_mut(); 4],
            children_cnt: 0,
        }
    }
}

impl Node for Node4 {
    pub fn find_child(&self, partial_key: u8) -> *mut Node {
        let mut keys = self.keys.load(Ordering::Acquire);
        let children_cnt = self.children_cnt.load(Ordering::Acquire);
        let mut idx: u8 = 0;
        while keys > 0 && idx < children_cnt {
            let c = keys & 255;
            if c == partial_key {
                return children_addr[idx as usize].load(Ordering::Acquire);
            }
            keys >>= 8;
            idx += 1;
        }

        ptr::null()
    }

    pub fn set_child(&mut self, partial_key: u8, child: *mut Node) {
        let children_cnt = self.children_cnt.load(Ordering::Acquire);
        assert!(children_cnt < 4);
        self.children_addr[children_cnt as usize].store(child, Ordering::release);

        let mut delta: u32 = partial_key;
        let mut idx = 0;
        while idx < children_cnt {
            delta <<= 8;
            idx += 1;
        }
        let mut new_keys = self.keys.load(Ordering::Acquire);
        new_keys |= delta;
        self.keys.store(new_keys, Ordering::Release);
        self.children_cnt.store(children_cnt + 1, Ordering::Release);
    }

    pub fn grow(&mut self) -> *mut Node {
        let cnt = children_cnt.load(Odering::Acquire);
        assert_eq!(cnt, 4);
        let mut new_node = Node16::default();
        new_node.keys.store(keys.load(Ordering::Acquire), Ordering::Relaxed);
        for i in 0..4 {
            new_node.children_addr[i].store(children_addr[i].load(Ordering::Acquire));
        }
        new_node.children_cnt.store(4, Ordering::Release);
        new_node
    }

    pub fn is_full(&self) -> bool {
        self.children_cnt.load(Ordering::Acquire) == 4
    }
}