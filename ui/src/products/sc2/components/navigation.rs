use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, Context, Div, Hsla, ImageSource, IntoElement,
    MouseButton, MouseDownEvent, ObjectFit, Render, RenderOnce, ScrollHandle, StyledImage as _,
    TextRun, Window, div, font, img, linear_color_stop, linear_gradient, prelude::*, px, rgb, rgba,
};

pub use crate::products::modal::ModalVariant;
use crate::products::sc2::{
    Sc2Assets,
    theme::{
        FONT_INTERFACE, FONT_NAVIGATION, TAB_BAR_HEIGHT, TAB_HEIGHT, TAB_NAME_INSET, TAB_NAME_LEAD,
        TAB_TOP,
    },
};
use crate::products::wc3::theme::FONT_TITLE as STONE_FONT;

// Reforged's carved-stone strip, from the `Channel Tabs` design: the same
// geometry as StarCraft II's — 182px tabs, the 43px strip, the + cell — in a
// different material. Idle ink → hover → active.
const STONE_INK_IDLE: u32 = 0x008a_745a;
const STONE_INK_HOVER: u32 = 0x00c8_b088;
const STONE_INK_ACTIVE: u32 = 0x00ff_e9a8;
const STONE_GOLD: u32 = 0x00e8_c874;
const STONE_EDGE: u32 = 0x008a_6d3b;

pub use crate::patterns::tabs::{
    TabRelease, TabStripState, marquee_duration, reordered_tab_origins, tab_landing_index,
    tab_origins,
};

#[derive(Clone, Copy, Debug, PartialEq)]
struct TabNameLayout {
    pub uses_marquee: bool,
    pub viewport_width: f32,
    pub travel: f32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChannelTabTone {
    #[default]
    Standard,
    Party,
    Group,
}

#[derive(Clone, Copy)]
pub struct TabDragPayload {
    pub index: usize,
}

pub struct TabDragPreview;

impl Render for TabDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(1.0)).opacity(0.0)
    }
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;
type MouseDownHandler = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>;
type HoverHandler = Box<dyn Fn(&TabHoverEvent, &mut Window, &mut App) + 'static>;

#[derive(Clone, Copy, Debug)]
pub struct TabHoverEvent {
    pub hovered: bool,
    pub travel: f32,
}

pub struct ChannelTab {
    id: String,
    label: String,
    unread: bool,
    active: bool,
    hovered: bool,
    marquee_offset: f32,
    drag_offset: f32,
    dragged_travel: Option<f32>,
    close_progress: Option<f32>,
    effect_opacity: [f32; 3],
    tone: ChannelTabTone,
    on_mouse_down: Option<MouseDownHandler>,
    on_click: Option<ClickHandler>,
    on_hover: Option<HoverHandler>,
    on_close: Option<ClickHandler>,
}

