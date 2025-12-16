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

use core::sync::atomic::{AtomicU64, Ordering};

use crate::{ core::interrupts::{gdt, pic::InterruptIndex}, println };
use spin::Lazy;
use x86_64::{instructions::port::Port, structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode}, VirtAddr};
use x86_64::structures::paging::Translate;
// PhysAddr is provided via frame.start_address when needed
pub static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();

    // CPU EXCEPTIONS (0-31)
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    idt.divide_error.set_handler_fn(divide_error_handler);
    idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
    idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);
    // Double fault needs its own stack to avoid cascading failures
    unsafe {
        idt.double_fault.set_handler_fn(double_fault_handler).set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
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

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: DIVIDE BY ZERO\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    panic!("EXCEPTION: INVALID OPCODE\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn general_protection_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) {
    panic!("EXCEPTION: GENERAL PROTECTION FAULT (error code: {})\n{:#?}", error_code, stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{stack_frame:#?}\n CODE{error_code}");
}

extern "x86-interrupt" fn page_fault_handler(stack_frame: InterruptStackFrame, error_code: PageFaultErrorCode) {
    use x86_64::registers::control::{Cr2, Cr3};

    println!("EXCEPTION: PAGE FAULT");
    let addr = match Cr2::read() {
        Ok(v) => v,
        Err(e) => {
            println!("Failed to read CR2: {:?}", e);
            VirtAddr::new(0)
        }
    };
    println!("Accessed Address: {:?}", addr);
    println!("Error Code: {:?}", error_code);

    // Print current CR3 (P4 frame physical start)
    let (p4_frame, _) = Cr3::read();
    println!("CR3 P4 frame: {:?}", p4_frame.start_address());

    // Attempt to translate the faulting address using the active mapper
    let mapping = unsafe { crate::memory::active_page_table() };
    match mapping.translate_addr(addr) {
        Some(pa) => println!("Address mapped to physical address: {:?}", pa),
        None => println!("Address is not mapped to any physical frame"),
    }

    // Use the new debug_page_walk function for diagnostics
    crate::memory::debug_page_walk(addr);

    println!("{:#?}", stack_frame);
    panic!("Page fault!");
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    TIMER_TICKS.fetch_add(1, Ordering::Relaxed);

    unsafe {
        // EOI to master PIC (port 0x20)
        use x86_64::instructions::port::Port;
        let mut port = Port::<u8>::new(0x20);
        port.write(0x20);
    }
}
extern "x86-interrupt" fn syscall_handler(_stack_frame: InterruptStackFrame) {
    use crate::core::interrupts::pic::PICS;

    // TODO: Implement syscall handling
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Syscall.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use crate::devices::drivers::ps2_keyboard;
    use crate::core::interrupts::pic::PICS;
    let mut port = Port::<u8>::new(0x60);
    let sc = unsafe { port.read() };

    // Log scancode received from hardware
    println!("IRQ: keyboard scancode {:#x}", sc);

    // enqueue (lock-free SPSC)
    ps2_keyboard::enqueue_scancode(sc);

    // end of interrupt — notify PIC(s)
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let byte: u8 = unsafe { Port::new(0x60).read() };
    crate::devices::drivers::ps2_mouse::enqueue_mouse_byte(byte);

    unsafe {
        // EOI to both PICs (mouse is on secondary PIC)
        Port::<u8>::new(0xA0).write(0x20); // Secondary PIC EOI
        Port::<u8>::new(0x20).write(0x20); // Master PIC EOI
    }
}

