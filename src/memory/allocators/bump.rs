use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self};
use core::sync::atomic::{AtomicUsize, Ordering};

#[allow(unused_imports)]
use crate::memory::allocators::core::{
    align_down, align_up, validate_region, AllocError, SpinLock,
};

// ============================================================================
// 1. BUMP ALLOCATOR (Simple, fast, no deallocation)
// ============================================================================

/// A simple bump allocator that never frees memory.
/// Best for: short-lived allocations, initialization, temporary buffers
///
/// # Safety
/// - Must call `init()` before use
/// - Thread-safe through atomic operations
/// - Never reuses memory until reset
#[allow(dead_code)]
pub struct BumpAllocator {
    heap_start: AtomicUsize,
    heap_end: AtomicUsize,
    next: AtomicUsize,
    initialized: AtomicUsize,
}

#[allow(dead_code)]
impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: AtomicUsize::new(0),
            heap_end: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
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
            return Err(AllocError::InvalidAddress); // Already initialized
        }

        self.heap_start.store(heap_start, Ordering::Release);
        self.heap_end
            .store(heap_start + heap_size, Ordering::Release);
        self.next.store(heap_start, Ordering::Release);
        Ok(())
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire) != 0
    }

    /// Reset the allocator, invalidating all previous allocations
    ///
    /// # Safety
    /// - All previously allocated memory becomes invalid
    /// - Caller must ensure no references to allocated memory exist
    pub unsafe fn reset(&self) {
        if self.is_initialized() {
            let start = self.heap_start.load(Ordering::Acquire);
            self.next.store(start, Ordering::Release);
        }
    }

    pub fn used(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        let start = self.heap_start.load(Ordering::Acquire);
        let next = self.next.load(Ordering::Acquire);
        next.saturating_sub(start)
    }

    pub fn remaining(&self) -> usize {
        if !self.is_initialized() {
            return 0;
        }
        let end = self.heap_end.load(Ordering::Acquire);
        let next = self.next.load(Ordering::Acquire);
        end.saturating_sub(next)
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.is_initialized() {
            return ptr::null_mut();
        }

        let size = layout.size();
        let align = layout.align();

        if size == 0 {
            return ptr::null_mut();
        }

        let heap_end = self.heap_end.load(Ordering::Acquire);

        loop {
            let current = self.next.load(Ordering::Acquire);
            let aligned = align_up(current, align);

            let new_next = match aligned.checked_add(size) {
                Some(n) => n,
                None => return ptr::null_mut(),
            };

            if new_next > heap_end {
                return ptr::null_mut();
            }

            if self
                .next
                .compare_exchange_weak(current, new_next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return aligned as *mut u8;
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator doesn't support individual deallocation
    }
}