impl ChannelTab {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            unread: false,
            active: false,
            hovered: false,
            marquee_offset: 0.0,
            drag_offset: 0.0,
            dragged_travel: None,
            close_progress: None,
            effect_opacity: [1.0; 3],
            tone: ChannelTabTone::Standard,
            on_mouse_down: None,
            on_click: None,
            on_hover: None,
            on_close: None,
        }
    }

    #[must_use]
    pub const fn unread(mut self, unread: bool) -> Self {
        self.unread = unread;
        self
    }

    #[must_use]
    pub const fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    #[must_use]
    pub const fn hovered(mut self, hovered: bool) -> Self {
        self.hovered = hovered;
        self
    }

    #[must_use]
    pub const fn marquee_offset(mut self, offset: f32) -> Self {
        self.marquee_offset = offset;
        self
    }

    #[must_use]
    pub const fn drag_offset(mut self, offset: f32) -> Self {
        self.drag_offset = offset;
        self
    }

    #[must_use]
    pub const fn dragged_travel(mut self, travel: Option<f32>) -> Self {
        self.dragged_travel = travel;
        self
    }

    #[must_use]
    pub const fn close_progress(mut self, progress: Option<f32>) -> Self {
        self.close_progress = progress;
        self
    }

    #[must_use]
    pub const fn effect_opacity(mut self, opacity: [f32; 3]) -> Self {
        self.effect_opacity = opacity;
        self
    }

    #[must_use]
    pub const fn tone(mut self, tone: ChannelTabTone) -> Self {
        self.tone = tone;
        self
    }

    #[must_use]
    pub fn on_mouse_down(
        mut self,
        handler: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_mouse_down = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_hover(
        mut self,
        handler: impl Fn(&TabHoverEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_hover = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_close(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_close = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn width(&self) -> f32 {
        tab_width(&self.label, self.unread)
    }
}

#[derive(IntoElement)]
pub struct ChannelTabs {
    items: Vec<ChannelTab>,
    assets: Sc2Assets,
    variant: ModalVariant,
    leading: f32,
    compact: bool,
    scroll: Option<ScrollHandle>,
    on_add: Option<ClickHandler>,
}

impl ChannelTabs {
    #[must_use]
    pub fn new(items: Vec<ChannelTab>, assets: Sc2Assets) -> Self {
        Self {
            items,
            assets,
            variant: ModalVariant::Sc2,
            leading: 0.0,
            compact: false,
            scroll: None,
            on_add: None,
        }
    }

    /// the realm the strip is dressed as: StarCraft II's nav plate and cyan
    /// glow line, or Reforged's carved stone and gold underline. Remastered
    /// keeps StarCraft II's material for now.
    #[must_use]
    pub const fn variant(mut self, variant: ModalVariant) -> Self {
        self.variant = variant;
        self
    }

    #[must_use]
    pub const fn leading(mut self, leading: f32) -> Self {
        self.leading = leading;
        self
    }

    #[must_use]
    pub fn compact(mut self, scroll: &ScrollHandle) -> Self {
        self.compact = true;
        self.scroll = Some(scroll.clone());
        self
    }

    #[must_use]
    pub fn on_add(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_add = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn tail(&self) -> f32 {
        self.items.iter().map(ChannelTab::width).sum::<f32>()
            - self
                .items
                .iter()
                .filter_map(|item| item.close_progress.map(|progress| item.width() * progress))
                .sum::<f32>()
    }
}

impl RenderOnce for ChannelTabs {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let variant = self.variant;
        let stone = variant == ModalVariant::Reforged;
        let widths = self.items.iter().map(ChannelTab::width).collect::<Vec<_>>();
        let origins = tab_origins(&widths, 0.0);
        let tail = self.tail();
        let mut regular = Vec::<AnyElement>::with_capacity(self.items.len());
        let mut carried = None;
        let mut closed_before = 0.0;

        for (index, item) in self.items.into_iter().enumerate() {
            let slot_width = widths[index];
            let close_progress = item.close_progress.unwrap_or(0.0).clamp(0.0, 1.0);
            let mut x = origins[index] + item.drag_offset - closed_before;
            let mut width = slot_width;
            let mut opacity = 1.0;
            if close_progress > 0.0 {
                x += slot_width * close_progress / 2.0;
                width *= 1.0 - close_progress;
                opacity = 1.0 - close_progress;
                closed_before += slot_width * close_progress;
            }
            if let Some(travel) = item.dragged_travel {
                x = origins[index] + travel;
            }

            // the stone strip marks unread with a gold stud beside the name,
            // not a bullet in it
            let label = if item.unread && !stone {
                format!("{}  •", item.label.to_uppercase())
            } else {
                item.label.to_uppercase()
            };
            let tint = if stone {
                stone_tab_tint(item.active, item.hovered, item.unread)
            } else {
                tab_text_tint(item.tone, item.active, item.hovered, item.unread)
            };
            let measured = measure_tab_title(&label, window, variant);
            let travel = tab_name_layout(measured, slot_width).travel;
            let mut tab = tab_slot(self.leading + x, width, opacity, variant)
                .id(item.id)
                .cursor_pointer()
                .when(stone && !item.active, |tab| {
                    tab.hover(|style| style.bg(rgba(0xe8c8_740d)))
                })
                .on_drag(TabDragPayload { index }, |_: &TabDragPayload, _, _, cx| {
                    cx.new(|_| TabDragPreview)
                });
            if let Some(handler) = item.on_mouse_down {
                tab = tab.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                    handler(event, window, cx);
                });
            }
            if let Some(handler) = item.on_click {
                tab = tab.on_click(move |event, window, cx| handler(event, window, cx));
            }
            if let Some(handler) = item.on_hover {
                tab = tab.on_hover(move |hovered, window, cx| {
                    handler(
                        &TabHoverEvent {
                            hovered: *hovered,
                            travel,
                        },
                        window,
                        cx,
                    );
                });
            }
            tab = tab.child(TabVisual::new(
                label,
                measured,
                item.marquee_offset,
                tint,
                TabChrome {
                    width: slot_width,
                    active: item.active,
                    divider: index > 0,
                    effect_opacity: item.effect_opacity,
                    tone: item.tone,
                    unread: item.unread,
                    variant,
                },
                self.assets.clone(),
            ));
            if let Some(handler) = item.on_close {
                tab = tab.child(
                    div()
                        .id(("close-channel-tab", index))
                        .absolute()
                        .right(px(6.0))
                        .top(px(if stone { 8.0 } else { 5.0 }))
                        .size(px(26.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.0))
                        .font_family(if stone { STONE_FONT } else { FONT_INTERFACE })
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_size(px(15.0))
                        .text_color(tint)
                        .cursor_pointer()
                        .map(|glyph| {
                            if stone {
                                glyph
                                    .hover(|style| {
                                        style
                                            .bg(rgba(0xe8c8_7426))
                                            .text_color(rgb(STONE_INK_ACTIVE))
                                    })
                                    .active(|style| style.bg(rgba(0xe8c8_7440)).opacity(0.72))
                            } else {
                                glyph
                                    .hover(|style| {
                                        style.bg(rgba(0x315d_8748)).text_color(rgb(0x00ff_ffff))
                                    })
                                    .active(|style| style.bg(rgba(0x315d_8790)).opacity(0.72))
                            }
                        })
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(move |event, window, cx| {
                            cx.stop_propagation();
                            handler(event, window, cx);
                        })
                        .child("×"),
                );
            }
            if item.dragged_travel.is_some() {
                carried = Some(tab.into_any_element());
            } else {
                regular.push(tab.into_any_element());
            }
        }

        let mut strip = tab_strip().children(regular).children(carried);
        if let Some(on_add) = self.on_add {
            if !stone {
                strip = strip.child(
                    img(self.assets.top_navigation_divider.clone())
                        .absolute()
                        .left(px(self.leading + tail))
                        .top(px(TAB_TOP + 2.0))
                        .w(px(1.0))
                        .h(px(32.0))
                        .object_fit(ObjectFit::Fill),
                );
            }
            let add = div()
                .id("add-channel")
                .absolute()
                .left(px(self.leading + tail + 1.0))
                .top(px(if stone { 0.0 } else { TAB_TOP }))
                .h(px(if stone { TAB_BAR_HEIGHT } else { TAB_HEIGHT }))
                .w(px(46.0))
                .flex()
                .items_center()
                .justify_center()
                .font_weight(gpui::FontWeight::BOLD)
                .text_size(px(16.0))
                .cursor_pointer()
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_add(event, window, cx);
                })
                .child("+");
            strip = strip.child(if stone {
                add.font_family(STONE_FONT)
                    .text_color(rgb(STONE_INK_IDLE))
                    .hover(|style| style.bg(rgba(0xe8c8_7412)).text_color(rgb(0x00e8_dcc0)))
                    .active(|style| {
                        style
                            .bg(rgba(0xe8c8_7424))
                            .text_color(rgb(STONE_INK_ACTIVE))
                    })
            } else {
                add.font_family(FONT_INTERFACE)
                    .text_color(rgb(0x007d_8fa8))
                    .hover(|style| style.bg(rgba(0x153e_6640)).text_color(rgb(0x00d6_e0f0)))
                    .active(|style| style.bg(rgba(0x245d_8b78)).text_color(rgb(0x00ff_ffff)))
            });
        }

        if self.compact {
            let content_width = self.leading + tail + 8.0;
            tab_strip()
                .id("channel-tabs-scroll")
                .overflow_x_scroll()
                .track_scroll(&self.scroll.expect("compact tabs require a scroll handle"))
                .child(
                    div()
                        .relative()
                        .h_full()
                        .w(px(
                            content_width.max(f32::from(window.viewport_size().width))
                        ))
                        .child(strip),
                )
                .into_any_element()
        } else {
            strip.into_any_element()
        }
    }
}

