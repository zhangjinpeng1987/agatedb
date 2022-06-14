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

    pub fn grow(&self) -> *mut Node {
        let cnt = self.children_cnt.load(Odering::Acquire);
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
}