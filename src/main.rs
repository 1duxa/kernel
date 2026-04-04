//! # DuxOS Kernel

#![no_std]
#![no_main]
#![feature(slice_pattern, abi_x86_interrupt, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

use crate::{
    app::{AppEvent, AppHost},
    apps::{editor_app::EditorApp, logs_app::LogsApp, terminal_app::TerminalApp},
    devices::{
        drivers::{ps2_keyboard, ps2_mouse},
        framebuffer::framebuffer::{init_framebuffer, FRAMEBUFFER},
        mouse_cursor,
    },
    kcore::interrupts::interrupts::TIMER_TICKS,
    ui_provider::{shape::Rect, theme::Theme},
};

use alloc::{boxed::Box, vec::Vec};
use bootloader_api::{entry_point, BootInfo};
use uart_16550::SerialPort;

mod app;
mod apps;
mod cmd_executor;
mod debug_pipeline;
mod devices;
mod kcore;
mod memory;
mod syscalls;
mod terminal_v2;
mod tests;
mod ui_provider;
mod vm;

const BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config.kernel_stack_size = 128 * 1024;
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

#[derive(Clone, Copy)]
struct UiLayout {
    content_width: usize,
    content_height: usize,
    tab_height: usize,
}

impl UiLayout {
    fn from_framebuffer(width: usize, height: usize) -> Self {
        let tab_height = 38;
        Self {
            content_width: width,
            content_height: height.saturating_sub(tab_height),
            tab_height,
        }
    }

    fn app_bounds(&self) -> Rect {
        Rect::new(0, self.tab_height, self.content_width, self.content_height)
    }

    fn tab_bounds(&self, index: usize) -> Rect {
        let tab_width = self.content_width / 3;
        let x = index * tab_width;
        Rect::new(x, 0, tab_width, self.tab_height)
    }
}

fn framebuffer_size() -> (usize, usize) {
    let guard = FRAMEBUFFER.lock();
    let fb = guard.as_ref().unwrap();
    (fb.width, fb.height)
}

fn draw_tabs(
    fb: &mut crate::devices::framebuffer::framebuffer::FramebufferWriter,
    layout: &UiLayout,
    theme: &Theme,
    focused: usize,
) {
    use crate::ui_provider::{
        render::{RenderCommand, RenderList},
        shape::Rect,
    };

    let tab_names = ["Terminal", "Logs", "Editor"];
    let mut render_list = RenderList::new();

    let margin_x = 10usize;
    let margin_y = 6usize;
    let radius = 10usize;

    for idx in 0..3 {
        let bounds = layout.tab_bounds(idx);
        let is_focused = idx == focused;

        let inner = Rect::new(
            bounds.x + margin_x,
            bounds.y + margin_y,
            bounds.w.saturating_sub(margin_x * 2),
            bounds.h.saturating_sub(margin_y * 2),
        );

        if inner.w < 4 || inner.h < 4 {
            continue;
        }

        let fill_color = if is_focused {
            theme.accent
        } else {
            theme.surface
        };

        let outer = Rect::new(
            inner.x.saturating_sub(1),
            inner.y.saturating_sub(1),
            inner.w + 2,
            inner.h + 2,
        );
        render_list.fill_rounded_rect(outer, radius + 1, theme.border);
        render_list.fill_rounded_rect(inner, radius, fill_color);

        let text_color = if is_focused {
            theme.on_accent
        } else {
            theme.text
        };
        let text_x = inner.x + (inner.w.saturating_sub(tab_names[idx].len() * 10) / 2).max(8);
        let text_y = inner.y + (inner.h.saturating_sub(20) / 2).max(2);
        render_list.push(RenderCommand::text(
            tab_names[idx],
            text_x,
            text_y,
            text_color,
        ));
    }

    render_list.push(RenderCommand::fill_rect(
        Rect::new(
            0,
            layout.tab_height.saturating_sub(1),
            layout.content_width,
            1,
        ),
        theme.border,
    ));

    crate::ui_provider::render::flush_commands(fb, render_list.as_slice());
}

fn init_ui(theme: &Theme, fb_width: usize, fb_height: usize) -> AppHost {
    let layout = UiLayout::from_framebuffer(fb_width, fb_height);
    let mut host = AppHost::new();

    host.register_app(Box::new(TerminalApp::new(
        layout.content_width,
        layout.content_height,
    )));
    host.register_app(Box::new(LogsApp::new(
        layout.content_width,
        layout.content_height,
    )));
    host.register_app(Box::new(EditorApp::new(
        layout.content_width,
        layout.content_height,
    )));

    let app_bounds = layout.app_bounds();
    for idx in 0..3 {
        host.layout_app(idx, app_bounds);
        host.app_mut(idx).init();
    }
    {
        let mut guard = FRAMEBUFFER.lock();
        let fb = guard.as_mut().unwrap();
        fb.clear(theme.background);
        host.compose(theme, theme.accent);
        host.flush(fb);
        // Draw tabs
        draw_tabs(fb, &layout, theme, host.focused_app_index());
        fb.render_frame();
    }

    host
}

fn handle_global_shortcut(host: &mut AppHost, ch: char) -> bool {
    let switched = match ch {
        '\x11' => host.switch_to_app(0), // F1
        '\x12' => host.switch_to_app(1), // F2
        '\x13' => host.switch_to_app(2), // F3
        _ => false,
    };

    if switched {
        host.request_redraw();
    }

    switched
}

