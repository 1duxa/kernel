//! # Interrupt Descriptor Table and Handlers
//!
//! Defines the IDT and all interrupt/exception handlers for the kernel.
//!
//! ## Exception Handlers
//!
//! | Exception              | Action                              |
//! |------------------------|-------------------------------------|
//! | Breakpoint (#BP)       | Print debug info, continue          |
//! | Page Fault (#PF)       | Diagnostic output, then panic       |
//! | Divide Error (#DE)     | Panic                               |
//! | Invalid Opcode (#UD)   | Panic                               |
//! | General Protection     | Panic with error code               |
//! | Double Fault (#DF)     | Panic (uses IST stack)              |
//!
//! ## Hardware Interrupts
//!
//! | IRQ  | Vector | Handler                   |
//! |------|--------|---------------------------|
//! | IRQ0 | 32     | Timer (increments tick)   |
//! | IRQ1 | 33     | Keyboard (PS/2 scancode)  |
//! | IRQ12| 44     | Mouse (PS/2 packet)       |
//!
//! ## Syscall
//!
//! Vector 0x80 handles system calls. Arguments in registers are
//! dispatched to the appropriate syscall handler.

use crate::{
    core::interrupts::{
        gdt,
        pic::{handle_interrupt, EoiTiming, InterruptIndex},
    },
    println,
    syscalls::dispatcher::SyscallContext,
};
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Lazy;
use x86_64::{
    instructions::port::Port,
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
};
// PhysAddr is provided via frame.start_address when needed
pub static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    // CPU EXCEPTIONS (0-31)
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
    idt.general_protection_fault
        .set_handler_fn(general_protection_fault_handler);
    // Double fault needs its own stack to avoid cascading failures
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    }
    // HARDWARE INTERRUPTS (32-47 after remapping)
    idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_interrupt_handler);
    idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_interrupt_handler);
    idt[InterruptIndex::Mouse.as_u8()].set_handler_fn(mouse_interrupt_handler);
    idt[InterruptIndex::Syscall.as_u8()].set_handler_fn(syscall_handler);

    idt
});

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(sf: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", sf);
}

extern "x86-interrupt" fn divide_error_handler(sf: InterruptStackFrame) {
    panic!("EXCEPTION: DIVIDE BY ZERO\n{:#?}", sf);
}

extern "x86-interrupt" fn invalid_opcode_handler(sf: InterruptStackFrame) {
    panic!("EXCEPTION: INVALID OPCODE\n{:#?}", sf);
}

extern "x86-interrupt" fn general_protection_fault_handler(sf: InterruptStackFrame, err: u64) {
    panic!(
        "EXCEPTION: GENERAL PROTECTION FAULT (error code: {})\n{:#?}",
        err, sf
    );
}

extern "x86-interrupt" fn double_fault_handler(sf: InterruptStackFrame, err: u64) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{sf:#?}\n CODE{err}");
}

extern "x86-interrupt" fn page_fault_handler(_sf: InterruptStackFrame, _err: PageFaultErrorCode) {
    use x86_64::registers::control::Cr2;
    if let Ok(addr) = Cr2::read() {
        crate::memory::debug_page_walk(addr);
    };
    panic!("Page fault!");
}

extern "x86-interrupt" fn timer_interrupt_handler(_sf: InterruptStackFrame) {
    handle_interrupt(
        InterruptIndex::Timer,
        || {
            TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
        },
        EoiTiming::After,
    );
}

extern "x86-interrupt" fn syscall_handler(sf: InterruptStackFrame) {
    handle_interrupt(
        InterruptIndex::Syscall,
        || {
            let _ = crate::syscalls::dispatcher::dispatch_syscall(SyscallContext {
                syscall_num: sf.stack_pointer.as_u64() as usize,
                arg0: 0,
                arg1: 0,
                arg2: 0,
                arg3: 0,
                arg4: 0,
                arg5: 0,
            });
        },
        EoiTiming::Before,
    );
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_sf: InterruptStackFrame) {
    use crate::devices::drivers::ps2_keyboard;
    handle_interrupt(
        InterruptIndex::Keyboard,
        || {
            let mut status_port = Port::<u8>::new(0x64);
            let mut data_port = Port::<u8>::new(0x60);
            while unsafe { (status_port.read() & 0x01) == 0x01 } {
                if unsafe { status_port.read() & 0x20 != 0 } {
                    continue;
                }
                let sc = unsafe { data_port.read() };
                println!("IRQ: keyboard scancode {:#x}", sc);
                ps2_keyboard::enqueue_scancode(sc);
            }
        },
        EoiTiming::Before,
    );
}

extern "x86-interrupt" fn mouse_interrupt_handler(_sf: InterruptStackFrame) {
    // Check PS/2 status register - bit 0 = output buffer full, bit 5 = from mouse
    handle_interrupt(
        InterruptIndex::Mouse,
        || {
            while unsafe { (Port::<u8>::new(0x64).read() & 0x21) == 0x21 } {
                let byte: u8 = unsafe { Port::new(0x60).read() };
                crate::devices::drivers::ps2_mouse::enqueue_mouse_byte(byte);
            }
        },
        EoiTiming::After,
    );
}
