//! # Time System Call Handlers
//!
//! Provides time-related system calls.
//!
//! ## Supported Operations
//!
//! - `sys_sleep`: Sleep for specified milliseconds
//! - `sys_gettime`: Get time since boot in milliseconds
//!
//! ## Timer Resolution
//!
//! The system uses the PIT (Programmable Interval Timer) at ~18.2 Hz,
//! giving approximately 55ms resolution per tick.
//!
//! ## Implementation Notes
//!
//! `sys_sleep` currently uses busy-waiting, which is not ideal for
//! power efficiency but simple to implement. A proper implementation
//! would yield the CPU and use timer interrupts.

use crate::syscalls::dispatcher::{SyscallResult};
use crate::core::interrupts::interrupts::TIMER_TICKS;
/// Sleep for specified milliseconds
pub fn sys_sleep(milliseconds: u64) -> SyscallResult {
    // For now, busy wait (not ideal!)
    let target_ticks = TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed)
        + (milliseconds * 18 / 1000); // ~18.2 Hz timer
    
    while TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed) < target_ticks {
        core::hint::spin_loop();
    }
    
    Ok(0)
}

/// Get current time in milliseconds since boot
pub fn sys_gettime() -> SyscallResult {
    let ticks = TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed);
    // Convert ticks to milliseconds (~18.2 Hz = ~55ms per tick)
    Ok((ticks * 55) as usize)
}
