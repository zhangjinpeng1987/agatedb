// https://db.in.tum.de/~leis/papers/ART.pdf Adaptive Radix Tree Node

use std::ptr;
use std::sync::atomic::{AtomicU32, AtomicU8, AtomicU64, AtomicPtr, Ordering};
use std::sync::Arc;

use crate trie_rs::TrieBuilder;

/*
#[repr(C)]
pub enum Node {
    Node4 {
        keys: AtomicU32, // 4 * U8
        children_addr: [AtomicPtr<Node>; 4],
        children_cnt: AtomicU8,
    },
    Node16 {
        keys_1: AtomicU64, // 8 U8
        keys_2: AtomicU64, // 8 U8
        children_addr: [AtomicPtr<Node>; 16],
        children_cnt: AtomicU8,
    },
    Node48 {
        keys: [AtomicU8; 256],
        children_addr: [AtomicPtr<Node>; 48],
        children_cnt: AtomicU8,
    },
    Node256 {
        children_addr: [AtomicPtr<Node>; 256],
    }
}

const EMPTY: u8 = 255;

impl Default for Node::Node16 {
    fn default() -> Self {
        Self {
            keys: [AtomicU8::new(EMPTY)],
            ..Self::default(),
        }
    }
}

impl Node {
    pub fn find_child(&self, partial_key: u8) -> *mut Node {
        match self {
            Self::Node4 { ref keys, ref children_addr, ref children_cnt } => {
                let mut keys = keys.load(Ordering::Acquire);
                let children_cnt = children_cnt.load(Ordering::Acquire);
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
            Self::Node16 { keys, children_addr, children_cnt } => {
                // Find with SIMD
                #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                let find = || {
                    #[cfg(target_arch = "x86")]
                    use std::arch::x86::*;
                    #[cfg(target_arch = "x86_64")]
                    use std::arch::x86_64::*;

                    // Replicate partial_key to 16 u8 integers
                    let key_search = unsafe { _mm128_setl_epi8(partial_key) };
                    let keys = keys.load(Ordering::Acquire);
                    // Compare 16 u8 integers with single instruction, and set the result to cmp
                    let cmp = unsafe { _mm128_cmpeq_epi8(key_search, keys) };
                    let mut mask: u128 = 1;
                    let mut children_cnt = children_cnt.load(Ordering::Acquire);
                    while children_cnt > 0 {
                        mask <<= 8;
                        children_cnt -= 1;
                    }
                    mask -= 1;
                    let bitfield = unsafe { _mm_movemask_epi8(cmp) & mask };
                    if bitfield > 0 {
                        return children_addr[bitfield.trailing_zeros() / 8].load(Ordering::Acquire);
                    }

                    ptr::null()
                };

                // Find without SIMD
                #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
                let find = || {
                    let mut keys = keys.load(Ordering::Acquire);
                    let children_cnt = children_cnt.load(Ordering::Acquire);
                    let mut idx: u8 = 0;
                    while idx < children_cnt {
                        let c: u8 = keys & 255;
                        keys >>= 8;
                        if c == partial_key {
                            return Some(children_addr[idx as usize].load(Ordering::Acquire));
                        }
                        idx += 1;
                    }
                    ptr::null()
                };

                find()
            }
            Self::Node48 { ref keys, ref children_addr, .. } => {
                let index = keys[partial_key as usize].load(Ordering::Acquire);
                if index == EMPTY {
                    return ptr::null();
                }
                children_addr[index as usize].load(Ordering::Acquire)
            }
            Self::Node256 { children_addr } => {
                children_addr[partial_key as usize].load(Ordering::Acquire)
            }
        }
    }

    pub fn set_child(&self, partial_key: u8, child: *mut Node) {
        match self {
            Self::Node4 { ref keys, ref children_addr, ref children_cnt } => {
                let cnt = children_cnt.load(Ordering::Acquire);
                assert!(cnt < 4);
                children_addr[cnt as usize].store(child, Ordering::Relaxed);

                let mut delta: u32 = partial_key;
                let mut idx = 0;
                while idx < cnt {
                    delta <<= 8;
                    idx += 1;
                }
                let mut new_keys = keys.load(Ordering::Acquire);
                new_keys |= delta;
                keys.store(new_keys, Ordering::Relaxed);
                children_cnt.store(cnt + 1, Ordering::Release);
            }
            Self::Node16 { ref keys, ref children_addr, ref children_cnt } => {
                let cnt = children_cnt.load(Ordering::Acquire);
                assert!(cnt < 16);
                let mut delta: u128 = partial_key;
                let mut idx: u8 = 0;
                while idx < cnt {
                    delta <<= 8;
                    idx += 1;
                }
                let new_keys = keys.load(Ordering::Acquire);
                new_keys &= delta;
                children_addr[cnt].store(child, Ordering::Relaxed);
                keys.store(new_keys, Ordering::Relaxed);
                children_cnt.store(cnt + 1, Release);
            }
            self::Node48 { ref keys, ref children_addr, ref children_cnt } => {
                let cnt = children_cnt.load(Ordering::Acquire);
                assert!(cnt < 48);
                children_addr[cnt].store(child, Ordering::Relaxed);
                children_cnt.store(cnt + 1, Ordering::Relaxed);
                keys[partial_key as usize].store(cnt, Ordering::Release);
            }
            self::Node256 { ref children_addr } => {
                children_addr[partial_key].store(child, Ordering::Release);
            }
        }
    }

    pub fn grow(&self) -> *mut Node {
        match self {
            Self::Node4 { ref keys, ref children_addr, ref children_cnt } => {
                let cnt = children_cnt.load(Ordering::Acquire);
                assert_eq!(cnt, 4);
                let mut new_node = Node::Node16::default();
                new_node.keys.store(keys.load(Ordering::Acquire), Ordering::Relaxed);
                for i in 0..4 {
                    new_node.children_addr[i].store(children_addr[i].load(Ordering::Acquire));
                }
                new_node.children_cnt.store(4, Ordering::Release);
                new_node
            }
            Self::Node16 { ref keys, ref children_addr, ref children_cnt } => {
                let cnt = children_cnt.load(Odering::Acquire);
                assert_eq!(cnt, 16);
                let mut new_node = Self::Node48::default();
                let mut k: u128 = keys.load(Ordering::Acquire);
                for i in 0..16 {
                    let c = k >> 8;
                    k >>= 8;
                    new_node.children_addr[c as usize].store(children_addr[i].load(Ordering::Acquire), Ordering::Relaxed);
                }
                new_node.children_cnt.store(16, Ordering::Release);
                new_node
            }
            self::Node48 { ref keys, ref children_addr, ref children_cnt } => {
                let cnt = children_cnt.load(Odering::Acquire);
                assert_eq!(cnt, 48);
                let mut new_node = Self::Node256::default();
                for i in 0..256 {
                    let addr = keys[i];
                    if !addr.is_null() {
                        new_node.children_addr[i].store(addr, Ordering::Release)
                    }
                }
                new_node
            }
            self::Node256 { children_addr } => {
                unimplemented!("Radix Tree Node256 can't grow!!!");
            }
        }
    }

    pub fn is_full(&self) -> bool {
        match self {
            Self::Node4 { children_cnt, .. } => {
                children_cnt.load(Ordering::Acquire) == 4
            }
            Self::Node16 { children_cnt, .. } => {
                children_cnt.load(Ordering::Acquire) == 16
            }
            Self::Node48 { children_cnt, .. } => {
                children_cnt.load(Ordering::Acquire) == 48
            }
            Self::Node256 {
                false
            }
        }
    }
}
 
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_default() {
        let n4 = Node::Node4::default();
        let n16 = Node::Node16::default();
    }
}
*/

