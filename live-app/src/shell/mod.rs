//! browser host shell for a Live feed — the game picker, and the realm you
//! enter from it.
//!
//! The picker is a faithful port of the desktop's `picker_section`: the
//! centred logo lockup, the room that crossfades between games as the pointer
//! moves, the card row drawn from the shared [`superiority_ui::products::games`]
//! system, the enter-game glide-flood-dissolve, and the keyboard legend. Picking
//! a card enters that product's read-only view; a floating way back returns.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, Context, Entity, FocusHandle, Focusable, ImageSource,
    KeyDownEvent, MouseButton, ObjectFit, PathBuilder, Render, Window, canvas, div, img, point,
    prelude::*, px, relative, rgb, rgba,
};
use serde::Deserialize;
use superiority_ui::{
    foundation::assets::{AssetResolver as _, WebAssetResolver},
    products::games::{
        CardData, CardLook, CardMotion, CardSize, CardState, GAMES, LiveCard, Motion, PickerState,
        ROOM, StageMotion, card_art, card_button, card_shell, destination_line, eased, fade,
        picker_size, realm_flood, room_layer, tracked,
    },
    products::sc2::theme::{FONT_NAVIGATION, scope_glow},
};

use crate::products::{sc2::LiveView as Sc2LiveView, scr::ScrLiveView, wc3::Wc3LiveView};

pub(crate) mod transport;

use transport::get_json;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FeedProduct {
    Sc2,
    Scr,
    Wc3,
}

impl FeedProduct {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Sc2 => "StarCraft II",
            Self::Scr => "StarCraft: Remastered",
            Self::Wc3 => "Warcraft III",
        }
    }

    const fn card(self) -> usize {
        match self {
            Self::Sc2 => 0,
            Self::Scr => 1,
            Self::Wc3 => 2,
        }
    }

    fn from_card(index: usize) -> Option<Self> {
        [Self::Sc2, Self::Scr, Self::Wc3].into_iter().nth(index)
    }
}

enum ProductView {
    Sc2(Entity<Sc2LiveView>),
    Scr(Entity<ScrLiveView>),
    Wc3(Entity<Wc3LiveView>),
}

impl ProductView {
    fn set_active(&self, active: bool, cx: &mut Context<LiveShell>) {
        match self {
            Self::Sc2(view) => view.update(cx, |view, cx| view.set_active(active, cx)),
            Self::Scr(view) => view.update(cx, |view, cx| view.set_active(active, cx)),
            Self::Wc3(view) => view.update(cx, |view, cx| view.set_active(active, cx)),
        }
    }

    fn element(&self) -> AnyElement {
        match self {
            Self::Sc2(view) => view.clone().into_any_element(),
            Self::Scr(view) => view.clone().into_any_element(),
            Self::Wc3(view) => view.clone().into_any_element(),
        }
    }
}

#[derive(Clone, Default)]
struct CardStatus {
    present: bool,
    channel: Option<String>,
    online: Option<usize>,
}

struct MountedView {
    product: FeedProduct,
    view: ProductView,
}

enum Screen {
    Picker,
    InGame(FeedProduct),
}

pub(crate) struct LiveShell {
    focus: FocusHandle,
    backend: String,
    feed_id: String,
    asset_root: String,
    resolver: WebAssetResolver,
    cards: [CardStatus; 3],
    views: Vec<MountedView>,
    screen: Screen,
    /// the picker's selection and choreography, on the browser's millisecond
    /// clock — the same `PickerState` the desktop shell drives.
    picker: PickerState<f64>,
    loading: bool,
    error: Option<String>,
}

impl LiveShell {
    pub(crate) fn new(
        backend: String,
        feed_id: String,
        asset_root: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let asset_root = asset_root.trim_end_matches('/').to_owned();
        let now = js_sys::Date::now();
        let shell = Self {
            focus: cx.focus_handle(),
            backend: backend.trim_end_matches('/').to_owned(),
            feed_id,
            resolver: WebAssetResolver::new(&asset_root),
            asset_root,
            cards: Default::default(),
            views: Vec::new(),
            screen: Screen::Picker,
            picker: PickerState::new(now),
            loading: true,
            error: None,
        };
        shell.start_polling(cx);
        shell
    }

