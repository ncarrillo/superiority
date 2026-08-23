//! The shared modal demo — `SUPERIORITY_PREVIEW_MODAL=1`.
//!
//! One realm's shell at a time, centred on a dark stage: switch dressings,
//! replay the open and close, and flip reduced motion, all without a
//! session. The friends list inside is design-time dressing straight off
//! `Canvas.dc.html`; the component under demonstration is the frame, the
//! title, and the motion.

use super::*;

pub(in crate::app::client) const MODAL_WIDTH: f32 = 380.0;
pub(in crate::app::client) const MODAL_HEIGHT: f32 = 560.0;
pub(in crate::app::client) const ERROR_WIDTH: f32 = 560.0;
pub(in crate::app::client) const ERROR_HEIGHT: f32 = 300.0;

pub(in crate::app::client) struct ModalPreviewComponent {
    /// Whether the demo owns the window. Only its own flag sets this.
    pub(in crate::app::client) showing: bool,
    /// Which realm the shell is dressed as, as an index into
    /// [`ui_shared_modal::ModalVariant::ALL`].
    pub(in crate::app::client) variant: usize,
    /// Whether the last replay was a close. Fill-mode both: a closed modal
    /// stays dark until it is opened again.
    pub(in crate::app::client) closing: bool,
    /// Bumped per replay. The animation ids inside the panel are stable, so
    /// the container's id carries this and every press starts the beats over.
    pub(in crate::app::client) generation: u64,
    pub(in crate::app::client) reduced: bool,
    /// Whether the shell is shown gone wrong: the error dressing, with the
    /// connection-lost fixture inside it.
    pub(in crate::app::client) alarm: bool,
    pub(in crate::app::client) textures: ui_shared_modal::ModalTextures,
}

impl ModalPreviewComponent {
    pub(in crate::app::client) fn new(showing: bool, reduced: bool) -> Self {
        Self {
            showing,
            variant: 0,
            closing: false,
            generation: 0,
            reduced,
            alarm: false,
            textures: ui_shared_modal::ModalTextures::default(),
        }
    }

    /// Chooses a realm and replays its entrance, so switching dressings also
    /// demonstrates the dressing's own arrival.
    pub(in crate::app::client) fn select(&mut self, variant: usize) {
        if variant >= ui_shared_modal::ModalVariant::ALL.len() {
            return;
        }
        self.variant = variant;
        self.open();
    }

    pub(in crate::app::client) fn open(&mut self) {
        self.closing = false;
        self.generation += 1;
    }

    pub(in crate::app::client) fn close(&mut self) {
        self.closing = true;
        self.generation += 1;
    }

    pub(in crate::app::client) fn toggle_reduced(&mut self) {
        self.reduced = !self.reduced;
        self.generation += 1;
    }

    pub(in crate::app::client) fn toggle_alarm(&mut self) {
        self.alarm = !self.alarm;
        self.open();
    }

    fn shown(&self) -> ui_shared_modal::ModalVariant {
        ui_shared_modal::ModalVariant::ALL[self.variant]
    }
}

/// What one realm's mock roster is dressed in. Fixture tokens from the doc,
/// not a live session's.
struct MockVoice {
    font: &'static str,
    row_height: f32,
    portrait: f32,
    portrait_border: Option<(u32, u32)>,
    framed: bool,
    name_size: f32,
    name: u32,
    name_dim: u32,
    online: u32,
    offline: u32,
    header: u32,
    count: u32,
    hover: u32,
}

/// The rule that carries the section header to the frame's edge.
fn header_rule(variant: ui_shared_modal::ModalVariant) -> Div {
    let rule = div().flex_1().h(px(1.0));
    match variant {
        ui_shared_modal::ModalVariant::Sc2 => rule.bg(rgba(0x33a8_f059)),
        ui_shared_modal::ModalVariant::Remastered => rule.bg(rgba(0xc924_1a80)),
        ui_shared_modal::ModalVariant::Reforged => rule.bg(gpui::linear_gradient(
            90.0,
            linear_color_stop(rgba(0x8a6d_3bcc), 0.0),
            linear_color_stop(rgba(0x8a6d_3b00), 1.0),
        )),
    }
}

