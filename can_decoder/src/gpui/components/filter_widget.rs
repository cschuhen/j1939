use gpui::{
    div, prelude::*, px, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Render, SharedString, Styled, Window,
};

use std::sync::Arc;

use can_decoder::filter_editor::FieldType;

pub struct FilterWidget {
    field_type: FieldType,
    input_text: String,
    cursor_pos: usize,
    enabled: bool,
    focus_handle: FocusHandle,
    label: SharedString,
}

impl FilterWidget {
    pub fn new(field_type: FieldType, cx: &mut Context<Self>) -> Self {
        let label = SharedString::from(Arc::from(field_type.label()));
        Self {
            field_type,
            input_text: String::new(),
            cursor_pos: 0,
            enabled: true,
            focus_handle: cx.focus_handle(),
            label,
        }
    }

    pub fn toggle_enabled(&mut self, cx: &mut Context<Self>) {
        self.enabled = !self.enabled;
        cx.notify();
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn field_type(&self) -> FieldType {
        self.field_type
    }

    pub fn input_text(&self) -> &str {
        &self.input_text
    }

    pub fn set_input_text(&mut self, text: String, cx: &mut Context<Self>) {
        let len = text.len();
        if self.input_text != text {
            self.input_text = text;
            self.cursor_pos = len;
            cx.notify();
        }
    }

    pub fn append_char(&mut self, ch: char, cx: &mut Context<Self>) {
        if !self.enabled {
            return;
        }
        let pos = self.cursor_pos.min(self.input_text.len());
        self.input_text.insert(pos, ch);
        self.cursor_pos += 1;
        cx.notify();
    }

    pub fn delete_char(&mut self, cx: &mut Context<Self>) {
        if !self.enabled || self.input_text.is_empty() {
            return;
        }
        let pos = self.cursor_pos.min(self.input_text.len());
        if pos > 0 {
            self.input_text.remove(pos - 1);
            self.cursor_pos -= 1;
        } else if pos < self.input_text.len() {
            self.input_text.remove(pos);
        }
        cx.notify();
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }
}

impl Render for FilterWidget {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let label = self.label.clone();
        let input_text = self.input_text.clone();
        let enabled = self.enabled;
        let cursor_pos = self.cursor_pos;

        div()
            .flex_col()
            .w_full()
            .mb(px(8.0))
            .child(
                div()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .h(px(24.0))
                    .px(px(4.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(if enabled {
                                gpui::rgb(0x88cc88)
                            } else {
                                gpui::rgb(0x555577)
                            })
                            .child(label),
                    )
                    .child(
                        div()
                            .w_8()
                            .h_5()
                            .justify_center()
                            .items_center()
                            .rounded(px(3.0))
                            .cursor_pointer()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(if enabled {
                                gpui::rgb(0x88cc88)
                            } else {
                                gpui::rgb(0x775555)
                            })
                            .bg(if enabled {
                                gpui::rgb(0x1a3a2e)
                            } else {
                                gpui::rgb(0x3a1a1a)
                            })
                            .hover(|this| {
                                this.bg(gpui::rgb(0x2a4a3e))
                                    .text_color(gpui::rgb(0xaaddaa))
                            })
                            .child(if enabled { "ON" } else { "OFF" })
                            .on_mouse_down(MouseButton::Left, cx.listener(move |this, _event, _window, cx| {
                                this.toggle_enabled(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .h(px(22.0))
                    .border_1()
                    .border_color(if enabled {
                        gpui::rgb(0x444466)
                    } else {
                        gpui::rgb(0x333344)
                    })
                    .rounded(px(3.0))
                    .bg(if enabled {
                        gpui::rgb(0x1a1a2e)
                    } else {
                        gpui::rgb(0x111122)
                    })
                    .overflow_hidden()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _event, window, cx| {
                        this.focus_handle.focus(window, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _window, cx| {
                        let key = event.keystroke.key.as_str();
                        if key == "backspace" {
                            this.delete_char(cx);
                        } else if key.len() == 1 && !event.keystroke.modifiers.secondary() {
                            if let Some(ch) = key.chars().next() {
                                if ch.is_ascii_graphic() || ch == ' ' {
                                    this.append_char(ch, cx);
                                }
                            }
                        }
                    }))
                    .child(
                        div()
                            .w_full()
                            .h_full()
                            .px(px(6.0))
                            .py(px(2.0))
                            .text_xs()
                            .font_family("monospace")
                            .text_color(if enabled {
                                gpui::rgb(0xcccccc)
                            } else {
                                gpui::rgb(0x555577)
                            })
                            .child(if input_text.is_empty() {
                                format!("Filter by {}", self.field_type.label())
                            } else {
                                let display = if cursor_pos < input_text.len() {
                                    format!("{}|{}", &input_text[..cursor_pos], &input_text[cursor_pos..])
                                } else {
                                    format!("{}|", input_text)
                                };
                                display
                            }),
                    ),
            )
    }
}
