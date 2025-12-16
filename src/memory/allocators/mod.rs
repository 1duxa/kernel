//! # Memory Allocators
//!
//! Provides heap allocation implementations for the kernel.
//!
//! ## Allocator Types
//!
//! ### BumpAllocator
//! - Simplest allocator, never frees memory
//! - Very fast allocation (just pointer increment)
//! - Good for: initialization, temporary buffers
//!
//! ### FreeListAllocator  
//! - Maintains linked list of free blocks
//! - Supports allocation and deallocation
//! - First-fit allocation strategy
//!
//! ### BuddyAllocator
//! - Power-of-two block sizes
//! - Efficient for varied allocation sizes
//! - Good fragmentation properties
//!
//! ## Core Utilities
//!
//! - `SpinLock`: Simple spinlock for thread safety
//! - `AllocError`: Allocation failure types
//! - Alignment helpers: `align_up`, `align_down`

mod core;
pub mod simple;

pub use core::AllocError;
pub use simple::*;
