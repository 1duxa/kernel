use crate::data_structures::vec::{String, ToString};
use crate::format_no_std;
use crate::framebuffer::framebuffer::{Color, FramebufferWriter, Point, Rect};
use crate::ui::theme::Theme;

/// Draw a card/panel with shadow and rounded corners (modern UI style)
pub fn draw_card(fb: &mut FramebufferWriter, rect: Rect, theme: &Theme, has_shadow: bool) {
    if has_shadow {
        fb.draw_shadow(rect, 4, 2, 2, Color::BLACK);
    }
    
    let radius = 8.min(rect.width / 10).min(rect.height / 10);
    fb.fill_rounded_rect(rect, radius, theme.surface);
    fb.draw_rounded_rect(rect, radius, theme.text_secondary.darken(0.5));
}

/// Simple UI Widget system
#[derive(Debug, Clone)]
pub struct Widget {
    pub rect: Rect,
    pub background: Color,
    pub border_color: Option<Color>,
    pub text: String,
    pub text_color: Color,
    pub visible: bool,
    pub widget_type: WidgetType,
    pub hovered: bool,
    pub theme: Option<Theme>,
    pub radius: usize,
    pub has_shadow: bool,
}

#[derive(Debug, Clone)]
pub enum WidgetType {
    Label,
    Button { 
        pressed: bool,
        hover_brightness: f32,  // How much to brighten on hover (0.0-1.0)
    },
    TextBox { 
        focused: bool, 
        cursor_pos: usize,
        placeholder: String,
    },
    Panel { 
        border_width: usize,
    },
    ProgressBar { 
        value: f32, 
        max: f32,
        show_percentage: bool,
        bar_color: Option<Color>,
    },
    Card {
        elevation: usize,  // Shadow elevation
    },
}

impl Widget {
    pub fn new_label(rect: Rect, text: &str, color: Color) -> Self {
        Self {
            rect,
            background: Color::BLACK,
            border_color: None,
            text: text.to_string(),
            text_color: color,
            visible: true,
            widget_type: WidgetType::Label,
            hovered: false,
            theme: None,
            radius: 0,
            has_shadow: false,
        }
    }

    pub fn new_button(rect: Rect, text: &str, theme: Option<Theme>) -> Self {
        let bg = theme.as_ref().map(|t| t.primary).unwrap_or(Color::DARK_GRAY);
        Self {
            rect,
            background: bg,
            border_color: None,
            text: text.to_string(),
            text_color: theme.as_ref().map(|t| t.text).unwrap_or(Color::WHITE),
            visible: true,
            widget_type: WidgetType::Button { 
                pressed: false,
                hover_brightness: 0.2,
            },
            hovered: false,
            theme,
            radius: 6,
            has_shadow: true,
        }
    }

    pub fn new_button_modern(rect: Rect, text: &str, theme: &Theme) -> Self {
        Self::new_button(rect, text, Some(*theme))
    }

    pub fn new_progress_bar(rect: Rect, value: f32, max: f32, theme: Option<Theme>) -> Self {
        Self {
            rect,
            background: theme.as_ref().map(|t| t.surface.darken(0.3)).unwrap_or(Color::DARK_GRAY),
            border_color: None,
            text: String::new(),
            text_color: theme.as_ref().map(|t| t.text).unwrap_or(Color::WHITE),
            visible: true,
            widget_type: WidgetType::ProgressBar { 
                value, 
                max,
                show_percentage: true,
                bar_color: theme.as_ref().map(|t| t.primary),
            },
            hovered: false,
            theme,
            radius: 4,
            has_shadow: false,
        }
    }

    pub fn new_card(rect: Rect, theme: &Theme, elevation: usize) -> Self {
        Self {
            rect,
            background: theme.surface,
            border_color: None,
            text: String::new(),
            text_color: theme.text,
            visible: true,
            widget_type: WidgetType::Card { elevation },
            hovered: false,
            theme: Some(*theme),
            radius: 8,
            has_shadow: true,
        }
    }