pub trait NodeTrait {
    fn find_child(&self, partial_key: u8) -> Option<Arc<Box<dyn NodeTrait>>>;
    fn set_child(&mut self, parial_key: u8, child: *mut Node);
    fn grow(&self) -> Box<dyn Node>;
    fn is_full(&self) -> bool;
}

#[repr(C)]
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

#[repr(C)]
#[derive(Default)]
pub struct Node16 {
    pub(crate) keys: AtomicU128, // 16 * U8
    pub(crate) children_addr: [AtomicPtr<Node>; 16],
    pub(crate) children_cnt: AtomicU8,
}

impl Node for Node16 {
    // SIMD version
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    pub find_child(&self, partial_key: u8) -> Option<*mut Node> {
        #[cfg(target_arch = "x86")]
        use std::arch::x86::*;
        #[cfg(target_arch = "x86_64")]
        use std::arch::x86_64::*;

        // Replicate partial_key to 16 u8 integers
        let key_16 = unsafe { _mm128_setl_epi8(partial_key) };
        let keys = self.keys.load(Ordering::Acquire);
        // Compare 16 u8 integers with single instruction, and set the result to cmp
        let cmp = unsafe { _mm128_cmpeq_epi8(key_16, keys) };
        let mut mask: u128 = 1;
        let mut children_cnt = self.children_cnt.load(Ordering::Acquire);
        while children_cnt > 0 {
            mask <<= 8;
            children_cnt -= 1;
        }
        mask -= 1;
        let bitfield = unsafe { _mm_movemask_epi8(cmp) & mask };
        if bitfield > 0 {
            return Some(self.children_addr[bitfield.trailing_zeros() / 8].load(Ordering::Acquire));
        }

        None
    }