fn handle_alt_shortcut(host: &mut AppHost, ch: char, ctrl: bool, alt: bool) -> (bool, bool) {
    if !alt || ctrl {
        return (false, false);
    }

    match ch {
        '\t' => {
            host.cycle_focus();
            (true, true)
        }
        '1'..='9' => {
            let app_idx = (ch as usize) - ('1' as usize);
            let switched = host.switch_to_app(app_idx);
            if switched {
                host.request_redraw();
            }
            (true, switched)
        }
        _ => (false, false),
    }
}

fn key_event_to_app_event(key: ps2_keyboard::KeyEvent) -> AppEvent {
    if key.is_arrow {
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
    }
}

fn collect_pending_events(
    host: &mut AppHost,
    decoder: &mut ps2_keyboard::ScancodeDecoder,
    layout: &UiLayout,
    last_tick: &mut u64,
) -> (Vec<AppEvent>, bool) {
    let mut need_render = false;
    let mut pending_events = Vec::new();

    let current_tick = TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed);
    while *last_tick < current_tick {
        pending_events.push(AppEvent::Tick);
        *last_tick += 1;
    }

    while let Some(mouse_event) = ps2_mouse::poll_mouse_event() {
        mouse_cursor::update_position(mouse_event.dx, -mouse_event.dy);

        if mouse_event.buttons != 0 {
            let (mx, my) = mouse_cursor::get_position();
            if mx >= 0 && my >= 0 {
                let mx = mx as usize;
                let my = my as usize;

                let mut clicked_tab = false;
                for tab_idx in 0..3 {
                    let tab_bounds = layout.tab_bounds(tab_idx);
                    if mx >= tab_bounds.x
                        && mx < tab_bounds.x + tab_bounds.w
                        && my >= tab_bounds.y
                        && my < tab_bounds.y + tab_bounds.h
                    {
                        if tab_idx != host.focused_app_index() {
                            host.switch_to_app(tab_idx);
                            host.request_redraw();
                        }
                        clicked_tab = true;
                        need_render = true;
                        break;
                    }
                }

                // If not on tab, pass to app
                if !clicked_tab {
                    host.handle_mouse_click(mx, my);
                }
            }
        }

        pending_events.push(AppEvent::Mouse(mouse_event));
        need_render = true;
    }

    while let Some(scancode) = ps2_keyboard::dequeue_scancode() {
        if let Some(key) = decoder.process_scancode(scancode) {
            if handle_global_shortcut(host, key.character) {
                need_render = true;
                continue;
            }

            let (handled, switched) = handle_alt_shortcut(host, key.character, key.ctrl, key.alt);
            if handled {
                need_render |= switched || key.character == '\t';
                continue;
            }

            pending_events.push(key_event_to_app_event(key));
            need_render = true;
        }
    }

    (pending_events, need_render)
}

fn render_pending(
    host: &mut AppHost,
    theme: &Theme,
    layout: &UiLayout,
    pending_events: &mut Vec<AppEvent>,
) {
    for ev in pending_events.drain(..) {
        host.dispatch_event(ev);
    }

    let mut guard = FRAMEBUFFER.lock();
    let fb = guard.as_mut().unwrap();

    // Clear full screen to ensure clean rendering
    fb.clear(theme.background);

    // Move unfocused apps off-screen so they don't overlap when rendering
    let focused_idx = host.focused_app_index();
    let content_bounds = layout.app_bounds();
    let off_screen = Rect::new(99999, 99999, 1, 1); // Off-screen bounds

    for idx in 0..3 {
        if idx != focused_idx {
            // Move non-focused apps off-screen
            host.layout_app(idx, off_screen);
        } else {
            // Keep focused app at proper bounds
            host.layout_app(idx, content_bounds);
        }
    }

    // Now render all apps - only focused one will be visible
    host.compose(theme, theme.accent);
    host.flush(fb);

    // Draw tabs on top
    draw_tabs(fb, layout, theme, focused_idx);

    mouse_cursor::draw(fb);

    fb.render_frame();
}

pub fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    unsafe {
        if let Err(e) = memory::init(boot_info) {
            println!("PANIC: Failed to init memory: {}", e);
            loop_arch_mm();
        }
    }

    let _ = kcore::kernel::init_kernel();
    init_framebuffer(boot_info);

    let theme = Theme::dark_modern();
    let (fb_width, fb_height) = framebuffer_size();
    let layout = UiLayout::from_framebuffer(fb_width, fb_height);

    mouse_cursor::init(fb_width, fb_height);

    let mut host = init_ui(&theme, fb_width, fb_height);
    let mut decoder = ps2_keyboard::ScancodeDecoder::new();
    let mut last_tick = TIMER_TICKS.load(core::sync::atomic::Ordering::Relaxed);

    log_info!("Kernel ready");
    log_info!("F1=Terminal, F2=Logs, F3=Editor, Shift+Enter=Execute/Run");

    loop {
        let (mut pending_events, input_requested_redraw) =
            collect_pending_events(&mut host, &mut decoder, &layout, &mut last_tick);

        let debug_requested_redraw = debug_pipeline::is_dirty();
        let cursor_requested_redraw = mouse_cursor::needs_redraw();

        if true {
            render_pending(&mut host, &theme, &layout, &mut pending_events);
        }

        x86_64::instructions::hlt();
    }
}