#[derive(Clone, Copy)]
struct TabChrome {
    pub width: f32,
    pub active: bool,
    pub divider: bool,
    pub effect_opacity: [f32; 3],
    pub tone: ChannelTabTone,
    pub unread: bool,
    pub variant: ModalVariant,
}

#[derive(IntoElement)]
struct TabVisual {
    label: String,
    measured: f32,
    marquee_offset: f32,
    tint: Hsla,
    chrome: TabChrome,
    assets: Sc2Assets,
}

impl TabVisual {
    #[must_use]
    fn new(
        label: String,
        measured: f32,
        marquee_offset: f32,
        tint: impl Into<Hsla>,
        chrome: TabChrome,
        assets: Sc2Assets,
    ) -> Self {
        Self {
            label,
            measured,
            marquee_offset,
            tint: tint.into(),
            chrome,
            assets,
        }
    }
}

impl RenderOnce for TabVisual {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let chrome = if self.chrome.variant == ModalVariant::Reforged {
            stone_tab_chrome(self.chrome)
        } else {
            tab_chrome(self.chrome, &self.assets)
        };
        div().absolute().inset_0().child(chrome).child(tab_label(
            self.label,
            self.measured,
            self.chrome.width,
            self.marquee_offset,
            self.tint,
            self.chrome.variant,
            self.chrome.unread,
        ))
    }
}

