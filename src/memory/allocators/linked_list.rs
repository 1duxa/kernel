#[allow(unused_imports)]
use crate::memory::allocators::core::{
    align_down, align_up, validate_region, AllocError, SpinLock,
};
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::{self, NonNull};
// ============================================================================
// 2. LINKED LIST ALLOCATOR (Simple free list with proper locking)
// ============================================================================

struct ListNode {
    size: usize,
    next: Option<NonNull<ListNode>>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        Self { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

struct LinkedListAllocatorInner {
    head: Option<NonNull<ListNode>>,
    initialized: bool,
}

/// A linked list allocator with proper synchronization
/// Best for: general-purpose allocation when fragmentation is acceptable
///
/// # Safety
/// - Must call `init()` before use
/// - Thread-safe through spin lock
pub struct LinkedListAllocator {
    inner: UnsafeCell<LinkedListAllocatorInner>,
    lock: SpinLock,
}

// Safety: The UnsafeCell is protected by SpinLock
unsafe impl Sync for LinkedListAllocator {}

impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(LinkedListAllocatorInner {
                head: None,
                initialized: false,
            }),
            lock: SpinLock::new(),
        }
    }

    /// Initialize the allocator with a memory region
    ///
    /// # Safety
    /// - `heap_start` must point to valid, unused memory
    /// - `heap_size` must be large enough for at least one ListNode
    /// - Must only be called once
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        validate_region(heap_start, heap_size)?;

        if heap_size < core::mem::size_of::<ListNode>() {
            return Err(AllocError::InvalidSize);
        }

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();

            if inner.initialized {
                return Err(AllocError::InvalidAddress); // Already initialized
            }

            let node_ptr = heap_start as *mut ListNode;
            node_ptr.write(ListNode::new(heap_size));
            inner.head = NonNull::new(node_ptr);
            inner.initialized = true;
            Ok(())
        })
    }

    fn alloc_from_region(
        node: &mut ListNode,
        size: usize,
        align: usize,
    ) -> Result<usize, AllocError> {
        let alloc_start = align_up(node.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(AllocError::Overflow)?;

        if alloc_end > node.end_addr() {
            return Err(AllocError::OutOfMemory);
        }

        let prefix = alloc_start - node.start_addr();
        if prefix > 0 && prefix < core::mem::size_of::<ListNode>() {
            // Alignment would leave a front fragment too small to store a ListNode.
            return Err(AllocError::OutOfMemory);
        }

        let excess = node.end_addr() - alloc_end;
        if excess > 0 && excess < core::mem::size_of::<ListNode>() {
            // Not enough space for a new node
            return Err(AllocError::OutOfMemory);
        }

        Ok(alloc_start)
    }

    /// Merge two free regions that touch in physical address order. Returns true if one merge happened.
    unsafe fn merge_adjacent_once(head: &mut Option<NonNull<ListNode>>) -> bool {
        unsafe {
            let mut prev_a: *mut Option<NonNull<ListNode>> = core::ptr::from_mut(head);

            while let Some(mut na) = *prev_a {
                let a_start = na.as_ref().start_addr();
                let a_end = na.as_ref().end_addr();

                let mut prev_b: *mut Option<NonNull<ListNode>> = core::ptr::from_mut(head);

                while let Some(mut nb) = *prev_b {
                    if na.as_ptr() == nb.as_ptr() {
                        prev_b = core::ptr::addr_of_mut!((*nb.as_ptr()).next);
                        continue;
                    }

                    let b_start = nb.as_ref().start_addr();
                    let b_end = nb.as_ref().end_addr();

                    if a_end == b_start {
                        let add = nb.as_ref().size;
                        let nb_next = nb.as_mut().next;
                        na.as_mut().size += add;
                        *prev_b = nb_next;
                        return true;
                    }
                    if b_end == a_start {
                        let add = na.as_ref().size;
                        let na_next = na.as_mut().next;
                        nb.as_mut().size += add;
                        *prev_a = na_next;
                        return true;
                    }

                    prev_b = core::ptr::addr_of_mut!((*nb.as_ptr()).next);
                }

                prev_a = core::ptr::addr_of_mut!((*na.as_ptr()).next);
            }

            false
        }
    }
}

unsafe impl GlobalAlloc for LinkedListAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            return ptr::null_mut();
        }

        let required_size = align_up(size.max(core::mem::size_of::<ListNode>()), align);

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();

            if !inner.initialized {
                return ptr::null_mut();
            }

            let mut current = &mut inner.head;

            while let Some(mut node_ptr) = *current {
                let node = node_ptr.as_mut();

                match Self::alloc_from_region(node, required_size, align) {
                    Ok(alloc_start) => {
                        // Remove or split the node
                        if alloc_start == node.start_addr()
                            && alloc_start + required_size == node.end_addr()
                        {
                            // Exact fit - remove node
                            *current = node.next;
                        } else if alloc_start == node.start_addr() {
                            // Allocate from front - shrink node
                            let new_start = alloc_start + required_size;
                            let new_size = node.end_addr() - new_start;
                            let new_node_ptr = new_start as *mut ListNode;
                            new_node_ptr.write(ListNode {
                                size: new_size,
                                next: node.next,
                            });
                            *current = NonNull::new(new_node_ptr);
                        } else if alloc_start + required_size == node.end_addr() {
                            // Allocate from end - shrink node
                            node.size = alloc_start - node.start_addr();
                        } else {
                            // Split node
                            let second_start = alloc_start + required_size;
                            let second_size = node.end_addr() - second_start;
                            let second_ptr = second_start as *mut ListNode;
                            second_ptr.write(ListNode {
                                size: second_size,
                                next: node.next,
                            });
                            node.size = alloc_start - node.start_addr();
                            node.next = NonNull::new(second_ptr);
                        }
                        return alloc_start as *mut u8;
                    }
                    Err(_) => {
                        current = &mut node.next;
                    }
                }
            }

            ptr::null_mut()
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let size = align_up(
            layout.size().max(core::mem::size_of::<ListNode>()),
            layout.align(),
        );

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();

            if !inner.initialized {
                return;
            }

            let node_ptr = ptr as *mut ListNode;
            node_ptr.write(ListNode {
                size,
                next: inner.head,
            });
            inner.head = NonNull::new(node_ptr);

            unsafe {
                while Self::merge_adjacent_once(&mut inner.head) {}
            }
        });
    }
}
