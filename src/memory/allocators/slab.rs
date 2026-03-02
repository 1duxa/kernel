//! # Memory Allocator Implementations
//!
//! This module provides several allocator implementations for different use cases:
//!
//! ## Active Allocators (Currently Used)
//!
//! - [`FixedSizeBlockAllocator`]: Main kernel heap allocator with size-class bins
//! - [`LinkedListAllocator`]: Fallback for large allocations (used by FixedSizeBlockAllocator)
//!
//! ## Reserved Allocators (For Future Use)
//!
//! - [`BumpAllocator`]: Simple, fast allocator that never frees (initialization, temporary buffers)
//! - [`StackAllocator`]: LIFO allocation pattern (temporary allocations with predictable lifetimes)
//! - [`SlabAllocator`]: Cache-aligned, object-specific allocation
//! - [`StackHeapAllocator`]: Separate stack/heap regions
//!
//! ## Thread Safety
//!
//! All allocators are thread-safe, using either atomic operations or spinlocks.

use super::block::BlockNode;
#[allow(unused_imports)]
use crate::memory::allocators::core::{
    align_down, align_up, validate_region, AllocError, SpinLock,
};
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::{self, NonNull};

// ============================================================================
// 5. SLAB ALLOCATOR (Cache-aligned, object-specific)
// ============================================================================

#[allow(dead_code)]
struct SlabAllocatorInner<const SIZE: usize, const ALIGN: usize> {
    head: Option<NonNull<BlockNode>>,
}

#[allow(dead_code)]
pub struct SlabAllocator<const SIZE: usize, const ALIGN: usize> {
    inner: UnsafeCell<SlabAllocatorInner<SIZE, ALIGN>>,
    lock: SpinLock,
}

// Safety: The UnsafeCell is protected by SpinLock
unsafe impl<const SIZE: usize, const ALIGN: usize> Sync for SlabAllocator<SIZE, ALIGN> {}

#[allow(dead_code)]
impl<const SIZE: usize, const ALIGN: usize> SlabAllocator<SIZE, ALIGN> {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(SlabAllocatorInner { head: None }),
            lock: SpinLock::new(),
        }
    }

    pub unsafe fn add_slab(&self, slab_start: usize, slab_size: usize) {
        let num_blocks = slab_size / SIZE;

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();

            for i in 0..num_blocks {
                let addr = slab_start + i * SIZE;
                let node = &mut *(addr as *mut BlockNode);
                node.next = inner.head;
                inner.head = NonNull::new(node as *mut BlockNode);
            }
        });
    }
}

unsafe impl<const SIZE: usize, const ALIGN: usize> GlobalAlloc for SlabAllocator<SIZE, ALIGN> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > SIZE || layout.align() > ALIGN {
            return ptr::null_mut();
        }

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();

            match inner.head {
                Some(mut node_ptr) => {
                    let node = node_ptr.as_mut();
                    inner.head = node.next;
                    node_ptr.as_ptr() as *mut u8
                }
                None => ptr::null_mut(),
            }
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if ptr.is_null() {
            return;
        }

        self.lock.with_lock(|| {
            let inner = &mut *self.inner.get();
            let node = &mut *(ptr as *mut BlockNode);
            node.next = inner.head;
            inner.head = NonNull::new(node);
        });
    }
}
