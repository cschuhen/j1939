use gpui::prelude::*;
use gpui::{
    div, px, Context, Entity, FocusHandle, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Render, SharedString, Styled, Window,
};

use super::message_list::MessageFilter;
use can_decoder::filter_editor::FieldType;

/// Filter sections that can be expanded/collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterSection {
    SourceAddr,
    DestAddr,
    Pgn,
    Title,
    SourceName,
    DestName,
}

impl FilterSection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::SourceAddr => "Source Addr",
            Self::DestAddr => "Dest Addr",
            Self::Pgn => "PGN",
            Self::Title => "Title",
            Self::SourceName => "Src NAME",
            Self::DestName => "Dst NAME",
        }
    }

    pub fn field_type(&self) -> FieldType {
        match self {
            Self::SourceAddr => FieldType::SourceAddr,
            Self::Pgn => FieldType::Pgn,
            _ => FieldType::SourceAddr,
        }
    }

    /// Return all filter sections in display order.
    pub fn all_sections() -> &'static [Self] {
        &[
            Self::SourceAddr,
            Self::DestAddr,
            Self::Pgn,
            Self::Title,
            Self::SourceName,
            Self::DestName,
        ]
    }
}

/// Items in the flat filter list.
#[derive(Debug, Clone)]
pub enum FilterItem {
    SectionHeader(FilterSection),
    FilterOption(FilterOptionData),
}

/// Data for a filter option (a unique value seen in messages).
#[derive(Debug, Clone)]
pub struct FilterOptionData {
    pub section: FilterSection,
    pub label: SharedString,
    pub raw_value: u64,
}

/// State for the filter panel.
pub struct FilterPanel {
    focus_handle: FocusHandle,
    expanded_sections: Vec<FilterSection>,
    active_filters: Vec<(FilterSection, u64)>,
    selected_titles: Vec<String>,
    expanded_name_details: Option<u64>,
    items: Vec<FilterItem>,
    message_list: Option<Entity<super::message_list::MessageList>>,

    // Incremental state tracking - updated per-message instead of rescanning all messages
    source_addrs: std::collections::BTreeSet<u8>,
    dest_addrs: std::collections::BTreeSet<u8>,
    pgns: std::collections::BTreeSet<u32>,
    titles: std::collections::BTreeSet<String>,
    source_names: std::collections::BTreeSet<u64>,
    dest_names: std::collections::BTreeSet<u64>,
}

impl FilterPanel {
    pub fn new(
        message_list: Entity<super::message_list::MessageList>,
        cx: &mut Context<Self>,
    ) -> Self {
        let items = vec![];
        Self {
            focus_handle: cx.focus_handle(),
            expanded_sections: vec![
                FilterSection::SourceAddr,
                FilterSection::DestAddr,
                FilterSection::Pgn,
                FilterSection::Title,
                FilterSection::SourceName,
                FilterSection::DestName,
            ],
            active_filters: Vec::new(),
            selected_titles: Vec::new(),
            expanded_name_details: None,
            items,
            message_list: Some(message_list),

            source_addrs: std::collections::BTreeSet::new(),
            dest_addrs: std::collections::BTreeSet::new(),
            pgns: std::collections::BTreeSet::new(),
            titles: std::collections::BTreeSet::new(),
            source_names: std::collections::BTreeSet::new(),
            dest_names: std::collections::BTreeSet::new(),
        }
    }

    /// Add a single message to the incremental filter state.
    pub fn add_message(&mut self, msg: &can_decoder::types::DecodedMessage) {
        self.source_addrs.insert(msg.source_address());
        self.dest_addrs.insert(msg.dest_address());
        self.pgns.insert(msg.pgn());
        self.titles.insert(msg.title.clone());
        if let Some(name) = msg.source_name() {
            self.source_names.insert(name);
        }
        if let Some(name) = msg.dest_name() {
            self.dest_names.insert(name);
        }
    }

