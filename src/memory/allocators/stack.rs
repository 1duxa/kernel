use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self};
use core::sync::atomic::{AtomicUsize, Ordering};

#[allow(unused_imports)]
use crate::memory::allocators::core::{
    align_down, align_up, validate_region, AllocError, SpinLock,
};

// ============================================================================
// 4. STACK ALLOCATOR (LIFO allocation/deallocation)
// ============================================================================

/// A stack allocator for LIFO allocation patterns
/// Best for: temporary allocations with predictable lifetimes
///
/// # Safety
/// - Must call `init()` before use
/// - Deallocations must happen in reverse order of allocations
/// - Thread-safe through atomic operations
#[allow(dead_code)]
pub struct StackAllocator {
    start: AtomicUsize,
    end: AtomicUsize,
    top: AtomicUsize,
    initialized: AtomicUsize,
}

#[allow(dead_code)]
impl StackAllocator {
    pub const fn new() -> Self {
        Self {
            start: AtomicUsize::new(0),
            end: AtomicUsize::new(0),
            top: AtomicUsize::new(0),
            initialized: AtomicUsize::new(0),
        }
    }

    /// Initialize the allocator with a memory region
    ///
    /// # Safety
    /// - `heap_start` must point to valid, unused memory
    /// - `heap_size` must not exceed available memory
    /// - Must only be called once
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) -> Result<(), AllocError> {
        validate_region(heap_start, heap_size)?;

        if self.initialized.swap(1, Ordering::SeqCst) != 0 {
            return Err(AllocError::InvalidAddress);
        }

        self.start.store(heap_start, Ordering::Release);
        self.end.store(heap_start + heap_size, Ordering::Release);
        self.top.store(heap_start, Ordering::Release);
        Ok(())
    }

    /// Reset the allocator to initial state
    ///
    /// # Safety
    /// - All previously allocated memory becomes invalid
    /// - Caller must ensure no references to allocated memory exist
    pub unsafe fn reset(&self) {
        if self.initialized.load(Ordering::Acquire) != 0 {
            let start = self.start.load(Ordering::Acquire);
            self.top.store(start, Ordering::Release);
        }
    }

    pub fn used(&self) -> usize {
        let start = self.start.load(Ordering::Acquire);
        let top = self.top.load(Ordering::Acquire);
        top.saturating_sub(start)
    }
}

unsafe impl GlobalAlloc for StackAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if self.initialized.load(Ordering::Acquire) == 0 {
            return ptr::null_mut();
        }

        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            return ptr::null_mut();
        }

        let end = self.end.load(Ordering::Acquire);

        loop {
            let current = self.top.load(Ordering::Acquire);
            let aligned = align_up(current, align);

            let new_top = match aligned.checked_add(size) {
                Some(n) => n,
                None => return ptr::null_mut(),
            };

            if new_top > end {
                return ptr::null_mut();
            }

            if self
                .top
                .compare_exchange_weak(current, new_top, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return aligned as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        // Only allow deallocation if it's the most recent allocation (LIFO)
        let addr = ptr as usize;
        let size = layout.size();
        let expected_top = addr.saturating_add(size);

        // Try to pop this allocation off the stack
        _ = self
            .top
            .compare_exchange(expected_top, addr, Ordering::AcqRel, Ordering::Acquire);
        // If this fails, it means deallocations are out of order
        // In a production OS, you might want to panic or log this
    }
}
