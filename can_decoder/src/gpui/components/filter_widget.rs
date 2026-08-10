use gpui::{div, prelude::*, px, FocusHandle, IntoElement, Render, SharedString, Styled, Window};

use std::sync::Arc;

use can_decoder::filter_editor::FieldType;

pub struct FilterWidget {
    field_type: FieldType,
    enabled: bool,
    input_text: String,
    focus_handle: FocusHandle,
    label: SharedString,
}

impl FilterWidget {
    pub fn new(field_type: FieldType, cx: &mut Context<Self>) -> Self {
        let label = SharedString::from(Arc::from(field_type.label()));
        Self {
            field_type,
            enabled: false,
            input_text: String::new(),
            focus_handle: cx.focus_handle(),
            label,
        }
    }

    pub fn field_type(&self) -> FieldType {
        self.field_type
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.enabled != enabled {
            self.enabled = enabled;
            cx.notify();
        }
    }

    pub fn input_text(&self) -> &str {
        &self.input_text
    }

    pub fn set_input_text(&mut self, text: String, cx: &mut Context<Self>) {
        if self.input_text != text {
            self.input_text = text;
            cx.notify();
        }
    }

    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

impl Render for FilterWidget {
    fn render(&mut self, _window: &mut Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let enabled = self.enabled;
        let input_text = self.input_text.clone();
        let label = self.label.clone();

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
                                gpui::rgb(0x666666)
                            })
                            .child(label),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .h(px(22.0))
                    .border_1()
                    .border_color(gpui::rgb(0x444466))
                    .rounded(px(3.0))
                    .bg(gpui::rgb(0x1a1a2e))
                    .overflow_hidden()
                    .when(enabled, |this| this)
                    .when(!enabled, |this| this.opacity(0.5).cursor_not_allowed())
                    .child(
                        div()
                            .w_full()
                            .h_full()
                            .px(px(6.0))
                            .py(px(2.0))
                            .text_xs()
                            .font_family("monospace")
                            .text_color(gpui::rgb(0xcccccc))
                            .when(!enabled, |this| this.text_color(gpui::rgb(0x666666)))
                            .child(input_text),
                    ),
            )
    }
}
