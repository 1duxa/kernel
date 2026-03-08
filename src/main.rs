//! # DuxOS Kernel
//!
//! A minimal x86_64 operating system kernel featuring:
//!
//! - Physical frame allocation and virtual memory paging
//! - PS/2 keyboard and mouse drivers with interrupt handling
//! - Framebuffer graphics with terminal emulation
//! - JIT code execution via executable memory mapping
//! - Cooperative task scheduling

#![no_std]
#![no_main]
#![feature(slice_pattern, abi_x86_interrupt, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

use crate::{
    app::{AppEvent, AppHost},
    apps::{logs_app::LogsApp, terminal_app::TerminalApp},
    devices::{
        drivers::{ps2_keyboard, ps2_mouse},
        framebuffer::framebuffer::{init_framebuffer, FRAMEBUFFER},
        mouse_cursor,
    },
    terminal_v2::Terminal,
    ui_provider::{shape::Rect, theme::Theme},
};

use alloc::boxed::Box;
use bootloader_api::{entry_point, BootInfo};
use uart_16550::SerialPort;
use x86_64::registers::control::Cr3;

mod app;
mod apps;
mod asm_executor;
mod cmd_executor;
mod devices;
mod kcore;
mod memory;
mod syscalls;
mod terminal_logger;
mod terminal_v2;
mod test_env;
mod ui_provider;

const BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

pub static mut SERIAL: SerialPort = unsafe { SerialPort::new(0x3F8) };

pub fn kprintln(args: alloc::fmt::Arguments) {
    use alloc::fmt::Write;
    unsafe {
        let _ = crate::SERIAL.write_fmt(args);
    }
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        $crate::kprintln(format_args!($($arg)*));
    }};
}

fn loop_arch_mm() -> ! {
    loop {
        unsafe {
            ::core::arch::x86_64::_mm_pause();
        }
    }
}

#[panic_handler]
fn panic(info: &::core::panic::PanicInfo) -> ! {
    println!("KERNEL PANIC: {}", info);
    loop_arch_mm()
}

#[alloc_error_handler]
fn alloc_error(layout: ::alloc::alloc::Layout) -> ! {
    println!("ALLOC ERROR: {:?}", layout);
    loop_arch_mm()
}

