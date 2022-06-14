

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
        let cnt = self.children_cnt.load(Odering::Acquire);
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
}