#[must_use]
pub fn tab_width(title: &str, unread: bool) -> f32 {
    let characters = title.to_uppercase().chars().count() + usize::from(unread) * 3;
    let characters = u16::try_from(characters).unwrap_or(u16::MAX);
    f32::from(characters).mul_add(8.1, 98.0).clamp(182.0, 258.0)
}

#[must_use]
fn measure_tab_title(title: &str, window: &Window, variant: ModalVariant) -> f32 {
    let run = TextRun {
        len: title.len(),
        font: font(tab_font(variant)),
        color: rgb(0x00ff_ffff).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    f32::from(
        window
            .text_system()
            .layout_line(title, px(12.5), &[run], None)
            .width,
    )
}

#[must_use]
fn tab_chrome(spec: TabChrome, assets: &Sc2Assets) -> Div {
    let mut chrome = div().absolute().inset_0().overflow_hidden();
    if spec.active {
        let (selected, line, glow) = match spec.tone {
            ChannelTabTone::Group => (
                assets.top_navigation_selected_orange.clone(),
                assets.top_navigation_selected_line_orange.clone(),
                assets.top_navigation_selected_glow_orange.clone(),
            ),
            ChannelTabTone::Party => (
                assets.top_navigation_selected_pink.clone(),
                assets.top_navigation_selected_line_pink.clone(),
                assets.top_navigation_selected_glow_pink.clone(),
            ),
            ChannelTabTone::Standard => (
                assets.top_navigation_selected.clone(),
                assets.top_navigation_selected_line.clone(),
                assets.top_navigation_selected_glow.clone(),
            ),
        };
        chrome = chrome
            .child(
                img(selected)
                    .absolute()
                    .top(px(1.0))
                    .left(px(5.0))
                    .w(px((spec.width - 10.0).max(0.0)))
                    .h(px(35.0))
                    .opacity(spec.effect_opacity[0])
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                img(line)
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(px(5.0))
                    .opacity(spec.effect_opacity[1])
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                img(glow)
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .w_full()
                    .h(px(5.0))
                    .opacity(spec.effect_opacity[2])
                    .object_fit(ObjectFit::Fill),
            );
    }
    if spec.divider {
        chrome = chrome.child(
            img(assets.top_navigation_divider.clone())
                .absolute()
                .top(px(2.0))
                .left_0()
                .w(px(1.0))
                .h(px(32.0))
                .object_fit(ObjectFit::Fill),
        );
    }
    chrome
}

/// the stone strip's ink: one gold, three weights, and unread lifts an idle
/// name to the hover weight so the stud beside it has company.
fn stone_tab_tint(active: bool, hovered: bool, unread: bool) -> Hsla {
    if active {
        rgb(STONE_INK_ACTIVE).into()
    } else if hovered || unread {
        rgb(STONE_INK_HOVER).into()
    } else {
        rgb(STONE_INK_IDLE).into()
    }
}

/// Reforged's tab chrome: the active tab is a darker stone block with bronze
/// sides and a glowing gold underline; an idle tab carries a hairline at its
/// right edge, inset from the strip's top and bottom.
fn stone_tab_chrome(spec: TabChrome) -> Div {
    let mut chrome = div().absolute().inset_0().overflow_hidden();
    if spec.active {
        chrome = chrome
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .border_l_1()
                    .border_r_1()
                    .border_color(rgb(STONE_EDGE))
                    .bg(linear_gradient(
                        180.0,
                        linear_color_stop(rgba(0x2418_08ff), 0.0),
                        linear_color_stop(rgba(0x1710_06ff), 1.0),
                    ))
                    .opacity(spec.effect_opacity[0]),
            )
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(3.0))
                    .bg(rgb(STONE_GOLD))
                    .shadow(vec![
                        BoxShadow::new(px(0.0), px(0.0), rgba(0xe8c8_74b3).into())
                            .blur_radius(px(8.0)),
                    ])
                    .opacity(spec.effect_opacity[1]),
            );
    } else {
        chrome = chrome.child(
            div()
                .absolute()
                .right_0()
                .top(px(6.0))
                .bottom(px(6.0))
                .w(px(1.0))
                .bg(rgba(0x5e4a_2699)),
        );
    }
    chrome
}