fn voice(variant: ui_shared_modal::ModalVariant) -> MockVoice {
    use ui_shared_modal::ModalVariant::{Reforged, Remastered, Sc2};
    match variant {
        Sc2 => MockVoice {
            font: FONT_INTERNATIONAL,
            row_height: 34.0,
            portrait: 20.0,
            portrait_border: None,
            framed: true,
            name_size: 12.5,
            name: 0x00d6_e0f0,
            name_dim: 0x00d6_e0f0,
            online: 0x0047_d184,
            offline: 0x003a_4a5e,
            header: 0x00e6_f9ff,
            count: 0x007d_8fa8,
            hover: 0x1231_5e59,
        },
        Remastered => MockVoice {
            font: FONT_INTERFACE,
            row_height: 32.0,
            portrait: 20.0,
            portrait_border: Some((0x003a_3634, 0x003a_3634)),
            framed: false,
            name_size: 12.5,
            name: 0x00d8_f0e0,
            name_dim: 0x00d8_d4c8,
            online: 0x003f_dc3f,
            offline: 0x003a_4038,
            header: 0x00e8_d43f,
            count: 0x008a_7d6e,
            hover: 0x3fdc_3f0f,
        },
        Reforged => MockVoice {
            font: ui_wc3_theme::FONT_TITLE,
            row_height: 34.0,
            portrait: 22.0,
            portrait_border: Some((0x008a_6d3b, 0x004a_3a22)),
            framed: false,
            name_size: 13.5,
            name: 0x00f2_e8d0,
            name_dim: 0x00e8_dcc0,
            online: 0x007c_c46a,
            offline: 0x003a_3226,
            header: 0x00e8_c874,
            count: 0x009c_8a6e,
            hover: 0xe8c8_7414,
        },
    }
}

/// Who the mock knows. Remastered's doc draws every seat with the marine.
fn friends(variant: ui_shared_modal::ModalVariant) -> [(&'static str, &'static str, bool); 4] {
    let portraits = if variant == ui_shared_modal::ModalVariant::Remastered {
        ["images/portraits/marine.png"; 4]
    } else {
        [
            "images/portraits/zeratul.png",
            "images/portraits/marine.png",
            "images/portraits/kerrigan.png",
            "images/portraits/zergling.png",
        ]
    };
    [
        ("NelsonTest91", portraits[0], true),
        ("Carlos Perez", portraits[1], false),
        ("Echoes", portraits[2], false),
        ("HoggySquaddy", portraits[3], false),
    ]
}

fn friend_row(
    index: usize,
    name: &'static str,
    art: &'static str,
    online: bool,
    voice: &MockVoice,
) -> Stateful<Div> {
    let portrait: AnyElement = if voice.framed {
        div()
            .relative()
            .flex_shrink_0()
            .w(px(24.0))
            .h(px(24.0))
            .child(
                img(art)
                    .absolute()
                    .left(px(2.0))
                    .top(px(2.0))
                    .w(px(voice.portrait))
                    .h(px(voice.portrait))
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                img("images/nine-patch/portraits/frame.png")
                    .absolute()
                    .inset_0()
                    .w(px(24.0))
                    .h(px(24.0))
                    .object_fit(ObjectFit::Fill),
            )
            .into_any_element()
    } else {
        let (lit, dim) = voice.portrait_border.unwrap_or_default();
        img(art)
            .flex_shrink_0()
            .w(px(voice.portrait))
            .h(px(voice.portrait))
            .object_fit(ObjectFit::Cover)
            .border_1()
            .border_color(rgb(if online { lit } else { dim }))
            .into_any_element()
    };
    let dot = div()
        .flex_shrink_0()
        .w(px(8.0))
        .h(px(8.0))
        .rounded_full()
        .bg(rgb(if online { voice.online } else { voice.offline }))
        .when(online, |dot| {
            dot.shadow(vec![
                gpui::BoxShadow::new(px(0.0), px(0.0), rgba((voice.online << 8) | 0xe6).into())
                    .blur_radius(px(6.0)),
            ])
        });
    let hover = voice.hover;
    div()
        .id(("modal-preview-friend", index))
        .h(px(voice.row_height))
        .flex()
        .items_center()
        .gap(px(9.0))
        .px(px(4.0))
        .cursor_pointer()
        .when(!online, |row| row.opacity(0.45))
        .hover(move |row| row.bg(rgba(hover)))
        .child(portrait)
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_family(voice.font)
                .text_size(px(voice.name_size))
                .text_color(rgb(if online { voice.name } else { voice.name_dim }))
                .child(name),
        )
        .child(dot)
}

