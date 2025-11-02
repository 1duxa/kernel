/// Core utilities and error types for allocators
use core::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// SPIN LOCK (for thread-safe allocators)
// ============================================================================

pub(crate) struct SpinLock {
    locked: AtomicUsize,
}

impl SpinLock {
    pub(crate) const fn new() -> Self {
        Self { locked: AtomicUsize::new(0) }
    }

    pub(crate) fn lock(&self) {
        while self.locked.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed).is_err() {
            while self.locked.load(Ordering::Relaxed) != 0 {
                core::hint::spin_loop();
            }
        }
    }

    pub(crate) fn unlock(&self) {
        self.locked.store(0, Ordering::Release);
    }
}

struct SpinLockGuard<'a> {
    lock: &'a SpinLock,
}

impl<'a> Drop for SpinLockGuard<'a> {
    fn drop(&mut self) {
        self.lock.unlock();
    }
}

impl SpinLock {
    pub(crate) fn with_lock<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.lock();
        let _guard = SpinLockGuard { lock: self };
        f()
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

#[inline]
pub(crate) const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

#[inline]
pub(crate) const fn align_down(addr: usize, align: usize) -> usize {
    addr & !(align - 1)
}

#[inline]
pub(crate) fn is_aligned(addr: usize, align: usize) -> bool {
    addr & (align - 1) == 0
}

/// Validates that a memory region is safe to use
pub(crate) fn validate_region(start: usize, size: usize) -> Result<(), AllocError> {
    if start == 0 {
        return Err(AllocError::InvalidAddress);
    }
    if size == 0 {
        return Err(AllocError::InvalidSize);
    }
    start.checked_add(size).ok_or(AllocError::Overflow)?;
    Ok(())
}

// ============================================================================
// ERROR TYPES
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocError {
    OutOfMemory,
    InvalidAddress,
    InvalidSize,
    Overflow,
    Uninitialized,
}