    // Normal version
    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
    pub find_child(&self, partial_key: u8) -> Option<*mut Node> {
        let mut keys = self.keys.load(Ordering::Acquire);
        let children_cnt = self.children_cnt.load(Ordering::Acquire);
        let mut idx: u8 = 0;
        while idx < children_cnt {
            let c: u8 = keys & 255;
            keys >>= 8;
            if c == partial_key {
                return Some(self.children_addr[idx as usize].load(Ordering::Acquire));
            }
            idx += 1;
        }

        None
    }

    pub set_child(&mut self, partial_key: u8, child: *mut Node) {
        let children_cnt = self.children_cnt.load(Ordering::Acquire);
        assert!(children_cnt < 16);
        let mut delta: u128 = partial_key;
        let mut idx: u8 = 0;
        while idx < children_cnt {
            delta <<= 8;
            idx += 1;
        }
        let new_keys = self.keys.load(Ordering::Acquire);
        new_keys &= delta;
        self.children_addr[children_cnt].store(child, Ordering::Relaxed);
        self.keys.store(new_keys, Ordering::Relaxed);
        self.children_cnt.store(children_cnt + 1, Release);
    }

    pub fn is_full(&self) -> bool {
        self.children_cnt.load(Ordering::Acquire) == 16
    }

    pub grow(&mut self) -> *mut Node {
        let new_node = Node48::default();

    }
}

#[repr(C)]
#[derive(Default)]
pub struct Node48 {
    keys: [AtomicU8; 256],
    children_addr: [AtomicPtr<Node>; 48],
    children_cnt: AtomicU8,
}

impl Node48 {
    const EMPTY: u8 = 255;

    pub fn new() -> Self {
        Self {
            keys: [AtomicU8::new(EMPTY); 256],
            children_addr: [AtomicPtr::new(ptr::null_mut()); 48],
            children_cnt: AtomicU8::new(0),
        }
    }
}

impl Node for Node48 {
    pub fn find_child(&self, partial_key: u8) -> *mut Node {
        let index = self.keys[partial_key as usize].load(Ordering::Acquire);
        if index == Self::EMPTY {
            return None;
        }
        Some(self.children_addr[index as usize].load(Ordering::Acquire));
    }

    pub fn set_child(&mut self, partial_key: u8, child: *mut Node) {
        let children_cnt = self.children_cnt.load(Ordering::Acquire);
        assert!(children_cnt < 48);
        self.children_addr[children_cnt].store(child, Ordering::Relaxed);
        self.children_cnt.store(children_cnt + 1, Ordering::Relaxed);
        self.keys[partial_key as usize].store(children_cnt, Ordering::Release);
    }

    pub fn is_full(&self) -> bool {
        self.children_cnt.load(Ordering::Acquire) == 48
    }
}

#[repr(C)]
#[derive(Default)]
pub struct Node256 {
    children_addr[AtomicPtr<BoxNode>; 256],
}

impl Node256 {
    fn pub new() -> Self {
        Self::default()
    }
}

impl Node for Node256 {
    pub fn find_child(&self, partial_key: u8) -> Box<Node> {
        self.children_addr[partial_key as usize].load(Ordering::Acquire)
    }

    pub fn set_child(&self, partial_key: u8, child: *mut Node) {
        self.children_addr[partial_key].store(child, Ordering::Release);
    }

    pub fn is_full(&self) -> bool { false }

    pub fn grow(&self) -> Box<Node> {
        !unimplemented()
    }
}