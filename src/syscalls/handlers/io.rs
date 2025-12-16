//! # I/O System Call Handlers
//!
//! Implements file descriptor operations for user processes.
//!
//! ## Supported Operations
//!
//! - `sys_read`: Read from file descriptor
//! - `sys_write`: Write to file descriptor  
//! - `sys_open`: Open file (not implemented)
//! - `sys_close`: Close file descriptor (not implemented)
//!
//! ## File Descriptors
//!
//! | FD | Stream | Implementation  |
//! |----|--------|-----------------|
//! | 0  | stdin  | Keyboard buffer |
//! | 1  | stdout | Serial/terminal |
//! | 2  | stderr | Serial/terminal |
//!
//! ## Note
//!
//! Currently only stdout/stderr write is fully implemented.

use crate::syscalls::dispatcher::{SyscallResult, SyscallError};

/// Read from file descriptor
pub fn sys_read(fd: i32, buf: *mut u8, count: usize) -> SyscallResult {
    // Validate arguments
    if buf.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    match fd {
        0 => {
            // stdin - read from keyboard buffer
            // TODO: Implement keyboard buffer reading
            Err(SyscallError::NotImplemented)
        }
        _ => Err(SyscallError::BadFileDescriptor),
    }
}

/// Write to file descriptor
pub fn sys_write(fd: i32, buf: *const u8, count: usize) -> SyscallResult {
    // Validate arguments
    if buf.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    match fd {
        1 | 2 => {
            // stdout/stderr - write to serial/terminal
            unsafe {
                let slice = core::slice::from_raw_parts(buf, count);
                if let Ok(s) = core::str::from_utf8(slice) {
                    // Write to terminal
                    use core::fmt::Write;
                    let _ = write!(crate::SERIAL.lock(), "{}", s);
                    Ok(count)
                } else {
                    Err(SyscallError::InvalidArgument)
                }
            }
        }
        _ => Err(SyscallError::BadFileDescriptor),
    }
}

/// Open a file
pub fn sys_open(path: *const u8, flags: usize, mode: usize) -> SyscallResult {
    if path.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    // TODO: Implement file system
    Err(SyscallError::NotImplemented)
}

/// Close a file descriptor
pub fn sys_close(fd: i32) -> SyscallResult {
    if fd < 0 {
        return Err(SyscallError::BadFileDescriptor);
    }
    
    // TODO: Implement file descriptor table
    Err(SyscallError::NotImplemented)
}