/// The connection-lost fixture inside the alarm dressing — the legacy
/// orange hex dialog's modern descendants, one per realm, with the modern
/// buttons in their error channels. The copy is the doc's; the buttons
/// replay the demo's own open and close.
fn alarm_panel(
    variant: ui_shared_modal::ModalVariant,
    textures: &ui_shared_modal::ModalTextures,
    cx: &mut Context<SuperiorityView>,
) -> Div {
    use ui_shared_modal::ModalVariant::{Reforged, Remastered, Sc2};
    let (title, body_size, body_colour) = match variant {
        Sc2 => ("ERROR", 14.5, 0x00d6_e0f0),
        Remastered => ("TRANSMISSION LOST", 13.0, 0x00e0_c8c0),
        Reforged => ("Connection Severed", 14.5, 0x00e8_dcc0),
    };
    let mut content = ui_shared_modal::content(variant)
        .items_center()
        .px(px(32.0))
        .pb(px(12.0));
    if variant == Remastered {
        content = content
            .child(ui_shared_modal::hazard_bar(textures, ERROR_WIDTH - 96.0))
            .child(div().h(px(16.0)));
    } else {
        content = content.child(div().h(px(14.0)));
    }
    content = content.child(ui_shared_modal::error_title(variant, title));
    content = match variant {
        Sc2 => content.child(
            div()
                .mt(px(6.0))
                .child(ui_shared_modal::error_subtitle(variant, "UPLINK LOST")),
        ),
        Remastered => content.child(div().mt(px(8.0)).child(ui_shared_modal::error_subtitle(
            variant,
            "/// SIGNAL TERMINATED BY REMOTE HOST ///",
        ))),
        Reforged => content.child(div().mt(px(10.0)).child(ui_shared_modal::error_divider())),
    };
    let seat = |button: Stateful<Div>| {
        if variant == Remastered {
            button.h(px(36.0)).px(px(10.0))
        } else {
            button.w(px(170.0)).h(px(40.0))
        }
    };
    let quit_tone = if variant == Sc2 {
        // the whole SC2 dialog speaks in the alert channel, quit included
        ui_buttons::ButtonTone::Danger
    } else {
        ui_buttons::ButtonTone::Chrome
    };
    let quit_label = if variant == Reforged { "Quit" } else { "QUIT" };
    let reconnect_label = if variant == Reforged {
        "Reconnect"
    } else {
        "RECONNECT"
    };
    let content = content
        .child(
            div()
                .mt(px(20.0))
                .max_w(px(430.0))
                .text_size(px(body_size))
                .text_color(rgb(body_colour))
                .text_center()
                .when(variant == Reforged, gpui::Styled::italic)
                .child("The connection to Battle.net was lost. Reconnect or quit Superiority."),
        )
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .gap(px(if variant == Remastered { 26.0 } else { 14.0 }))
                .child(
                    seat(ui_buttons::button(
                        "alarm-quit",
                        variant,
                        ui_buttons::ButtonWeight::Ghost,
                        quit_tone,
                        ui_buttons::ButtonLife::Ready,
                        ui_buttons::worded(variant, quit_label),
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.modal_preview.close();
                        cx.notify();
                    })),
                )
                .child(
                    seat(ui_buttons::button(
                        "alarm-reconnect",
                        variant,
                        ui_buttons::ButtonWeight::Primary,
                        ui_buttons::ButtonTone::Danger,
                        ui_buttons::ButtonLife::Ready,
                        ui_buttons::worded(variant, reconnect_label),
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.modal_preview.open();
                        cx.notify();
                    })),
                ),
        );
    div()
        .relative()
        .w(px(ERROR_WIDTH))
        .h(px(ERROR_HEIGHT))
        .child(ui_shared_modal::error_frame(
            variant,
            ERROR_WIDTH,
            ERROR_HEIGHT,
            textures,
        ))
        .child(content)
}