/// the gold stud that marks an unread stone tab.
fn stone_unread_stud() -> Div {
    div()
        .size(px(5.0))
        .flex_shrink_0()
        .bg(rgb(STONE_GOLD))
        .shadow(vec![
            BoxShadow::new(px(0.0), px(0.0), rgba(0xe8c8_74e6).into()).blur_radius(px(6.0)),
        ])
}

const fn tab_font(variant: ModalVariant) -> &'static str {
    match variant {
        ModalVariant::Reforged => STONE_FONT,
        ModalVariant::Sc2 | ModalVariant::Remastered => FONT_NAVIGATION,
    }
}

fn tab_text_tint(tone: ChannelTabTone, active: bool, hovered: bool, unread: bool) -> Hsla {
    if unread && !active {
        return match tone {
            ChannelTabTone::Party => rgb(0x00f0_92c4).into(),
            ChannelTabTone::Group => rgb(0x00f0_aa64).into(),
            ChannelTabTone::Standard => rgb(0x006b_c2f2).into(),
        };
    }
    if active {
        return match tone {
            ChannelTabTone::Party => rgb(0x00ff_e8f6).into(),
            ChannelTabTone::Group => rgb(0x00ff_edd7).into(),
            ChannelTabTone::Standard => rgb(0x00e6_f9ff).into(),
        };
    }
    if hovered {
        return match tone {
            ChannelTabTone::Party => rgb(0x00b9_789d).into(),
            ChannelTabTone::Group => rgb(0x00b7_8358).into(),
            ChannelTabTone::Standard => rgb(0x0073_94b4).into(),
        };
    }
    match tone {
        ChannelTabTone::Party => rgb(0x0076_516a).into(),
        ChannelTabTone::Group => rgb(0x0074_5b45).into(),
        ChannelTabTone::Standard => rgb(0x0041_5d7d).into(),
    }
}