    fn start_polling(&self, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let feed_id = self.feed_id.clone();
        cx.spawn(async move |this, cx| {
            loop {
                let overview = fetch_overview(&backend, &feed_id).await;
                if this
                    .update(cx, |shell, cx| shell.apply_overview(overview, cx))
                    .is_err()
                {
                    break;
                }
                gloo_timers::future::TimeoutFuture::new(3_000).await;
            }
        })
        .detach();
    }

    fn apply_overview(
        &mut self,
        overview: Result<Vec<ProductOverview>, String>,
        cx: &mut Context<Self>,
    ) {
        self.loading = false;
        match overview {
            Ok(products) => {
                self.error = None;
                let mut cards: [CardStatus; 3] = Default::default();
                for entry in products {
                    let channel = entry.channels.first();
                    cards[entry.product.card()] = CardStatus {
                        present: true,
                        channel: channel.map(ChannelSummary::label),
                        online: channel.map(|channel| channel.member_count),
                    };
                }
                self.cards = cards;
                if !self.actionable(self.picker.selected)
                    && let Some(first) = (0..3).find(|index| self.actionable(*index))
                {
                    self.picker.selected = first;
                    self.picker.previously_selected = first;
                }
                // build every present game's view up front, so entering one is
                // a flip rather than an allocation mid-render
                for index in 0..3 {
                    if let Some(product) = FeedProduct::from_card(index)
                        && self.cards[index].present
                        && !self.views.iter().any(|mounted| mounted.product == product)
                    {
                        let view = self.build_view(product, cx);
                        self.views.push(MountedView { product, view });
                    }
                }
            }
            Err(error) => {
                if self.cards.iter().all(|card| !card.present) {
                    self.error = Some(error);
                }
            }
        }
        let entered = match self.screen {
            Screen::InGame(product) => Some(product),
            Screen::Picker => None,
        };
        for mounted in &self.views {
            mounted
                .view
                .set_active(Some(mounted.product) == entered, cx);
        }
        cx.notify();
    }

    /// whether a card can be entered: its game is shared on this feed.
    fn actionable(&self, index: usize) -> bool {
        GAMES.get(index).is_some_and(|game| game.shown)
            && self.cards.get(index).is_some_and(|card| card.present)
    }

    fn enter(&mut self, product: FeedProduct, cx: &mut Context<Self>) {
        if !self.views.iter().any(|mounted| mounted.product == product) {
            return;
        }
        self.screen = Screen::InGame(product);
        for mounted in &self.views {
            mounted.view.set_active(mounted.product == product, cx);
        }
        cx.notify();
    }

    fn show_picker(&mut self, cx: &mut Context<Self>) {
        self.screen = Screen::Picker;
        self.picker.motion = Motion::Entrance {
            started: js_sys::Date::now(),
        };
        for mounted in &self.views {
            mounted.view.set_active(false, cx);
        }
        cx.notify();
    }

    fn build_view(&self, product: FeedProduct, cx: &mut Context<Self>) -> ProductView {
        let backend = self.backend.clone();
        let feed_id = self.feed_id.clone();
        let asset_root = self.asset_root.clone();
        match product {
            FeedProduct::Sc2 => {
                ProductView::Sc2(cx.new(|cx| Sc2LiveView::new(backend, feed_id, asset_root, cx)))
            }
            FeedProduct::Scr => {
                ProductView::Scr(cx.new(|cx| ScrLiveView::new(backend, feed_id, asset_root, cx)))
            }
            FeedProduct::Wc3 => {
                ProductView::Wc3(cx.new(|cx| Wc3LiveView::new(backend, feed_id, asset_root, cx)))
            }
        }
    }

    // ── picker state ─────────────────────────────────────────────────────────

    /// reaches for a card: hovering one also selects it, so the room follows
    /// the pointer; leaving leaves the selection where it was.
    fn reach(&mut self, card: Option<usize>) -> bool {
        let mask = self.actionable_mask();
        self.picker.reach(card, js_sys::Date::now(), &mask)
    }

    fn choose(&mut self, card: usize) -> bool {
        let mask = self.actionable_mask();
        self.picker.choose(card, js_sys::Date::now(), &mask)
    }

    fn move_choice(&mut self, delta: isize) {
        let mask = self.actionable_mask();
        self.picker.move_choice(delta, js_sys::Date::now(), &mask);
    }

