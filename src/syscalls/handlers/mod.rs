//! # System Call Handlers
//!
//! Contains implementations for all system call categories.
//!
//! ## Modules
//!
//! - `io`: File I/O operations (read, write, open, close)
//! - `process`: Process management (exit, fork, exec, getpid)
//! - `time`: Time operations (sleep, gettime)
//! - `memory`: Memory management (mmap, munmap, brk)
//!
//! ## Handler Signature
//!
//! Each handler takes arguments from registers and returns a result:
//! ```ignore
//! fn sys_write(fd: usize, buf: *const u8, count: usize) -> SyscallResult
//! ```

pub mod io;
pub mod process;
pub mod time;
pub mod memory;