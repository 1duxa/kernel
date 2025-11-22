pub mod numbers;
pub mod dispatcher;
pub mod handlers;

use dispatcher::{SyscallContext, dispatch_syscall};
use x86_64::structures::idt::InterruptStackFrame;

/// Initialize syscall support
pub fn init() {
    // Set up syscall MSRs for fast syscall instruction
    // This is where you'd configure STAR, LSTAR, SFMASK MSRs
    // For now, we'll use software interrupts (int 0x80)
    
    crate::println!("Syscall interface initialized");
}

/// Syscall interrupt handler (for int 0x80 style syscalls)
pub extern "x86-interrupt" fn syscall_handler(stack_frame: InterruptStackFrame) {
    // In a real implementation, you'd extract registers from stack frame
    // For now, this is a placeholder
    
    // TODO: Extract syscall number and arguments from saved registers
    // TODO: Call dispatch_syscall
    // TODO: Return result in RAX
}
