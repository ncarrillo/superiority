//! Reforged's channel roster.
//!
//! The information hierarchy and filtering interaction match the SC:R and
//! SC2 rosters. The carved-stone frame, parchment type, portrait treatment,
//! and presence colors remain WC3-owned.

use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, IntoElement, ObjectFit, RenderOnce, Stateful,
    Window, div, img, prelude::*, px, rgb, rgba,
};

use crate::products::wc3::{
    RosterPresence, RosterUser,
    theme::{
        FONT_INTERFACE, GOLD, GOLD_BRIGHT, GOLD_DIM, MUTED, PANEL, PARCHMENT, ROSTER_ROW_HEIGHT,
        ROSTER_SEGMENT_HEIGHT, ROSTER_WIDTH, SHELL, STONE_BRIGHT, STONE_DIM,
    },
};

const DIMMED_ROW_OPACITY: f32 = 0.55;
const DIMMED_ROW_HOVER_OPACITY: f32 = 0.82;
const PORTRAIT_FRAME: f32 = 30.0;
const PORTRAIT_FACE: f32 = 28.0;
const ROW_INSET: f32 = 12.0;
const STATUS_DOT: f32 = 8.0;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type HoverHandler = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RosterHeaderModel {
    pub heading: String,
    pub count: String,
    pub filter_active: bool,
}

impl RosterHeaderModel {
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        total: usize,
        filtered: usize,
        filter: &str,
        focused: bool,
    ) -> Self {
        let filter_active = !filter.is_empty();
        let title = title.into();
        Self {
            heading: if filter_active {
                format!("{title}  /  {filter}")
            } else {
                title
            },
            count: if filter_active {
                format!("{filtered} / {total}")
            } else if focused {
                "type to filter".to_owned()
            } else {
                format!("{total} players")
            },
            filter_active,
        }
    }
}

#[derive(IntoElement)]
pub struct RosterHeader {
    id: String,
    model: RosterHeaderModel,
    focused: bool,
    on_focus: Option<ClickHandler>,
    on_clear: Option<ClickHandler>,
}

impl RosterHeader {
    #[must_use]
    pub fn new(id: impl Into<String>, model: RosterHeaderModel, focused: bool) -> Self {
        Self {
            id: id.into(),
            model,
            focused,
            on_focus: None,
            on_clear: None,
        }
    }