pub fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    use core::arch::asm;
    unsafe {
        if let Err(e) = memory::init(boot_info) {
            println!("PANIC: Failed to init memory: {}", e);
            loop_arch_mm();
        }
    }
    let _ = kcore::kernel::init_kernel();
    println!("4");
    init_framebuffer(boot_info);

    println!("5");
    let theme = Theme::dark_modern();
    let (fb_width, fb_height) = {
        let guard = FRAMEBUFFER.lock();
        let fb = guard.as_ref().unwrap();
        (fb.width, fb.height)
    };

    println!("6");
    let log_cols = fb_width / 10;
    let log_rows = 4;
    terminal_logger::init(log_cols, log_rows, &theme);
    kprintln!("DuxOS Kernel Starting...");

    let cols = fb_width / 10;
    let rows = (fb_height - 60) / 20;
    let mut host = AppHost::new();

    let term = Terminal::new(cols / 2, rows, &theme);
    host.register_app(Box::new(TerminalApp::new(term)));
    host.register_app(Box::new(LogsApp::new(cols / 2, rows, &theme)));

    {
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().unwrap();

        fb.clear(theme.background);
        host.layout_app(0, Rect::new(0, 0, fb.width / 2, fb.height));
        host.layout_app(1, Rect::new(fb.width / 2, 0, fb.width / 2, fb.height));

        host.app_mut(0).init();
        host.app_mut(1).init();

        host.render_app_once(0, fb, &theme);
        host.render_app_once(1, fb, &theme);
        fb.render_frame();
        host.app_mut(0).overlay(fb, &theme);
    }

    let mut decoder = ps2_keyboard::ScancodeDecoder::new();

    if let Err(e) = ps2_mouse::init() {
        println!("PS/2 mouse init failed: {}", e);
    }
    mouse_cursor::init(fb_width, fb_height);

    {
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().unwrap();
        mouse_cursor::draw(fb);
        fb.render_frame();
    }

    let mut last_mouse_buttons: u8 = 0;

    log_info!("Kernel ready");
    log_info!("F1=Terminal, F2=Logs, Shift+Enter=Execute");

    loop {
        let mut had_keyboard_event = false;
        let mut mouse_moved = false;

        while let Some(scancode) = ps2_keyboard::dequeue_scancode() {
            if let Some(key) = decoder.process_scancode(scancode) {
                if !key.character.is_control() {
                    log_debug!("Key: '{}'", key.character);
                }

                match key.character {
                    '\x11' => {
                        host.switch_to_app(0);
                        let mut guard = FRAMEBUFFER.lock();
                        let fb = guard.as_mut().unwrap();
                        host.render_all_apps(fb, &theme);
                        mouse_cursor::draw(fb);
                        fb.render_frame();
                        had_keyboard_event = true;
                        continue;
                    }
                    '\x12' => {
                        host.switch_to_app(1);
                        let mut guard = FRAMEBUFFER.lock();
                        let fb = guard.as_mut().unwrap();
                        host.render_all_apps(fb, &theme);
                        mouse_cursor::draw(fb);
                        fb.render_frame();
                        had_keyboard_event = true;
                        continue;
                    }
                    _ => {}
                }

                if key.alt && !key.ctrl {
                    match key.character {
                        '\t' => {
                            host.cycle_focus();
                            let mut guard = FRAMEBUFFER.lock();
                            let fb = guard.as_mut().unwrap();
                            host.render_all_apps(fb, &theme);
                            mouse_cursor::draw(fb);
                            fb.render_frame();
                            had_keyboard_event = true;
                            continue;
                        }
                        '1'..='9' => {
                            let app_idx = (key.character as usize) - ('1' as usize);
                            if host.switch_to_app(app_idx) {
                                let mut guard = FRAMEBUFFER.lock();
                                let fb = guard.as_mut().unwrap();
                                host.render_all_apps(fb, &theme);
                                mouse_cursor::draw(fb);
                                fb.render_frame();
                            }
                            had_keyboard_event = true;
                            continue;
                        }
                        _ => {}
                    }
                }

                let event = if key.is_arrow {
                    AppEvent::KeyPress {
                        ch: '\0',
                        ctrl: key.ctrl,
                        alt: key.alt,
                        shift: key.shift,
                        arrow: key.arrow_direction,
                    }
                } else {
                    AppEvent::KeyPress {
                        ch: key.character,
                        ctrl: key.ctrl,
                        alt: key.alt,
                        shift: key.shift,
                        arrow: None,
                    }
                };

                let mut guard = FRAMEBUFFER.lock();
                let fb = guard.as_mut().unwrap();
                host.dispatch_event(fb, &theme, event, theme.accent);
                mouse_cursor::draw(fb);
                fb.render_frame();
                had_keyboard_event = true;
            }
        }

        while let Some(mouse_event) = ps2_mouse::poll_mouse_event() {
            mouse_cursor::update_position(mouse_event.dx, mouse_event.dy);
            mouse_moved = true;

            let left_clicked =
                (mouse_event.buttons & 0x01) != 0 && (last_mouse_buttons & 0x01) == 0;
            let right_clicked =
                (mouse_event.buttons & 0x02) != 0 && (last_mouse_buttons & 0x02) == 0;

            if left_clicked || right_clicked || mouse_event.buttons != 0 {
                let (mx, my) = mouse_cursor::get_position();
                let event = AppEvent::Mouse(mouse_event);
                let mut guard = FRAMEBUFFER.lock();
                let fb = guard.as_mut().unwrap();

                if left_clicked {
                    host.handle_mouse_click(mx as usize, my as usize);
                }

                host.dispatch_event(fb, &theme, event, theme.accent);
                mouse_cursor::draw(fb);
                fb.render_frame();
            }

            last_mouse_buttons = mouse_event.buttons;
        }

        if mouse_moved && !had_keyboard_event {
            let mut guard = FRAMEBUFFER.lock();
            let fb = guard.as_mut().unwrap();
            host.render_all_apps(fb, &theme);
            mouse_cursor::draw(fb);
            fb.render_frame();
        }

        unsafe {
            ::core::arch::x86_64::_mm_pause();
        }
    }
}