#[must_use]
fn centered_tab_label(
    label: String,
    width: f32,
    tint: Hsla,
    variant: ModalVariant,
    stud: bool,
) -> Div {
    div()
        .absolute()
        .left(px(TAB_NAME_INSET))
        .top_0()
        .w(px((width - TAB_NAME_INSET * 2.0).max(1.0)))
        .h(px(tab_slot_height(variant)))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(7.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .font_family(tab_font(variant))
        .text_size(px(12.5))
        .text_color(tint)
        .child(label)
        .when(stud, |label| label.child(stone_unread_stud()))
}

#[must_use]
fn tab_name_layout(measured: f32, slot_width: f32) -> TabNameLayout {
    let centered_width = (slot_width - TAB_NAME_INSET * 2.0).max(1.0);
    let viewport_width = (slot_width - TAB_NAME_LEAD - TAB_NAME_INSET).max(1.0);
    let uses_marquee = measured > centered_width;
    let travel = if uses_marquee {
        (measured - viewport_width).max(0.0)
    } else {
        0.0
    };
    TabNameLayout {
        uses_marquee,
        viewport_width,
        travel,
    }
}

#[must_use]
fn tab_label(
    label: String,
    measured: f32,
    slot_width: f32,
    marquee_offset: f32,
    tint: impl Into<Hsla>,
    variant: ModalVariant,
    unread: bool,
) -> Div {
    let tint = tint.into();
    let stud = unread && variant == ModalVariant::Reforged;
    let layout = tab_name_layout(measured, slot_width);
    if !layout.uses_marquee {
        return centered_tab_label(label, slot_width, tint, variant, stud);
    }
    let label_left = if layout.travel > 0.0 {
        marquee_offset
    } else {
        ((layout.viewport_width - measured) / 2.0).max(0.0)
    };
    div()
        .absolute()
        .top_0()
        .left(px(TAB_NAME_LEAD))
        .w(px(layout.viewport_width))
        .h(px(tab_slot_height(variant)))
        .overflow_hidden()
        .child(
            div()
                .absolute()
                .left(px(label_left))
                .top_0()
                .w(px(measured + 24.0))
                .h(px(tab_slot_height(variant)))
                .flex()
                .items_center()
                .gap(px(7.0))
                .whitespace_nowrap()
                .font_family(tab_font(variant))
                .text_size(px(12.5))
                .text_color(tint)
                .child(label)
                .when(stud, |label| label.child(stone_unread_stud())),
        )
}

/// StarCraft II's tabs sit inside the bar on their nav plate; the stone tabs
/// are the full height of the strip, the way the design cuts them.
const fn tab_slot_height(variant: ModalVariant) -> f32 {
    match variant {
        ModalVariant::Reforged => TAB_BAR_HEIGHT,
        ModalVariant::Sc2 | ModalVariant::Remastered => TAB_HEIGHT,
    }
}

#[must_use]
fn tab_slot(left: f32, width: f32, opacity: f32, variant: ModalVariant) -> Div {
    let top = if variant == ModalVariant::Reforged {
        0.0
    } else {
        TAB_TOP
    };
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .h(px(tab_slot_height(variant)))
        .opacity(opacity)
        .overflow_hidden()
}

#[must_use]
fn tab_strip() -> Div {
    div().absolute().inset_0().overflow_hidden()
}

#[must_use]
pub fn bar(background: Option<ImageSource>) -> Div {
    div()
        .relative()
        .h(px(TAB_BAR_HEIGHT))
        .flex_shrink_0()
        .overflow_hidden()
        .bg(rgb(0x0008_101a))
        .children(
            background.map(|source| img(source).absolute().inset_0().object_fit(ObjectFit::Fill)),
        )
}

/// Reforged's strip: carved stone instead of the nav plate — a dark stone
/// gradient under a bronze rule, the same 43px the StarCraft II bar stands.
#[must_use]
pub fn stone_bar() -> Div {
    div()
        .relative()
        .h(px(TAB_BAR_HEIGHT))
        .flex_shrink_0()
        .overflow_hidden()
        .bg(linear_gradient(
            180.0,
            linear_color_stop(rgba(0x1710_06ff), 0.0),
            linear_color_stop(rgba(0x0d09_05ff), 1.0),
        ))
        .border_b_1()
        .border_color(rgb(STONE_EDGE))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        foundation::animation::{animation_progress, interpolated_offsets},
        products::sc2::theme::TAB_NAME_INSET,
    };

    use super::{
        TabRelease, TabStripState, marquee_duration, reordered_tab_origins, tab_landing_index,
        tab_name_layout, tab_origins, tab_width,
    };

    #[test]
    fn tab_width_matches_the_native_contract() {
        assert!((tab_width("General", false) - 182.0).abs() < f32::EPSILON);
        assert!((tab_width(&"x".repeat(40), false) - 258.0).abs() < f32::EPSILON);
        let expected = 12.0_f32.mul_add(8.1, 98.0);
        assert!((tab_width("aaaaaaaaaaß", false) - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn unread_marker_reserves_label_space() {
        let title = "abcdefghijkl";
        assert!(tab_width(title, true) > tab_width(title, false));
    }

    #[test]
    fn tab_reordering_uses_the_dragged_tab_midpoint() {
        let widths = [100.0, 120.0, 80.0];
        assert_eq!(tab_origins(&widths, 10.0), vec![10.0, 110.0, 230.0]);
        assert_eq!(
            reordered_tab_origins(&widths, 10.0, 0, 2),
            vec![210.0, 10.0, 130.0]
        );
        assert_eq!(tab_landing_index(&widths, 10.0, 0, 240.0), 2);
    }

    #[test]
    fn marquee_layout_uses_the_shared_clip_window() {
        let layout = tab_name_layout(170.0, 182.0);
        assert!(layout.uses_marquee);
        assert!(layout.viewport_width > 182.0 - TAB_NAME_INSET * 2.0);
        assert_eq!(layout.travel, 40.0);
        assert_eq!(marquee_duration(55.0), Duration::from_secs(1));
    }

    #[test]
    fn reorder_offsets_share_the_same_easing_output() {
        let progress = animation_progress(Duration::from_millis(80), Duration::from_millis(160));
        assert_eq!(
            interpolated_offsets(&[0.0, 10.0], &[20.0, -10.0], progress),
            vec![10.0, 0.0]
        );
    }

    #[test]
    fn shared_tab_state_distinguishes_clicks_and_reorders() {
        let mut state = TabStripState::<String, f64>::default();
        assert!(state.begin_pointer(1, 150.0, 3));
        assert_eq!(state.finish(1), TabRelease::Click(1));

        assert!(state.begin_pointer(0, 50.0, 3));
        assert!(state.update_drag(0, 280.0, &[100.0, 100.0, 100.0], 0.0));
        assert_eq!(state.finish(0), TabRelease::Reorder { from: 0, to: 2 });
    }

    #[test]
    fn shared_tab_state_animates_marquee_in_both_clock_domains() {
        let mut state = TabStripState::<String, f64>::default();
        let key = "long-tab".to_owned();
        assert!(state.set_name_hover(key.clone(), true, 55.0, 0.0));
        assert!((state.name_offset(&key, 500.0) + 27.5).abs() < f32::EPSILON);
        assert!(state.name_animation_is_running(500.0));
        assert!(!state.name_animation_is_running(1_000.0));
    }
}