    /// the three cards' actionability, the mask the shared picker filters on.
    fn actionable_mask(&self) -> [bool; GAMES.len()] {
        std::array::from_fn(|index| self.actionable(index))
    }

    /// the rooms to paint behind the cards, bottom first — a crossfade between
    /// the game left and the one arriving.
    fn rooms(&self, now: f64) -> Vec<(usize, f32)> {
        let mask = self.actionable_mask();
        self.picker.rooms(now, &mask)
    }

    fn card_motion(&self, index: usize, size: CardSize, now: f64) -> CardMotion {
        self.picker
            .card_motion(index, slot_offset(index, size), now, false)
    }

    /// winds the choreography: the entrance starts on the first frame, and
    /// entering a realm hands over once the card has dissolved. Returns whether
    /// another frame is needed.
    fn advance_motion(&mut self, now: f64, cx: &mut Context<Self>) -> bool {
        let advanced = self.picker.advance(now);
        if let Some(card) = advanced.entered
            && let Some(product) = FeedProduct::from_card(card)
        {
            self.enter(product, cx);
        }
        advanced.animating
    }

    // ── the picker screen ────────────────────────────────────────────────────

    fn picker(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let now = js_sys::Date::now();
        let motion_animating = self.advance_motion(now, cx);
        if let Screen::InGame(product) = self.screen {
            return self.realm(product, cx);
        }
        // keep the frame clock running while the ROOM crossfades, not only while
        // a card is moving — otherwise the background switch renders one frame
        // and freezes
        let room_animating = self.picker.previously_selected != self.picker.selected
            && eased(self.picker.chosen, ROOM, now) < 1.0;
        if motion_animating || room_animating {
            window.request_animation_frame();
        }

        let size = picker_size(f32::from(window.viewport_size().width));
        let stage = self.picker.motion.stage(now, false);

        div()
            .id("live-picker")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(gpui::linear_gradient(
                180.0,
                gpui::linear_color_stop(rgba(0x1014_1cff), 0.0),
                gpui::linear_color_stop(rgba(0x0507_0aff), 0.65),
            ))
            .on_key_down(cx.listener(Self::on_key_down))
            // the room, crossfading; it dims as a realm floods over it
            .children(self.rooms(now).into_iter().filter_map(|(index, light)| {
                let palette = GAMES.get(index)?;
                let art = self.resolver.image(palette.art);
                Some(room_layer(palette, light * (1.0 - stage.flood), art))
            }))
            // the stage blooms out of black on the way in
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .bg(rgba(0x0406_0aff))
                    .opacity(1.0 - stage.glow),
            )
            // the chosen realm's nebula taking the whole stage
            .children(
                stage
                    .flooding
                    .and_then(|index| GAMES.get(index))
                    .filter(|_| stage.flood > 0.001)
                    .map(|palette| {
                        let art = self.resolver.image(palette.art);
                        realm_flood(palette, stage.flood, art)
                    }),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(44.0))
                    .child(self.lockup(&stage))
                    .child(self.card_row(size, now, cx))
                    .child(self.legend(&stage)),
            )
            .children((stage.entering > 0.001).then(|| self.entering_overlay(&stage)))
            .into_any_element()
    }

    /// the stacked lockup: the mark, the wordmark, and the tagline, centred
    /// above the cards and fading in with the entrance.
    fn lockup(&self, stage: &StageMotion) -> AnyElement {
        div()
            .relative()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(11.0))
            .opacity(stage.title)
            .top(px(stage.title_rise))
            .child(
                img(ImageSource::from(format!(
                    "{}/ui/brand/logo-tile.png",
                    self.asset_root
                )))
                .w(px(72.0))
                .h(px(72.0))
                .object_fit(ObjectFit::Contain),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(5.0))
                    .child(tracked(
                        "SUPERIORITY",
                        8.0,
                        div()
                            .font_family(FONT_NAVIGATION)
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_size(px(16.0))
                            .text_color(rgb(PAPER)),
                    ))
                    .child(tracked(
                        "UPLINK ESTABLISHED",
                        3.5,
                        div().text_size(px(9.0)).text_color(rgb(0x008f_a2ba)),
                    )),
            )
            .into_any_element()
    }

    fn card_row(&self, size: CardSize, now: f64, cx: &mut Context<Self>) -> AnyElement {
        // collect before the div, so each card's `cx` borrow is released before
        // the row's own hover listener takes one
        let mut cards: Vec<AnyElement> = Vec::new();
        for index in (0..GAMES.len()).filter(|index| GAMES[*index].shown) {
            let motion = self.card_motion(index, size, now);
            // the one being entered holds its slot and shows nothing; the copy
            // above the row does the travelling
            let held = CardMotion {
                opacity: if self.picker.motion.entering_card() == Some(index) {
                    0.0
                } else {
                    motion.opacity
                },
                glide: 0.0,
                scale: 1.0,
                ..motion
            };
            cards.push(self.picker_card(index, size, &held, cx));
        }
        let leaving = self
            .picker
            .motion
            .entering_card()
            .map(|index| self.leaving_card(index, size, now, cx));
        div()
            .id("live-card-row")
            .relative()
            .flex()
            .flex_wrap()
            .items_stretch()
            .justify_center()
            .gap(px(size.gap_between))
            .on_hover(cx.listener(|shell, hovered: &bool, _, cx| {
                if !hovered && shell.reach(None) {
                    cx.notify();
                }
            }))
            .children(cards)
            // the winner, drawn again over the row so growing it shoves nothing
            .children(leaving)
            .into_any_element()
    }

    fn picker_card(
        &self,
        index: usize,
        size: CardSize,
        motion: &CardMotion,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = &GAMES[index];
        let present = self.actionable(index);
        let chosen = self.picker.selected == index;

        let mut look = CardLook::resolve(
            palette,
            CardState::Connected,
            CardState::Connected,
            0.0,
            0.0,
        );
        look.live();
        if !present {
            look.out_of_play(palette);
        }

        let data = CardData {
            state: CardState::Connected,
            owned: present,
            licensed: present,
            live_mode: true,
            live: present.then(|| LiveCard {
                channel: self.cards[index].channel.clone(),
                online: self.cards[index].online,
                ..LiveCard::default()
            }),
        };

        let scaled = CardSize {
            width: size.width * motion.scale,
            art: size.art * motion.scale,
            ..size
        };
        let art = self.resolver.image(palette.art);
        let mut card = card_shell(palette, &look, scaled, 0.0)
            .id(("live-card", index))
            .relative()
            .opacity(motion.opacity)
            .top(px(motion.offset))
            .left(px(motion.glide))
            .when(present && !chosen, |card| {
                card.border_color(rgba(fade(look.border << 8, 0.45)))
            });
        if present {
            let glow = palette.glow;
            let focus = palette.focus;
            card = card
                .cursor_pointer()
                .when(chosen && motion.lit, |card| {
                    card.border_color(rgb(focus)).shadow(scope_glow(glow))
                })
                .hover(move |style| style.border_color(rgb(focus)).shadow(scope_glow(glow)))
                .on_hover(cx.listener(move |shell, hovered: &bool, window, cx| {
                    // grab focus on hover (never during render), so the arrow
                    // keys and 1/2/3 work once the pointer is on the picker
                    if *hovered {
                        shell.focus.focus(window, cx);
                    }
                    if shell.reach(hovered.then_some(index)) {
                        cx.notify();
                    }
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |shell, _, _, cx| {
                    shell.choose(index);
                    shell.picker.motion = Motion::Entering {
                        card: index,
                        started: js_sys::Date::now(),
                    };
                    cx.notify();
                }));
        }
        card.child(card_art(palette, &look, scaled, art))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(9.0))
                    .px(px(scaled.pad_x))
                    .pt(px(12.0))
                    .pb(px(scaled.pad_bottom))
                    .child(destination_line(palette, &data, &look, scaled))
                    .child(
                        div()
                            .relative()
                            .child(card_button(
                                palette,
                                &look,
                                scaled,
                                (!present).then_some("NOT SHARED"),
                                present && chosen,
                            ))
                            // a reliable return key: the web font has no ⏎, so
                            // it is drawn. only the chosen, enterable card wears it
                            .when(present && chosen, |seat| {
                                seat.child(
                                    div()
                                        .absolute()
                                        .right(px(8.0))
                                        .top_0()
                                        .bottom_0()
                                        .flex()
                                        .items_center()
                                        .child(return_glyph(palette.focus)),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }

    /// the winner lifted out of the row: it starts exactly on its slot, then
    /// glides to the middle and grows into the flood.
    fn leaving_card(
        &self,
        index: usize,
        size: CardSize,
        now: f64,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let motion = self.card_motion(index, size, now);
        let grown = size.width * motion.scale;
        let resting = index_offset(index, size);
        div()
            .absolute()
            .top(px(size.art.mul_add(-(motion.scale - 1.0), 0.0) / 2.0))
            .left(px(motion.glide + resting - (grown - size.width) / 2.0))
            .opacity(motion.opacity)
            .child(self.picker_card(
                index,
                size,
                &CardMotion {
                    opacity: 1.0,
                    offset: 0.0,
                    glide: 0.0,
                    ..motion
                },
                cx,
            ))
            .into_any_element()
    }

    fn legend(&self, stage: &StageMotion) -> AnyElement {
        div()
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(10.0))
            .opacity(stage.title)
            .text_size(px(9.0))
            .text_color(rgb(0x005a_6478))
            .child("← → SELECT")
            .child("·")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(return_glyph(0x005a_6478))
                    .child("ENTER"),
            )
            .child("·")
            .child("1 2 3 JUMP")
            .into_any_element()
    }

    fn entering_overlay(&self, stage: &StageMotion) -> AnyElement {
        let palette = stage
            .flooding
            .and_then(|index| GAMES.get(index))
            .unwrap_or(&GAMES[0]);
        let channel = stage
            .flooding
            .and_then(FeedProduct::from_card)
            .and_then(|product| self.cards[product.card()].channel.clone());
        div()
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(14.0))
            .opacity(stage.entering)
            .child(
                div()
                    .font_family(FONT_NAVIGATION)
                    .text_size(px(24.0))
                    .text_color(rgb(palette.text))
                    .child("ENTERING"),
            )
            .child(
                div()
                    .flex()
                    .gap(px(6.0))
                    .text_size(px(12.0))
                    .text_color(rgb(palette.dim))
                    .child(format!("{} ·", palette.title_lower()))
                    .children(
                        channel.map(|channel| div().text_color(rgb(palette.ok)).child(channel)),
                    ),
            )
            .child(
                div().w(px(220.0)).h(px(2.0)).bg(rgba(0x133e_5b99)).child(
                    div()
                        .h(px(2.0))
                        .w(relative(stage.progress))
                        .bg(rgb(palette.focus))
                        .shadow(scope_glow(palette.glow)),
                ),
            )
            .into_any_element()
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.picker.motion, Motion::Ready) {
            return;
        }
        match event.keystroke.key.as_str() {
            "left" => self.move_choice(-1),
            "right" => self.move_choice(1),
            "1" | "2" | "3" => {
                if let Some(index) = event
                    .keystroke
                    .key
                    .parse::<usize>()
                    .ok()
                    .and_then(|d| d.checked_sub(1))
                    && self.actionable(index)
                {
                    self.choose(index);
                    self.picker.motion = Motion::Entering {
                        card: index,
                        started: js_sys::Date::now(),
                    };
                }
            }
            "enter" | "return" | "space" => {
                if self.actionable(self.picker.selected) {
                    self.picker.motion = Motion::Entering {
                        card: self.picker.selected,
                        started: js_sys::Date::now(),
                    };
                }
            }
            _ => return,
        }
        cx.notify();
    }

    // ── an entered realm ─────────────────────────────────────────────────────

    fn realm(&self, product: FeedProduct, cx: &mut Context<Self>) -> AnyElement {
        // the entered view reveals with the app's 340ms chrome fade, keyed by
        // product so switching games replays it
        let body = self
            .views
            .iter()
            .find(|mounted| mounted.product == product)
            .map(|mounted| {
                div()
                    .size_full()
                    .child(mounted.view.element())
                    .with_animation(
                        ("live-realm-reveal", product.card()),
                        Animation::new(Duration::from_millis(340)),
                        gpui::Styled::opacity,
                    )
            });
        div()
            .id("live-realm")
            .size_full()
            .relative()
            .children(body)
            .child(
                div()
                    .id("live-games")
                    .absolute()
                    .left(px(12.0))
                    .bottom(px(10.0))
                    .flex()
                    .items_center()
                    .h(px(26.0))
                    .px(px(12.0))
                    .rounded(px(3.0))
                    .bg(rgba(0x0810_18e6))
                    .border_1()
                    .border_color(rgba(0x2f4a_66cc))
                    .text_size(px(11.0))
                    .text_color(rgb(0xbcd2_e8))
                    .cursor_pointer()
                    .hover(|button| button.bg(rgba(0x1222_36f2)).border_color(rgb(0x4d84_c0)))
                    .child("‹ GAMES")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|shell, _, _, cx| shell.show_picker(cx))),
            )
            .into_any_element()
    }

    fn status_panel(message: impl Into<String>) -> AnyElement {
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(SHEET))
            .text_color(rgb(0x9fb4c8))
            .text_size(px(15.0))
            .child(message.into())
            .into_any_element()
    }
}

