use crate::syscall::dispatcher::{SyscallResult, SyscallError};

/// Sleep for specified milliseconds
pub fn sys_sleep(milliseconds: usize) -> SyscallResult {
    // TODO: Implement sleep using timer interrupts
    // For now, busy wait (not ideal!)
    let target_ticks = crate::interrupts::TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed)
        + (milliseconds * 18 / 1000); // ~18.2 Hz timer
    
    while crate::interrupts::TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed) < target_ticks {
        unsafe { core::arch::x86_64::_mm_pause(); }
    }
    
    Ok(0)
}

/// Get current time in milliseconds since boot
pub fn sys_gettime() -> SyscallResult {
    let ticks = crate::interrupts::TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed);
    // Convert ticks to milliseconds (~18.2 Hz = ~55ms per tick)
    Ok((ticks * 55) as usize)
}
