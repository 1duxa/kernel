#![no_std]
#![no_main]
#![feature(slice_pattern, abi_x86_interrupt, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

use alloc::boxed::Box;
use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;
use spin::Mutex;
use uart_16550::SerialPort;

use crate::app::{AppEvent, AppHost};
use crate::apps::terminal_app::TerminalApp;
use crate::{
    drivers::ps2_keyboard,
    framebuffer::framebuffer::{init_framebuffer, FRAMEBUFFER},
    kernel::init_kernel,
    terminal::Terminal,
    ui::Theme,
};
mod allocators;
mod app;
mod apps;
mod data_structures;
mod drivers;
mod framebuffer;
mod input;
mod interrupts;
mod kernel;
mod memory;
mod terminal;
mod ui;
mod syscalls;
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
    let mut host = AppHost::new();
    let term = Terminal::new(cols, rows, &theme);
    let app = TerminalApp::new(term.clone());

    host.register_app(Box::new(app));
    {
        use crate::data_structures::vec::String as KString;
        use crate::ui::widgets::{Label, Panel, Rect, VStack, Widget};
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().unwrap();
        let screen = Rect::new(0, (fb.height/2).try_into().unwrap(), fb.width, fb.height/2);

        fb.clear(theme.background);

        let header_h = 24usize;
        let mut header = Panel {
            rect: Rect::new(0, 0, 0, 0),
            bg: theme.primary,
            radius: Some(8),
        };
        let mut title = Label::new(KString::from("Welcome to RustOS"), theme.text);
        let mut root = VStack::new();
        root.push(&mut header, Some(header_h));
        root.push(&mut title, Some(header_h));
        root.layout(screen);
        root.render(fb, &theme);
        // Layout terminal app
        host.layout_app(
            0,
            Rect::new(
                0,
                (header_h * 2) as i32,
                fb.width,
                fb.height - (header_h * 2),
            ),
        );
        host.app_mut(0).init();
        host.render_app_once(0, fb, &theme);
        fb.render_frame();
        host.app_mut(0).overlay(fb, &theme);
    }
    let mut decoder = ps2_keyboard::ScancodeDecoder::new();

    loop {
        while let Some(scancode) = ps2_keyboard::dequeue_scancode() {
            if let Some(key) = decoder.process_scancode(scancode) {
                let event = if key.is_arrow {
                        AppEvent::KeyPress {
                            ch: if key.arrow_direction.is_some() { '\0' } else { key.character },
                            ctrl: key.ctrl,
                            alt: key.alt,
                            arrow: key.arrow_direction,
                        }
                } else {
                    let ch = key.character;
                    AppEvent::KeyPress {
                        ch,
                        ctrl: key.ctrl,
                        alt: key.alt,
                        arrow: None,
                    }
                };
                let mut guard = FRAMEBUFFER.lock();
                let fb = guard.as_mut().unwrap();
                host.dispatch_event(fb, &theme, event, theme.accent);
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
