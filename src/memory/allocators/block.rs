use super::linked_list::LinkedListAllocator;
#[allow(unused_imports)]
use crate::memory::allocators::core::{
    align_down, align_up, validate_region, AllocError, SpinLock,
};
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::{self, NonNull};

// ============================================================================
// 3. FIXED SIZE BLOCK ALLOCATOR (Fast, minimal fragmentation)
// ============================================================================

const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

pub struct BlockNode {
    pub next: Option<NonNull<BlockNode>>,
}

struct FixedSizeBlockAllocatorInner {
    list_heads: [Option<NonNull<BlockNode>>; BLOCK_SIZES.len()],
    fallback: LinkedListAllocator,
}

/// Fixed-size block allocator with fallback
/// Best for: frequent allocations of similar sizes
///
/// # Safety
/// - Must call `init()` before use
/// - Thread-safe through spin lock
pub struct FixedSizeBlockAllocator {
    inner: UnsafeCell<FixedSizeBlockAllocatorInner>,
    lock: SpinLock,
}

// Safety: The UnsafeCell is protected by SpinLock
unsafe impl Sync for FixedSizeBlockAllocator {}
unsafe impl Send for FixedSizeBlockAllocator {}

impl FixedSizeBlockAllocator {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(FixedSizeBlockAllocatorInner {
                list_heads: [None; BLOCK_SIZES.len()],
                fallback: LinkedListAllocator::new(),
            }),
            lock: SpinLock::new(),
        }
    }

    /// Initialize the allocator with a memory region
    ///
    /// # Safety
    /// - `heap_start` must point to valid, unused memory
    /// - `heap_size` must be sufficient for allocation
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            inner.fallback.init(heap_start, heap_size)
        })
    }

    fn list_index(layout: &Layout) -> Option<usize> {
        let required_size = layout.size().max(layout.align());
        BLOCK_SIZES.iter().position(|&s| s >= required_size)
    }
}

unsafe impl GlobalAlloc for FixedSizeBlockAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return ptr::null_mut();
        }

        match Self::list_index(&layout) {
            Some(idx) => self.lock.with_lock(|| {
                let inner = &mut *self.inner.get();

                if let Some(mut node_ptr) = inner.list_heads[idx] {
                    let node = node_ptr.as_mut();
                    inner.list_heads[idx] = node.next;
                    node_ptr.as_ptr() as *mut u8
                } else {
                    let block_size = BLOCK_SIZES[idx];
                    let block_layout =
                        Layout::from_size_align(block_size, block_size).unwrap_or(layout);
                    inner.fallback.alloc(block_layout)
                }
            }),
            None => self.lock.with_lock(|| {
                let inner = &mut *self.inner.get();
                inner.fallback.alloc(layout)
            }),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        match Self::list_index(&layout) {
            Some(idx) => self.lock.with_lock(|| {
                let inner = &mut *self.inner.get();
                let node_ptr = ptr as *mut BlockNode;
                (*node_ptr).next = inner.list_heads[idx];
                inner.list_heads[idx] = NonNull::new(node_ptr);
            }),
            None => {
                self.lock.with_lock(|| {
                    let inner = &mut *self.inner.get();
                    inner.fallback.dealloc(ptr, layout)
                });
            }
        }
    }
}
