//! # Memory System Call Handler Utilities
//!
//! Provides helper functions for memory-related system calls.
//!
//! ## Functions
//!
//! - `get_active_mapper`: Returns the active page table mapper
//!
//! ## Usage
//!
//! This is a convenience wrapper that delegates to `crate::memory::active_page_table()`.
//! It provides a cleaner interface for syscall handlers that need to manipulate
//! page tables.
//!
//! ## Note
//!
//! The actual mmap/munmap/brk implementations are in `crate::memory`.

use x86_64::structures::paging::OffsetPageTable;

// Return an active OffsetPageTable mapper rooted at CR3 as a convenience wrapper
pub unsafe fn get_active_mapper() -> OffsetPageTable<'static> {
    crate::memory::active_page_table()
}