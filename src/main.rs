#![no_std]
#![no_main]
#![feature(slice_pattern, abi_x86_interrupt, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;
use spin::Mutex;
use uart_16550::SerialPort;

use crate::{
    drivers::ps2_keyboard,
    framebuffer::framebuffer::{init_framebuffer, FRAMEBUFFER},
    kernel::init_kernel,
    terminal::Terminal,
    ui::Theme,
};
mod allocators;
mod data_structures;
mod drivers;
mod framebuffer;
mod input;
mod interrupts;
mod kernel;
mod memory;
mod terminal;
mod ui;

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
    unsafe {
        if let Err(e) = memory::init_heap(&boot_info) {
            println!("Failed to init heap: {}\n", e);
            loop_arch_mm();
        }
    }

    memory::memory_stats(&boot_info);

    if init_kernel().is_err() {
        println!("Kernel initialization failed!");
        loop_arch_mm();
    }

    init_framebuffer(boot_info);
    let theme = Theme::dark_modern();

    let (fb_width, fb_height) = {
        let guard = FRAMEBUFFER.lock();
        let fb = guard.as_ref().unwrap();
        (fb.width, fb.height)
    };

    let cols = fb_width / 10; 
    let rows = fb_height / 20; 

    let mut term = Terminal::new(cols, rows, &theme);

    {
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().unwrap();
        fb.clear(theme.background);
    }

    term.write("\x1b[1;32mWelcome to RustOS!\x1b[0m\n");
    term.write("Hardware interrupts: \x1b[32mEnabled\x1b[0m\n");
    term.write("\n> ");

    {
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().unwrap();
        term.render(fb);
    }

    let mut decoder = ps2_keyboard::ScancodeDecoder::new();

    loop {
        while let Some(scancode) = ps2_keyboard::dequeue_scancode() {
            if let Some(key) = decoder.process_scancode(scancode) {
                let ch = key.character;

                if key.ctrl && ch == 'c' {
                    term.write("\n^C\n> ");
                } else if key.ctrl && ch == 'l' {
                    term.clear();
                    term.write("> ");
                } else if ch == '\n' {
                    term.write("\n> ");
                } else if ch == '\x08' {
                    term.write("\x08");
                } else {
                    let mut buf = [0u8; 4];
                    term.write(ch.encode_utf8(&mut buf));
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
