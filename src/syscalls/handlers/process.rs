//! # Process Management System Calls
//!
//! Implements process creation, termination, and identification.
//!
//! ## Supported Operations
//!
//! - `sys_exit`: Terminate current process
//! - `sys_fork`: Create child process (partial)
//! - `sys_exec`: Execute new program (stub)
//! - `sys_wait`: Wait for child process (stub)
//! - `sys_getpid`: Get current process ID
//!
//! ## Process Table
//!
//! A simple fixed-size process table tracks active processes:
//! - Maximum 256 processes
//! - Protected by spinlock
//! - Each entry stores PID, parent PID, exit status
//!
//! ## PID Allocation
//!
//! PIDs are allocated atomically from a counter starting at 1.
//! PID 0 indicates no process (kernel context).

use crate::syscalls::dispatcher::{SyscallResult, SyscallError};
use core::sync::atomic::{AtomicUsize, Ordering};

static NEXT_PID: AtomicUsize = AtomicUsize::new(1);
static CURRENT_PID: AtomicUsize = AtomicUsize::new(0);

pub fn get_next_pid() -> usize {
    NEXT_PID.fetch_add(1, Ordering::SeqCst)
}

#[derive(Debug, Clone, Copy)]
struct ProcessContext {
    pid: usize,
    parent_pid: usize,
    exit_status: i32,
    page_table: u64,
}

static mut PROCESS_TABLE: [Option<ProcessContext>; 256] = [None; 256];
static PROCESS_TABLE_LOCK: spin::Mutex<()> = spin::Mutex::new(());

pub fn sys_exit(status: i32) -> SyscallResult {
    let pid = CURRENT_PID.load(Ordering::Relaxed);
    crate::println!("Process {} exiting with status: {}", pid, status);
    
    loop {
        ::core::hint::spin_loop();
    }
}

pub fn sys_getpid() -> SyscallResult {
    let pid = CURRENT_PID.load(Ordering::Relaxed);
    if pid == 0 {
        Ok(1)
    } else {
        Ok(pid)
    }
}

pub fn sys_fork() -> SyscallResult {
    let _guard = PROCESS_TABLE_LOCK.lock();
    let parent_pid = CURRENT_PID.load(Ordering::Relaxed);
    
    let child_pid = NEXT_PID.fetch_add(1, Ordering::SeqCst);
    if child_pid > 255 {
        return Err(SyscallError::NoMemory);
    }
    
    unsafe {
        let child_page_table = match crate::memory::create_process_page_table() {
            Ok(frame) => frame.start_address().as_u64(),
            Err(_) => return Err(SyscallError::NoMemory),
        };
        
        PROCESS_TABLE[child_pid] = Some(ProcessContext {
            pid: child_pid,
            parent_pid,
            exit_status: 0,
            page_table: child_page_table,
        });
    }
    
    Ok(child_pid)
}

pub fn sys_exec(path: *const u8, _argv: *const *const u8) -> SyscallResult {
    if path.is_null() {
        return Err(SyscallError::InvalidArgument);
    }
    
    let code_ptr = path as *const u8;
    let code_size = unsafe {
        let mut size = 0;
        while *(code_ptr.add(size)) != 0 && size < 10 * 1024 * 1024 {
            size += 1;
        }
        size
    };
    
    if code_size == 0 || code_size > 10 * 1024 * 1024 {
        return Err(SyscallError::InvalidArgument);
    }
    
    match unsafe { crate::memory::sys_pstart(code_ptr, code_size) } {
        Ok(pid) => {
            CURRENT_PID.store(pid, Ordering::Relaxed);
            Ok(pid)
        }
        Err(_) => Err(SyscallError::NoMemory),
    }
}

pub fn sys_wait(_status: *mut i32) -> SyscallResult {
    let pid = CURRENT_PID.load(Ordering::Relaxed);
    crate::println!("Process {} waiting for children", pid);
    
    loop {
        ::core::hint::spin_loop();
    }
}