impl Focusable for LiveShell {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

const SHEET: u32 = 0x0004_060a;
const PAPER: u32 = 0x00e6_edf7;

/// a drawn return-key mark, since the browser font bundle carries no ⏎.
fn return_glyph(color: u32) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let o = bounds.origin;
            let mut shaft = PathBuilder::stroke(px(1.0));
            shaft.move_to(point(o.x + px(9.5), o.y + px(1.5)));
            shaft.line_to(point(o.x + px(9.5), o.y + px(5.5)));
            shaft.line_to(point(o.x + px(2.5), o.y + px(5.5)));
            if let Ok(path) = shaft.build() {
                window.paint_path(path, rgb(color));
            }
            let mut head = PathBuilder::stroke(px(1.0));
            head.move_to(point(o.x + px(4.5), o.y + px(3.0)));
            head.line_to(point(o.x + px(2.0), o.y + px(5.5)));
            head.line_to(point(o.x + px(4.5), o.y + px(8.0)));
            if let Ok(path) = head.build() {
                window.paint_path(path, rgb(color));
            }
        },
    )
    .w(px(12.0))
    .h(px(10.0))
    .flex_shrink_0()
}

/// where card `index` sits from the row's centre, in pixels — the distance it
/// glides to the middle when it wins.
fn slot_offset(index: usize, size: CardSize) -> f32 {
    let count = (0..GAMES.len()).filter(|i| GAMES[*i].shown).count();
    let position = (0..index).filter(|i| GAMES[*i].shown).count();
    let middle = (count as f32 - 1.0) / 2.0;
    (middle - position as f32) * (size.width + size.gap_between)
}