    /// Rebuild items list from incremental state (only when sections expand/collapse).
    pub fn rebuild_items(&mut self) {
        self.items.clear();

        for section in FilterSection::all_sections() {
            self.items.push(FilterItem::SectionHeader(*section));

            if !self.expanded_sections.contains(&section) {
                continue;
            }

            match *section {
                FilterSection::SourceAddr => {
                    for addr in &self.source_addrs {
                        self.items.push(FilterItem::FilterOption(FilterOptionData {
                            section: *section,
                            label: SharedString::from(format!("{}", addr)),
                            raw_value: *addr as u64,
                        }));
                    }
                }
                FilterSection::DestAddr => {
                    for addr in &self.dest_addrs {
                        self.items.push(FilterItem::FilterOption(FilterOptionData {
                            section: *section,
                            label: SharedString::from(format!("{}", addr)),
                            raw_value: *addr as u64,
                        }));
                    }
                }
                FilterSection::Pgn => {
                    for pgn in &self.pgns {
                        self.items.push(FilterItem::FilterOption(FilterOptionData {
                            section: *section,
                            label: SharedString::from(can_decoder::utils::render_pgn(*pgn)),
                            raw_value: *pgn as u64,
                        }));
                    }
                }
                FilterSection::Title => {
                    for title in &self.titles {
                        self.items.push(FilterItem::FilterOption(FilterOptionData {
                            section: *section,
                            label: SharedString::from(title.clone()),
                            raw_value: 0,
                        }));
                    }
                }
                FilterSection::SourceName => {
                    for name in &self.source_names {
                        self.items.push(FilterItem::FilterOption(FilterOptionData {
                            section: *section,
                            label: SharedString::from(can_decoder::utils::render_name(*name)),
                            raw_value: *name,
                        }));
                    }
                }
                FilterSection::DestName => {
                    for name in &self.dest_names {
                        self.items.push(FilterItem::FilterOption(FilterOptionData {
                            section: *section,
                            label: SharedString::from(can_decoder::utils::render_name(*name)),
                            raw_value: *name,
                        }));
                    }
                }
            }
        }
    }

    /// Update filter options from the current messages (initial population or explicit refresh).
    pub fn update_from_messages(&mut self, cx: &mut Context<Self>) {
        let Some(msg_list) = self.message_list.as_ref() else {
            return;
        };

        msg_list.update(cx, |msg_list, _cx| {
            for msg in &msg_list.messages {
                self.source_addrs.insert(msg.source_address());
                self.dest_addrs.insert(msg.dest_address());
                self.pgns.insert(msg.pgn());
                self.titles.insert(msg.title.clone());
                if let Some(name) = msg.source_name() {
                    self.source_names.insert(name);
                }
                if let Some(name) = msg.dest_name() {
                    self.dest_names.insert(name);
                }
            }
        });

        self.rebuild_items();
        cx.notify();
    }

    /// Toggle a section's expanded state.
    pub fn toggle_section(&mut self, section: FilterSection) {
        if let Some(idx) = self.expanded_sections.iter().position(|s| *s == section) {
            self.expanded_sections.remove(idx);
        } else {
            self.expanded_sections.push(section);
        }
    }

    /// Toggle a filter option's active state and propagate to message list.
    pub fn toggle_filter(&mut self, section: FilterSection, raw_value: u64) {
        if let Some(idx) = self
            .active_filters
            .iter()
            .position(|(s, v)| *s == section && *v == raw_value)
        {
            self.active_filters.remove(idx);
        } else {
            self.active_filters.push((section, raw_value));
        }
    }

    /// Toggle a title filter option and propagate to message list.
    pub fn toggle_title_filter(&mut self, title: String) {
        if let Some(idx) = self.selected_titles.iter().position(|t| *t == title) {
            self.selected_titles.remove(idx);
        } else {
            self.selected_titles.push(title);
        }
    }

    /// Build a MessageFilter from active_filters and apply it to the message list.
    pub fn apply_to_message_list(&mut self, cx: &mut Context<Self>) {
        let mut source_addr = None;
        let mut dest_addr = None;
        let mut pgn = None;

        for (section, value) in &self.active_filters {
            match section {
                FilterSection::SourceAddr => source_addr = Some(*value as u8),
                FilterSection::DestAddr => dest_addr = Some(*value as u8),
                FilterSection::Pgn => pgn = Some(*value as u32),
                _ => {}
            }
        }

        if let Some(ref msg_list) = self.message_list {
            msg_list.update(cx, |list, _cx| {
                list.set_filter(MessageFilter {
                    source_addr,
                    dest_addr,
                    pgn,
                    title_contains: self.selected_titles.clone(),
                });
            });
        }
    }

    /// Get the height for a single row in the uniform list.
    fn row_height(&self, item: &FilterItem) -> f32 {
        match item {
            FilterItem::SectionHeader(_) => 24.0,
            FilterItem::FilterOption(_) => 20.0,
        }
    }
}

