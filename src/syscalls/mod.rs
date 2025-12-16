//! System Call Interface
//!
//! This module provides the syscall infrastructure for the kernel.
//! Currently implements memory-related syscalls (mmap, munmap, brk).
//!
//! # Syscall Numbers
//! See `numbers.rs` for the syscall number definitions.
//!
//! # Handlers
//! Individual syscall handlers are in the `handlers` submodule.

pub mod numbers;
pub mod dispatcher;
pub mod handlers;

// Re-export commonly used items
pub use dispatcher::SyscallError;