/// where a card's slot starts, from the left of the row.
fn index_offset(index: usize, size: CardSize) -> f32 {
    let position = (0..index).filter(|i| GAMES[*i].shown).count();
    position as f32 * (size.width + size.gap_between)
}

impl Render for LiveShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.loading {
            return Self::status_panel("Connecting to Superiority Live…");
        }
        if self.cards.iter().all(|card| !card.present) {
            return Self::status_panel(
                self.error
                    .clone()
                    .unwrap_or_else(|| "This feed has no live games yet.".to_owned()),
            );
        }
        match self.screen {
            Screen::Picker => self.picker(window, cx),
            Screen::InGame(product) => self.realm(product, cx),
        }
    }
}

// ── overview wire types ─────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OverviewResponse {
    #[serde(default)]
    products: Vec<ProductOverview>,
    status: Option<FeedIdentity>,
}

#[derive(Deserialize)]
struct ProductOverview {
    product: FeedProduct,
    #[serde(default)]
    channels: Vec<ChannelSummary>,
}

#[derive(Deserialize)]
struct FeedIdentity {
    product: FeedProduct,
}

#[derive(Clone, Deserialize)]
struct ChannelSummary {
    key: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    member_count: usize,
}

impl ChannelSummary {
    fn label(&self) -> String {
        if !self.name.is_empty() {
            return self.name.clone();
        }
        let tail = self
            .key
            .split_once(':')
            .map_or(self.key.as_str(), |(_, tail)| tail);
        tail.to_owned()
    }
}

async fn fetch_overview(backend: &str, feed_id: &str) -> Result<Vec<ProductOverview>, String> {
    let url = format!(
        "{}/v1/feeds/{}/overview",
        backend.trim_end_matches('/'),
        urlencoding::encode(feed_id)
    );
    let overview = get_json::<OverviewResponse>(&url).await?;
    if !overview.products.is_empty() {
        return Ok(overview.products);
    }
    Ok(overview
        .status
        .into_iter()
        .map(|status| ProductOverview {
            product: status.product,
            channels: Vec::new(),
        })
        .collect())
}