impl Render for FilterPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().clone();
        div()
            .h_full()
            .w_72()
            .flex_col()
            .bg(gpui::rgb(0x1a1a2e))
            .border_r_1()
            .border_color(gpui::rgb(0x333355))
            .child(
                div()
                    .h_6()
                    .w_full()
                    .bg(gpui::rgb(0x2a2a4e))
                    .flex_row()
                    .items_center()
                    .px_3()
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0xaaaaee))
                            .child(" Filters "),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .p_2()
                    .children(self.render_filter_items(entity, cx)),
            )
    }
}

impl FilterPanel {
    fn render_filter_items(
        &self,
        entity: Entity<Self>,
        _cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let mut elements: Vec<gpui::AnyElement> = Vec::new();

        for item in &self.items {
            match item {
                FilterItem::SectionHeader(section) => {
                    let section_copy = *section;
                    let is_open = self.expanded_sections.contains(&section_copy);
                    let chevron = if is_open { "▼" } else { "▶" };
                    let label = section.label().to_string();
                    let entity = entity.clone();

                    elements.push(
                        div()
                            .flex()
                            .h(px(24.0))
                            .w_full()
                            .flex_row()
                            .items_center()
                            .px_2()
                            .gap(px(4.0))
                            .cursor_pointer()
                            .hover(|this| this.bg(gpui::rgb(0x2a2a3e)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                entity.update(cx, |panel, _cx| {
                                    panel.toggle_section(section_copy);
                                    panel.update_from_messages(_cx);
                                    _cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .w_4()
                                    .text_color(gpui::rgb(0x8888aa))
                                    .child(chevron),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(gpui::rgb(0xccccdd))
                                    .child(label),
                            )
                            .into_any_element(),
                    );
                }
                FilterItem::FilterOption(opt) => {
                    let section = opt.section;
                    let raw_value = opt.raw_value;
                    let label = opt.label.clone();
                    let is_active = if section == FilterSection::Title {
                        self.selected_titles.iter().any(|t| *t == label.as_ref())
                    } else {
                        self.active_filters
                            .iter()
                            .any(|(s, v)| *s == section && *v == raw_value)
                    };
                    let entity = entity.clone();
                    let label_for_click = label.clone();
                    let name_details =
                        if matches!(section, FilterSection::SourceName | FilterSection::DestName) {
                            Some(can_decoder::utils::render_name(raw_value as u64))
                        } else {
                            None
                        };
                    let is_expanded = self.expanded_name_details == Some(raw_value);

                    elements.push(
                        div()
                            .flex()
                            .h(px(20.0))
                            .w_full()
                            .flex_row()
                            .items_center()
                            .px_4()
                            .gap(px(4.0))
                            .cursor_pointer()
                            .hover(|this| this.bg(gpui::rgb(0x2a2a3e)))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                entity.update(cx, |panel, _cx| {
                                    if section == FilterSection::Title {
                                        panel.toggle_title_filter(
                                            label_for_click.as_ref().to_string(),
                                        );
                                    } else {
                                        panel.toggle_filter(section, raw_value);
                                    }
                                    if matches!(
                                        section,
                                        FilterSection::SourceName | FilterSection::DestName
                                    ) {
                                        if panel.expanded_name_details == Some(raw_value) {
                                            panel.expanded_name_details = None;
                                        } else {
                                            panel.expanded_name_details = Some(raw_value);
                                        }
                                    }
                                    panel.apply_to_message_list(_cx);
                                    _cx.notify();
                                });
                            })
                            .child(
                                div()
                                    .w_3()
                                    .h_3()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(if is_active {
                                        gpui::rgb(0x88cc88)
                                    } else {
                                        gpui::rgb(0x444466)
                                    })
                                    .bg(if is_active {
                                        gpui::rgb(0x88cc88)
                                    } else {
                                        gpui::rgb(0x000000)
                                    }),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(if is_active {
                                        gpui::rgb(0x88cc88)
                                    } else {
                                        gpui::rgb(0x9999aa)
                                    })
                                    .child(label),
                            )
                            .into_any_element(),
                    );

                    if let Some(ref details) = name_details {
                        if is_expanded {
                            elements.push(
                                div()
                                    .w_full()
                                    .pl_8()
                                    .py_1()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(gpui::rgb(0xaaaaaa))
                                    .child(details.clone())
                                    .into_any_element(),
                            );
                        }
                    }
                }
            }
        }

        elements
    }
}