    pub fn new_text_box(rect: Rect, placeholder: &str, theme: Option<Theme>) -> Self {
        Self {
            rect,
            background: theme.as_ref().map(|t| t.surface).unwrap_or(Color::BLACK),
            border_color: theme.as_ref().map(|t| Some(t.text_secondary)).unwrap_or(Some(Color::LIGHT_GRAY)),
            text: String::new(),
            text_color: theme.as_ref().map(|t| t.text).unwrap_or(Color::WHITE),
            visible: true,
            widget_type: WidgetType::TextBox {
                focused: false,
                cursor_pos: 0,
                placeholder: placeholder.to_string(),
            },
            hovered: false,
            theme,
            radius: 4,
            has_shadow: false,
        }
    }

    pub fn render(&self, fb: &mut FramebufferWriter) {
        if !self.visible {
            return;
        }

        match &self.widget_type {
            WidgetType::Label => {
                // Draw background then text at top-left of widget rect
                if self.radius > 0 {
                    fb.fill_rounded_rect(self.rect, self.radius, self.background);
                } else {
                    fb.fill_rect(self.rect, self.background);
                }
                let text_col = self.rect.x / fb.cell_w;
                let text_row = self.rect.y / fb.cell_h;
                let old_fg = fb.fg_color;
                fb.set_fg_color(self.text_color);
                fb.write_at(&self.text, text_col, text_row);
                fb.set_fg_color(old_fg);
            }
            WidgetType::Button { pressed, hover_brightness } => {
                let bg_color = if *pressed {
                    self.background.darken(0.3)
                } else if self.hovered {
                    self.background.lighten(*hover_brightness)
                } else {
                    self.background
                };

                // Draw shadow if enabled
                if self.has_shadow && !*pressed {
                    fb.draw_shadow(self.rect, 2, 1, 1, Color::BLACK);
                }

                // Draw button with rounded corners
                if self.radius > 0 {
                    fb.fill_rounded_rect(self.rect, self.radius, bg_color);
                    if let Some(border) = self.border_color {
                        fb.draw_rounded_rect(self.rect, self.radius, border);
                    }
                } else {
                    fb.fill_rect(self.rect, bg_color);
                    if let Some(border) = self.border_color {
                        fb.draw_rect(self.rect, border);
                    }
                }

                // Center text in button (character coords)
                let text_w_px = self.text.len() * fb.cell_w;
                let text_h_px = fb.cell_h;
                let text_x_px = self.rect.x + (self.rect.width.saturating_sub(text_w_px) / 2);
                let text_y_px = self.rect.y + (self.rect.height.saturating_sub(text_h_px) / 2);
                let old_fg = fb.fg_color;
                fb.set_fg_color(self.text_color);
                fb.write_at(
                    &self.text,
                    text_x_px / fb.cell_w,
                    text_y_px / fb.cell_h,
                );
                fb.set_fg_color(old_fg);
            }
            WidgetType::ProgressBar { value, max, show_percentage, bar_color } => {
                // Background with rounded corners
                if self.radius > 0 {
                    fb.fill_rounded_rect(self.rect, self.radius, self.background);
                } else {
                    fb.fill_rect(self.rect, self.background);
                }

                // Progress fill
                let progress = (*value / *max).min(1.0).max(0.0);
                let fill_width = (self.rect.width as f32 * progress) as usize;
                if fill_width > 0 {
                    let fill_rect = Rect::new(self.rect.x, self.rect.y, fill_width, self.rect.height);
                    let bar_col = bar_color.unwrap_or(Color::from_hex(0x4CAF50));
                    if self.radius > 0 {
                        fb.fill_rounded_rect(fill_rect, self.radius.min(fill_width / 2), bar_col);
                    } else {
                        fb.fill_rect(fill_rect, bar_col);
                    }
                }

                // Border
                if let Some(border) = self.border_color {
                    if self.radius > 0 {
                        fb.draw_rounded_rect(self.rect, self.radius, border);
                    } else {
                        fb.draw_rect(self.rect, border);
                    }
                }

                // Progress text centered if enabled
                if *show_percentage {
                    let mut buf = [0u8; 16];
                    let progress_text = format_no_std!(&mut buf, "{:.0}%", progress * 100.0).unwrap_or_default();
                    let text_w_px = progress_text.len() * fb.cell_w;
                    let text_h_px = fb.cell_h;
                    let text_x_px = self.rect.x + (self.rect.width.saturating_sub(text_w_px) / 2);
                    let text_y_px = self.rect.y + (self.rect.height.saturating_sub(text_h_px) / 2);
                    let old_fg = fb.fg_color;
                    fb.set_fg_color(self.text_color);
                    fb.write_at(
                        &progress_text,
                        text_x_px / fb.cell_w,
                        text_y_px / fb.cell_h,
                    );
                    fb.set_fg_color(old_fg);
                }
            }
            WidgetType::TextBox { focused, cursor_pos: _, placeholder } => {
                // Draw background and border, then text starting at inner padding
                if self.radius > 0 {
                    fb.fill_rounded_rect(self.rect, self.radius, self.background);
                } else {
                    fb.fill_rect(self.rect, self.background);
                }
                
                let border_col = if *focused {
                    self.theme.as_ref().map(|t| t.primary).unwrap_or(Color::from_hex(0x2196F3))
                } else {
                    self.border_color.unwrap_or(Color::GRAY)
                };
                
                if self.radius > 0 {
                    fb.draw_rounded_rect(self.rect, self.radius, border_col);
                } else {
                    fb.draw_rect(self.rect, border_col);
                }
                
                let text_col = (self.rect.x + 2) / fb.cell_w;
                let text_row = (self.rect.y + 2) / fb.cell_h;
                let old_fg = fb.fg_color;
                
                if self.text.is_empty() && !placeholder.is_empty() {
                    fb.set_fg_color(self.text_color.darken(0.5));
                    fb.write_at(placeholder, text_col, text_row);
                } else {
                    fb.set_fg_color(self.text_color);
                    fb.write_at(&self.text, text_col, text_row);
                }
                fb.set_fg_color(old_fg);
            }
            WidgetType::Panel { border_width } => {
                if self.has_shadow {
                    fb.draw_shadow(self.rect, 4, 2, 2, Color::BLACK);
                }
                
                if self.radius > 0 {
                    fb.fill_rounded_rect(self.rect, self.radius, self.background);
                } else {
                    fb.fill_rect(self.rect, self.background);
                }
                
                if let Some(border) = self.border_color {
                    if *border_width > 1 {
                        // Draw multiple border lines for thicker border
                        for i in 0..*border_width {
                            let inset = i;
                            let border_rect = Rect::new(
                                self.rect.x + inset,
                                self.rect.y + inset,
                                self.rect.width.saturating_sub(inset * 2),
                                self.rect.height.saturating_sub(inset * 2),
                            );
                            if self.radius > inset {
                                fb.draw_rounded_rect(border_rect, self.radius - inset, border);
                            } else {
                                fb.draw_rect(border_rect, border);
                            }
                        }
                    } else {
                        if self.radius > 0 {
                            fb.draw_rounded_rect(self.rect, self.radius, border);
                        } else {
                            fb.draw_rect(self.rect, border);
                        }
                    }
                }
            }
            WidgetType::Card { elevation } => {
                if let Some(theme) = &self.theme {
                    draw_card(fb, self.rect, theme, self.has_shadow && *elevation > 0);
                } else {
                    if self.radius > 0 {
                        fb.fill_rounded_rect(self.rect, self.radius, self.background);
                    } else {
                        fb.fill_rect(self.rect, self.background);
                    }
                }
            }
        }
    }

    /// Check if a point is inside the widget
    pub fn contains(&self, point: Point) -> bool {
        self.rect.contains(point)
    }

    /// Set hover state
    pub fn set_hovered(&mut self, hovered: bool) {
        self.hovered = hovered;
    }
}