    #[must_use]
    pub fn on_focus(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_focus = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_clear(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_clear = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for RosterHeader {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let has_clear = self.on_clear.is_some();
        let heading_color = if self.focused || self.model.filter_active {
            rgb(GOLD_BRIGHT)
        } else {
            rgb(PARCHMENT)
        };
        let mut header = div()
            .id(self.id)
            .absolute()
            .inset_0()
            .font_family(FONT_INTERFACE)
            .child(
                div()
                    .absolute()
                    .left(px(13.0))
                    .top(px(14.0))
                    .right(px(128.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(12.5))
                    .text_color(heading_color)
                    .child(self.model.heading),
            )
            .child(
                div()
                    .absolute()
                    .right(px(if has_clear { 38.0 } else { 13.0 }))
                    .top(px(16.0))
                    .w(px(110.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .text_size(px(11.0))
                    .text_color(rgb(MUTED))
                    .child(self.model.count),
            );
        if let Some(on_focus) = self.on_focus {
            header = header
                .cursor_pointer()
                .on_click(move |event, window, cx| on_focus(event, window, cx));
        }
        if let Some(on_clear) = self.on_clear {
            header = header.child(
                div()
                    .id("wc3-roster-filter-clear")
                    .absolute()
                    .right(px(11.0))
                    .top(px(10.0))
                    .size(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(14.0))
                    .text_color(rgb(GOLD))
                    .hover(|style| style.text_color(rgb(GOLD_BRIGHT)))
                    .active(|style| style.opacity(0.64))
                    .on_click(move |event, window, cx| on_clear(event, window, cx))
                    .child("×"),
            );
        }
        header
    }
}

#[derive(IntoElement)]
pub struct RosterRow {
    id: String,
    group: String,
    user: RosterUser,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl RosterRow {
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        group: impl Into<String>,
        user: RosterUser,
        selected: bool,
    ) -> Self {
        Self {
            id: id.into(),
            group: group.into(),
            user,
            selected,
            on_click: None,
        }
    }

    #[must_use]
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for RosterRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut row = div()
            .id(self.id)
            .group(self.group.clone())
            .relative()
            .h(px(ROSTER_ROW_HEIGHT))
            .w_full()
            .flex_shrink_0()
            .cursor_pointer();
        if let Some(on_click) = self.on_click {
            row = row.on_click(move |event, window, cx| on_click(event, window, cx));
        }
        let dimmed = self.user.presence.dimmed();
        row.child(selection(self.selected))
            .when(!self.selected, |row| {
                row.child(hover_fill().group_hover(self.group.clone(), |style| style.opacity(1.0)))
            })
            .child(row_body(&self.user).when(dimmed, |body| {
                body.group_hover(self.group, |style| style.opacity(DIMMED_ROW_HOVER_OPACITY))
            }))
    }
}

fn row_body(user: &RosterUser) -> Div {
    div()
        .relative()
        .size_full()
        .flex()
        .items_center()
        .gap(px(9.0))
        .px(px(ROW_INSET))
        .opacity(if user.presence.dimmed() {
            DIMMED_ROW_OPACITY
        } else {
            1.0
        })
        .child(portrait(user))
        .child(name_block(user))
        .child(status_dot(user))
}

fn portrait(user: &RosterUser) -> Div {
    div()
        .relative()
        .size(px(PORTRAIT_FRAME))
        .flex_shrink_0()
        .border_1()
        .border_color(rgb(STONE_BRIGHT))
        .bg(rgb(SHELL))
        .child(
            img(user.portrait.clone())
                .absolute()
                .left(px((PORTRAIT_FRAME - PORTRAIT_FACE) / 2.0))
                .top(px((PORTRAIT_FRAME - PORTRAIT_FACE) / 2.0))
                .size(px(PORTRAIT_FACE))
                .object_fit(ObjectFit::Cover),
        )
}

fn name_block(user: &RosterUser) -> Div {
    let secondary = match (&user.clan_abbreviation, user.presence.detail()) {
        (Some(clan), Some(presence)) => Some(format!("[{clan}] · {presence}")),
        (Some(clan), None) => Some(format!("[{clan}]")),
        (None, Some(presence)) => Some(presence.to_owned()),
        (None, None) => None,
    };
    let block = div().flex_1().min_w_0().flex().flex_col().child(
        div()
            .min_w_0()
            .overflow_hidden()
            .whitespace_nowrap()
            .font_family(FONT_INTERFACE)
            .text_size(px(12.5))
            .text_color(rgb(PARCHMENT))
            .child(user.name.clone()),
    );
    if let Some(detail) = secondary {
        block.child(
            div()
                .font_family(FONT_INTERFACE)
                .text_size(px(10.0))
                .text_color(rgb(if user.clan_abbreviation.is_some() {
                    GOLD_DIM
                } else {
                    user.presence.color()
                }))
                .overflow_hidden()
                .whitespace_nowrap()
                .child(detail),
        )
    } else {
        block
    }
}

fn status_dot(user: &RosterUser) -> Div {
    let color = user.presence.color();
    let dot = div()
        .size(px(STATUS_DOT))
        .flex_shrink_0()
        .rounded(px(STATUS_DOT / 2.0))
        .bg(rgb(color));
    if user.presence == RosterPresence::Online {
        dot.shadow(vec![
            gpui::BoxShadow::new(px(0.0), px(0.0), rgba((color << 8) | 0xb8).into())
                .blur_radius(px(5.0)),
        ])
    } else {
        dot
    }
}

fn selection(selected: bool) -> Div {
    div()
        .absolute()
        .left(px(4.0))
        .right(px(4.0))
        .top(px(1.0))
        .bottom(px(1.0))
        .opacity(if selected { 1.0 } else { 0.0 })
        .bg(rgba(0x3a2d_18eb))
        .border_1()
        .border_color(rgba(0xe8c8_74d8))
}

fn hover_fill() -> Div {
    div()
        .absolute()
        .left(px(4.0))
        .right(px(4.0))
        .top(px(1.0))
        .bottom(px(1.0))
        .opacity(0.0)
        .bg(rgba(0x5e4a_2673))
}

#[must_use]
pub fn segment_header(count: usize) -> Div {
    div()
        .w_full()
        .h(px(ROSTER_SEGMENT_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px(px(ROW_INSET))
        .font_family(FONT_INTERFACE)
        .font_weight(gpui::FontWeight::BOLD)
        .text_size(px(9.5))
        .text_color(rgb(MUTED))
        .child(format!("EVERYONE — {count}"))
}

#[must_use]
pub fn list_layer(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .overflow_hidden()
}

#[derive(IntoElement)]
pub struct RosterPanel {
    header: AnyElement,
    rows: AnyElement,
    overlays: Vec<AnyElement>,
    focused: bool,
    on_hover: Option<HoverHandler>,
}

impl RosterPanel {
    #[must_use]
    pub fn new(header: impl IntoElement, rows: impl IntoElement) -> Self {
        Self {
            header: header.into_any_element(),
            rows: rows.into_any_element(),
            overlays: Vec::new(),
            focused: false,
            on_hover: None,
        }
    }

    #[must_use]
    pub fn overlay(mut self, overlay: impl IntoElement) -> Self {
        self.overlays.push(overlay.into_any_element());
        self
    }

    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub fn on_hover(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for RosterPanel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut panel = div()
            .id("wc3-channel-roster")
            .relative()
            .h_full()
            .w(px(ROSTER_WIDTH))
            .flex_shrink_0()
            .border_l_1()
            .border_color(rgba(0x5e4a_2680))
            .bg(rgb(SHELL))
            .child(
                div()
                    .absolute()
                    .left(px(5.0))
                    .right(px(5.0))
                    .top(px(5.0))
                    .h(px(37.0))
                    .bg(rgb(PANEL)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(5.0))
                    .right(px(5.0))
                    .top(px(41.0))
                    .h(px(1.0))
                    .bg(rgb(STONE_DIM)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .h(px(42.0))
                    .child(self.header),
            )
            .child(
                div()
                    .absolute()
                    .left(px(6.0))
                    .right(px(6.0))
                    .top(px(48.0))
                    .bottom(px(6.0))
                    .overflow_hidden()
                    .bg(rgb(PANEL))
                    .child(self.rows),
            )
            .children(self.overlays)
            .when(self.focused, |panel| {
                panel.child(
                    div()
                        .absolute()
                        .inset_0()
                        .border_1()
                        .border_color(rgb(GOLD)),
                )
            });
        if let Some(on_hover) = self.on_hover {
            panel = panel.on_hover(on_hover);
        }
        panel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_copy_matches_the_shared_roster_interaction() {
        let filtered = RosterHeaderModel::new("Players", 71, 3, "nel", true);
        assert_eq!(filtered.heading, "Players  /  nel");
        assert_eq!(filtered.count, "3 / 71");
        assert!(filtered.filter_active);

        let focused = RosterHeaderModel::new("Players", 71, 71, "", true);
        assert_eq!(focused.count, "type to filter");

        let resting = RosterHeaderModel::new("Players", 71, 71, "", false);
        assert_eq!(resting.count, "71 players");
    }
}