impl SuperiorityView {
    /// The demo page: realm switches, open/close replays, reduced motion —
    /// and the shell itself, centred where a dialog would sit.
    pub(in crate::app::client) fn modal_preview_view(
        &mut self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let variant = self.modal_preview.shown();
        let closing = self.modal_preview.closing;
        let reduced = self.modal_preview.reduced;
        let generation = self.modal_preview.generation;
        let chosen = self.modal_preview.variant;
        let alarm = self.modal_preview.alarm;

        let frame = ui_shared_modal::frame(
            variant,
            MODAL_WIDTH,
            MODAL_HEIGHT,
            &self.modal_preview.textures,
        );
        let voice = voice(variant);
        let friend_rows = friends(variant)
            .into_iter()
            .enumerate()
            .map(|(index, (name, art, online))| friend_row(index, name, art, online, &voice))
            .collect::<Vec<_>>();
        let mock_title = if variant == ui_shared_modal::ModalVariant::Reforged {
            "Social"
        } else {
            "SOCIAL"
        };
        let panel = div()
            .relative()
            .w(px(MODAL_WIDTH))
            .h(px(MODAL_HEIGHT))
            .child(frame)
            .child(
                ui_shared_modal::content(variant)
                    .child(ui_shared_modal::title(variant, mock_title))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .mt(px(18.0))
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(rgb(voice.online))
                                    .child("▼"),
                            )
                            .child(
                                div()
                                    .font_family(if variant == ui_shared_modal::ModalVariant::Sc2 {
                                        FONT_NAVIGATION
                                    } else {
                                        voice.font
                                    })
                                    .font_weight(FontWeight::BOLD)
                                    .text_size(px(11.0))
                                    .text_color(rgb(voice.header))
                                    .child(if variant == ui_shared_modal::ModalVariant::Reforged {
                                        "Friends"
                                    } else {
                                        "FRIENDS"
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(rgb(voice.count))
                                    .child("1 of 6 online"),
                            )
                            .child(header_rule(variant)),
                    )
                    .child(div().flex().flex_col().mt(px(6.0)).children(friend_rows))
                    // the ✕ measures its inset from the content box, the
                    // way the doc places it
                    .child(
                        ui_shared_modal::close_glyph(variant)
                            .id("modal-preview-close")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.modal_preview.close();
                                cx.notify();
                            })),
                    ),
            )
            .id("shared-modal-panel");
        let staged = if alarm {
            let panel = alarm_panel(variant, &self.modal_preview.textures, cx)
                .id("shared-modal-alarm-panel");
            ui_shared_modal::animated(variant, panel, closing, reduced, ERROR_WIDTH, ERROR_HEIGHT)
        } else {
            ui_shared_modal::animated(variant, panel, closing, reduced, MODAL_WIDTH, MODAL_HEIGHT)
        };

        let chip = |label: SharedString, accent: u32, edge: u32, active: bool| {
            div()
                .px(px(8.0))
                .py(px(2.0))
                .border_1()
                .border_color(rgba(edge))
                .text_size(px(10.0))
                .text_color(rgb(accent))
                .cursor_pointer()
                .when(active, |chip| chip.bg(rgba((accent << 8) | 0x24)))
                .child(label)
        };

        div()
            .id("modal-preview")
            .size_full()
            .flex()
            .flex_col()
            .font_family(FONT_INTERFACE)
            .bg(gpui::linear_gradient(
                180.0,
                linear_color_stop(rgba(0x0c12_20ff), 0.0),
                linear_color_stop(rgba(0x060a_12ff), 0.6),
            ))
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "1" => this.modal_preview.select(0),
                    "2" => this.modal_preview.select(1),
                    "3" => this.modal_preview.select(2),
                    "o" | "space" | "enter" | "return" => this.modal_preview.open(),
                    "c" | "escape" => this.modal_preview.close(),
                    "r" => this.modal_preview.toggle_reduced(),
                    "x" => this.modal_preview.toggle_alarm(),
                    _ => return,
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .px(px(28.0))
                    .py(px(16.0))
                    .child(
                        div()
                            .font_family(FONT_NAVIGATION)
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(13.0))
                            .text_color(rgb(0x00e6_f9ff))
                            .child("SHARED MODAL FRAME"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0x007d_8fa8))
                            .child("1/2/3 realm · O open · C close · X error dressing · R reduced motion · content is design-time mock"),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .gap(px(10.0))
                    .children(ui_shared_modal::ModalVariant::ALL.into_iter().enumerate().map(
                        |(index, candidate)| {
                            let (accent, edge) = match candidate {
                                ui_shared_modal::ModalVariant::Sc2 => (0x006b_c2f2, 0x33a8_f080),
                                ui_shared_modal::ModalVariant::Remastered => {
                                    (0x00e8_734a, 0xc924_1a80)
                                }
                                ui_shared_modal::ModalVariant::Reforged => {
                                    (0x00e8_c874, 0x8a6d_3bb3)
                                }
                            };
                            chip(candidate.label().into(), accent, edge, index == chosen)
                                .id(("modal-preview-variant", index))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.modal_preview.select(index);
                                    cx.notify();
                                }))
                        },
                    ))
                    .child(
                        chip("▶ OPEN".into(), 0x006b_c2f2, 0x33a8_f080, false)
                            .id("modal-preview-open")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.modal_preview.open();
                                cx.notify();
                            })),
                    )
                    .child(
                        chip("▶ CLOSE".into(), 0x006b_c2f2, 0x33a8_f080, false)
                            .id("modal-preview-do-close")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.modal_preview.close();
                                cx.notify();
                            })),
                    )
                    .child(
                        chip(
                            if alarm {
                                "ERROR DRESSING · ON".into()
                            } else {
                                "ERROR DRESSING · OFF".into()
                            },
                            0x00f0_a030,
                            0xf0a0_3080,
                            alarm,
                        )
                        .id("modal-preview-alarm")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.modal_preview.toggle_alarm();
                            cx.notify();
                        })),
                    )
                    .child(
                        chip(
                            if reduced {
                                "REDUCED MOTION · ON".into()
                            } else {
                                "REDUCED MOTION · OFF".into()
                            },
                            0x007d_8fa8,
                            0x7d8f_a880,
                            reduced,
                        )
                        .id("modal-preview-reduced")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.modal_preview.toggle_reduced();
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div().flex_1().flex().items_center().justify_center().child(
                    div()
                        .id(("shared-modal-run", generation))
                        .child(staged),
                ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .flex()
                    .justify_center()
                    .pb(px(16.0))
                    .text_size(px(10.0))
                    .text_color(rgb(0x005a_6478))
                    .child(
                        "SC2 opens on the legacy scan reveal · SC:R power-cycles in steps · WC3:R drops from above and sinks away",
                    ),
            )
            .into_any_element()
    }
}
