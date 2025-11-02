#![no_std]
#![no_main]
#![feature(slice_pattern, abi_x86_interrupt, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;
use spin::Mutex;
use uart_16550::SerialPort;

use crate::{drivers::ps2_keyboard, framebuffer::framebuffer::{FRAMEBUFFER, init_framebuffer}, terminal::Terminal};
mod allocators;
mod data_structures;
mod drivers;
mod format;
mod framebuffer;
mod interrupts;
mod memory;
mod terminal;

entry_point!(kernel_main);

pub static SERIAL: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3F8) });

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let mut serial = $crate::SERIAL.lock();
        let _ = writeln!(serial, $($arg)*);
    }};
}

pub fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    println!("Initializing kernel...\n");

    unsafe {
        if let Err(e) = memory::init_heap(&boot_info) {
            println!("Failed to init heap: {}\n", e);
            loop_arch_mm();
        }
    }

    memory::memory_stats(&boot_info);
    interrupts::init();
    println!("Interrupts initialized.\n");
    init_framebuffer(boot_info);

    let (cols, rows) = {
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().expect("framebuffer not initialized");
        (fb.cols(), fb.rows())
    };

    let mut term = Terminal::new(cols, rows);

    term.write("\x1b[1;32m"); 
    term.write("Welcome to RustOS!\n");
    term.write("\x1b[0m"); 
    term.write("Type something...\n\n> ");

    let mut guard = FRAMEBUFFER.lock();
    let fb = guard.as_mut().unwrap();
    term.render(fb);

    let mut decoder = ps2_keyboard::ScancodeDecoder::new();

    loop {
        if let Some(scancode) = ps2_keyboard::KEYBOARD.lock().read_scancode() {
            if let Some(key_event) = decoder.process_scancode(scancode) {
                // Handle special keys
                if key_event.ctrl && key_event.character == 'c' {
                    term.write("\n^C\n> ");
                } else if key_event.ctrl && key_event.character == 'l' {
                    // Clear screen
                    term.write("\x1b[2J\x1b[H");
                    term.write("> ");
                } else {
                    // Echo character
                    let ch = key_event.character;

                    if ch == '\n' {
                        term.write("\n> ");
                    } else if ch == '\x08' {
                        // Backspace - erase character
                        term.write("\x08");
                    } else {
                        // Regular character
                        let mut buf = [0u8; 4];
                        let s = ch.encode_utf8(&mut buf);
                        term.write(s);
                    }
                }

                let mut guard = FRAMEBUFFER.lock();
                let fb = guard.as_mut().unwrap();
                term.render(fb);
            }
        }

        unsafe {
            core::arch::x86_64::_mm_pause();
        }
    }
}

fn loop_arch_mm() -> ! {
    loop {
        unsafe {
            core::arch::x86_64::_mm_pause();
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let msg = info.message();
    let loc = info.location();
    println!("PANIC : {} | {:?}", msg, loc);

    loop_arch_mm()
}
