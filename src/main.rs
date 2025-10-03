#![no_std]
#![no_main]

extern crate rlibc;

use bootloader_api::{entry_point, BootInfo};
use core::fmt::Write;
use core::panic::PanicInfo;
use spin::Mutex;
use uart_16550::SerialPort;

use crate::framebuffer::{init_framebuffer, Color, Rect, FRAMEBUFFER};
mod framebuffer;
mod framebuffer_ext;
mod terminal;
mod format;
mod data_structures;
mod alloc;
entry_point!(kernel_main);

pub static SERIAL: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3F8) });

pub fn serial_write(s: &str) {
    let mut serial = SERIAL.lock();
    serial.init();
    let _ = write!(serial, "{}", s);
}

pub fn kernel_main(_boot_info: &'static mut BootInfo) -> ! {
    serial_write("Hello BIOS kernel (serial)!\n");
    init_framebuffer(_boot_info);
    let mut guard = FRAMEBUFFER.lock();
    let fb = guard.as_mut().expect("framebuffer not initialized");

    let info = fb.info();
    let _ = write!(fb, "Framebuffer demo\n");
    let _ = write!(
        fb,
        "Res: {}x{}  stride: {}  bpp: {}\n",
        info.width, info.height, info.stride, info.bytes_per_pixel
    );

    fb.draw_border(Color::GREEN);
    let cell_w = 8usize;
    let cell_h = 8usize;
    let box_rect = Rect::new(2 * cell_w, 4 * cell_h, 40 * cell_w, 6 * cell_h);
    fb.draw_box(box_rect, Color::CYAN, Some(" Demo Box "));

    fb.move_to(3, 5);
    fb.write_colored("White on black\n", Color::WHITE, Color::BLACK);
    fb.write_colored("Yellow on Blue\n", Color::YELLOW, Color::BLUE);

    fb.move_to(3, 8);
    fb.set_cursor_visible(true);
    let _ = write!(fb, "Cursor visible -> ");
    fb.set_cursor_visible(false);
    let _ = write!(fb, "hidden\n");

    let status_x = fb.cols().saturating_sub(20);
    let status_y = fb.rows().saturating_sub(3);
    fb.move_to(status_x, status_y);
    fb.write_colored("STATUS: OK", Color::GREEN, Color::BLACK);

    // Leave the system in an idle loop
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
