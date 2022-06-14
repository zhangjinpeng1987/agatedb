

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