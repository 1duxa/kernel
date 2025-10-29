#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(generic_const_exprs)]
#![feature(slice_pattern)]

extern crate rlibc;
extern crate alloc; 

use bootloader_api::{entry_point, BootInfo};
use core::fmt::Write;
use core::panic::PanicInfo;
use spin::Mutex;
use uart_16550::SerialPort;

use crate::{framebuffer::{Color, FRAMEBUFFER, Rect, init_framebuffer}, terminal::Terminal};
mod framebuffer;
mod framebuffer_ext;
mod terminal;
mod format;
mod data_structures;
mod allocators;
mod memory;

entry_point!(kernel_main);

pub static SERIAL: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3F8) });

pub fn serial_write(s: &str) {
    let mut serial = SERIAL.lock();
    serial.init();
    let _ = write!(serial, "{}", s);
}

pub fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    serial_write("Initializing kernel...\n");

    {
        let info_ref: &BootInfo = &*boot_info;
        unsafe {
            if let Err(e) = memory::init_heap(info_ref) {
                serial_write(&alloc::format!("Failed to init heap: {}\n", e));
                loop_arch_mm();
            }
        }

        memory::memory_stats(info_ref);
    } 
    {
        let info_mut: &'static mut BootInfo = boot_info;
        init_framebuffer(info_mut);
    } 

    {
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().expect("framebuffer not initialized");

        let cols = fb.cols();
        let rows = fb.rows();
        let mut term = Terminal::new(cols, rows);
        term.write("Hello world\n");
        term.render(fb);
    }

    loop_arch_mm()
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
    use core::fmt::Write;
    let _ = write!(SERIAL.lock(), "PANIC : {} | {:?}", msg, loc);

    loop_arch_mm()
}
