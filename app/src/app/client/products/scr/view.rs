//! `StarCraft: Remastered`'s own window — the Terran console.
//!
//! Not a dressed-up copy of `StarCraft II`'s. Remastered is a single-channel
//! realm: there is no tab strip because the channel *is* the window, and `/join`
//! swaps the whole thing in place rather than opening another. The header is
//! chromeless — the session rule and the roster header already name the channel,
//! so the titlebar keeps only the gateway and the portrait.
//!
//! Its roster consumes Remastered's own presence and avatar model. Those values
//! are adapted from LegacyChat plus the ToonProfile/Url services; they are not
//! substituted with StarCraft II's portrait-table types.
//!
//! From the `Realm Chat Windows` design, SC:R — TERRAN CONSOLE.

use gpui::{PathBuilder, canvas, point};

use super::*;

/// the Terran console's colours, which are nothing like `StarCraft II`'s.
/// The terminal pass retired the phosphor green: red is the chrome, and
/// everything else is a rust temperature ramp.
mod colour {
    /// the window itself — near-black with a red undertone.
    pub const SHELL: u32 = 0x0004_0302;
    /// the titlebar's veil over the window art (rgba).
    pub const HEADER: u32 = 0x0402_02d9;
    /// the hairline under the titlebar (rgba).
    pub const HEADER_EDGE: u32 = 0xc93a_2c59;
    /// the gateway readout.
    pub const READOUT: u32 = 0x00d8_8070;
    /// the muted rust for text that only reports.
    pub const MUTED: u32 = 0x008a_5a50;
    /// body text at the top of the ramp.
    pub const TEXT: u32 = 0x00f0_e6da;
    /// the accent — chrome red at full brightness.
    pub const ACCENT: u32 = 0x00ff_8a78;
}

const COMPOSER_HEIGHT: f32 = 32.0;
/// the frame StarCraft II's workspace keeps (its theme's `MARGIN`): panels
/// float inside it, and the composer row runs underneath them both.
const MARGIN: f32 = 22.0;

impl SuperiorityView {
    /// the whole Remastered window.
    pub(in crate::app::client) fn scr_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync_text_inputs(cx);
        let viewport_height = f32::from(window.viewport_size().height);
        let scanlines = self.chrome.modal_textures.scanline_veil(viewport_height);
        let vignette = self.chrome.modal_textures.vignette();
        div()
            .id("scr-window")
            // the same focus and keyboard the shared window takes: without
            // these the composer accepts a caret and nothing else — no Enter,
            // no commands
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .bg(rgb(colour::SHELL))
            .text_color(rgb(colour::TEXT))
            // the chosen backdrop fills the whole window, titlebar included;
            // a scrim rises from the left where the transcript reads, and the
            // floor fades to black under the composer row
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .overflow_hidden()
                    .child(
                        img(self.settings.background(Product::Remastered))
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(relative(0.55))
                            .bg(gpui::linear_gradient(
                                90.0,
                                gpui::linear_color_stop(rgba(0x0403_02b8), 0.0),
                                gpui::linear_color_stop(rgba(0x0403_0200), 1.0),
                            )),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom(px(66.0))
                            .h(px(124.0))
                            .bg(gpui::linear_gradient(
                                180.0,
                                gpui::linear_color_stop(rgba(0x0201_0100), 0.0),
                                gpui::linear_color_stop(rgba(0x0201_01bf), 1.0),
                            )),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .bottom_0()
                            .h(px(66.0))
                            .bg(gpui::linear_gradient(
                                180.0,
                                gpui::linear_color_stop(rgba(0x0201_01bf), 0.0),
                                gpui::linear_color_stop(rgba(0x0201_01f2), 1.0),
                            )),
                    ),
            )
            .child(self.scr_header(window, cx))
            // the same shape StarCraft II's workspace keeps: the panels float
            // inside a margin and the composer row runs under both of them,
            // so the roster no longer runs to the window's edge
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(MARGIN))
                    .pt(px(19.0))
                    .px(px(MARGIN))
                    .pb(px(MARGIN))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .gap(px(MARGIN))
                            .child(
                                div()
                                    .flex()
                                    .flex_1()
                                    .flex_col()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .relative()
                                            .flex_1()
                                            .overflow_hidden()
                                            .child(self.scr_transcript()),
                                    ),
                            )
                            .child(self.scr_roster(window, cx)),
                    )
                    .child(self.scr_footer(window, cx)),
            )
            // the tube itself: interference scanlines and a corner vignette
            // over everything, dialogs excepted — they hang above this window
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .child(
                        img(gpui::ImageSource::from(scanlines))
                            .absolute()
                            .inset_0()
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    )
                    .child(
                        img(gpui::ImageSource::from(vignette))
                            .absolute()
                            .inset_0()
                            .size_full()
                            .object_fit(ObjectFit::Fill),
                    ),
            )
            .into_any_element()
    }

    /// gateway, latency, and who you are. No channel plate: two other things on
    /// screen already name the channel.
    fn scr_header(&self, _window: &mut Window, cx: &mut Context<Self>) -> Stateful<Div> {
        // the header is the title bar, so it is also where the window is
        // dragged from and where the platform puts its controls
        #[cfg(target_os = "macos")]
        let controls = 78.0;
        #[cfg(target_os = "windows")]
        let controls = 0.0;
        let header = div()
            .id("scr-header")
            .relative()
            .h(px(ui_account_pattern::HEADER_HEIGHT))
            .flex()
            .items_center()
            .gap(px(12.0))
            .pl(px(14.0))
            .pr(px(14.0))
            .bg(rgba(colour::HEADER))
            .border_b_1()
            .border_color(rgba(colour::HEADER_EDGE))
            .child(div().w(px(controls)).flex_shrink_0())
            .child(div().flex_1());
        #[cfg(target_os = "windows")]
        let header = header.window_control_area(WindowControlArea::Drag);
        #[cfg(target_os = "macos")]
        let header = header.on_mouse_down(MouseButton::Left, |_, window, cx| {
            cx.stop_propagation();
            platform::begin_window_drag(window);
        });
        let identity = self.scr_account_identity();
        let account_open = self.overlays.active == Some(Overlay::Account);
        let account = ui_scr_account::AccountButton::new(
            identity.portrait,
            identity.name.clone(),
            account_open,
        )
        .on_click(cx.listener(|this, _, window, cx| {
            if let Some(scr) = this.session.scr_mut() {
                scr.composer_focused = false;
                scr.roster.focused = false;
            }
            if this.overlays.active == Some(Overlay::Account) {
                this.dismiss_overlay(window, cx);
            } else {
                this.overlays.active = Some(Overlay::Account);
                this.overlays.closing = false;
                cx.notify();
            }
        }));
        header
            .child(
                // the readout names the gateway; no latency figure, because
                // none is measured
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(colour::READOUT))
                    .child(
                        superiority_core::games::scr::session::DEFAULT_GATEWAY_NAME.to_uppercase(),
                    ),
            )
            .child(account)
    }

    pub(in crate::app::client) fn scr_account_identity(&self) -> ScrAccountIdentity {
        let account_battle_tag = self.session.account_battle_tag.as_deref();
        let scr = self
            .session
            .scr()
            .expect("the Remastered surface has Remastered UI state");
        let member = scr.local_member(account_battle_tag);
        let channel = scr.channel.as_ref().map(|channel| channel.title.as_str());
        let name = member.map_or_else(
            || {
                account_battle_tag
                    .unwrap_or("YOUR BATTLE.NET ACCOUNT")
                    .to_owned()
            },
            |member| member.name.clone(),
        );
        let (detail, detail_color) = member.map_or_else(
            || {
                let detail = match self.session.connection.stage {
                    ConnectionStage::Connected => channel.map_or_else(
                        || "Presence unknown · Battle.net".to_owned(),
                        |channel| format!("Presence unknown · {channel}"),
                    ),
                    ConnectionStage::Disconnected => "Offline".to_owned(),
                    _ => "Connecting…".to_owned(),
                };
                (detail, colour::MUTED)
            },
            |member| {
                let presence = scr_roster_presence(member.presence);
                (
                    channel.map_or_else(
                        || presence.label().to_owned(),
                        |channel| format!("{} · {channel}", presence.label()),
                    ),
                    presence.color(),
                )
            },
        );
        ScrAccountIdentity {
            name,
            detail,
            detail_color,
            portrait: member
                .and_then(|member| member.avatar_url.clone())
                .unwrap_or_else(avatar::default_source)
                .into(),
        }
    }

    /// what has been said, with SC2's scroll/selection/reveal behavior in the
    /// Remastered renderer. The shared transcript column pins a short history
    /// to the composer without making a long history unscrollable.
    fn scr_transcript(&self) -> ui_scr_chat::TranscriptViewport {
        let scr = self
            .session
            .scr()
            .expect("the Remastered surface has Remastered UI state");
        let mut transcript = ui_scr_chat::TranscriptViewport::new(
            "scr-chat-transcript",
            scr.transcript.selection.clone(),
        )
        .scroll(&scr.transcript.scroll);
        let Some(channel) = scr.channel.as_ref() else {
            return transcript.empty("JOIN A CHANNEL TO BEGIN CHATTING");
        };
        let now = Instant::now();
        let online = channel.members.len();
        for (index, line) in channel.transcript.iter().enumerate() {
            let line = shared_scr_transcript_line(line, &channel.members, online);
            let opacity = scr.transcript_reveal.as_ref().map_or(1.0, |reveal| {
                if reveal.channel_id != channel.id || reveal.line_index != index {
                    return 1.0;
                }
                ease_in_out(
                    (now.saturating_duration_since(reveal.started).as_secs_f32()
                        / SCR_CHAT_ENTRY_REVEAL_DURATION.as_secs_f32())
                    .clamp(0.0, 1.0),
                )
            });
            transcript = transcript.child(div().opacity(opacity).child(
                ui_scr_chat::selectable_transcript_row(
                    &line,
                    u64::from(channel.id),
                    index,
                    &scr.transcript.selection,
                ),
            ));
        }
        transcript
    }

    /// who is here, using SC:R's own presence and avatar presentation.
    fn scr_roster(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ui_scr_roster::RosterPanel {
        let scr = self
            .session
            .scr()
            .expect("the Remastered surface has Remastered UI state");
        let focused = scr.roster.focused;
        let (channel_id, title, total, filter) = scr.channel.as_ref().map_or_else(
            || (u32::MAX, "No channel".to_owned(), 0, String::new()),
            |channel| {
                (
                    channel.id,
                    channel.title.clone(),
                    channel.members.len(),
                    channel.roster_filter.clone(),
                )
            },
        );
        let filtered = scr.visible_members().len();
        let item_count = usize::from(scr.channel.is_some()) + filtered;
        let scroll = scr.roster.scroll.clone();
        let base_scroll = scroll.0.borrow().base_handle.clone();
        let now = Instant::now();
        let animating = scr.channel.as_ref().is_some_and(|channel| {
            scr.roster
                .animation
                .as_ref()
                .is_some_and(|animation| animation.scope == channel.id && animation.is_running(now))
        });
        let members = if animating {
            {
                scr.visible_members()
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            }
        } else {
            Default::default()
        };
        let selected = scr.selected_member();
        let animation = animating.then_some(scr.roster.animation.as_ref()).flatten();
        let rows = ui_roster_pattern::animated_rows(
            members,
            animation,
            now,
            |member| member.handle,
            SCR_ROSTER_ROW_HEIGHT,
            SCR_ROSTER_ROW_GAP,
            |member, motion| {
                if motion == ui_roster_pattern::RowMotion::Removed {
                    let handle = member.handle;
                    ui_scr_roster::RosterRow::new(
                        format!("scr-roster-member-removed-{handle}"),
                        format!("scr-roster-member-removed-{handle}"),
                        shared_scr_roster_user(member),
                        selected == Some(handle),
                    )
                    .into_any_element()
                } else {
                    self.scr_roster_row(member, selected == Some(member.handle), cx)
                        .into_any_element()
                }
            },
        );
        let mut list = ui_scr_roster::list_layer("scr-roster-list")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    let input = this
                        .session
                        .scr()
                        .expect("Remastered state")
                        .roster_input
                        .clone();
                    if let Some(scr) = this.session.scr_mut() {
                        scr.composer_focused = false;
                        scr.roster.focused = true;
                    }
                    input.focus(window, cx);
                    cx.notify();
                }),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(scr) = this.session.scr_mut() {
                    scr.set_selected_member(None);
                }
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                if let Some(scr) = this.session.scr_mut()
                    && scr.roster.animation.take().is_some()
                {
                    cx.notify();
                }
            }));
        list = if animating {
            list.overflow_y_scroll()
                .track_scroll(&base_scroll)
                .child(ui_roster_pattern::virtual_row_slot(
                    ui_scr_roster::segment_header(filtered),
                    SCR_ROSTER_ROW_HEIGHT,
                    SCR_ROSTER_ROW_GAP,
                ))
                .children(rows)
        } else {
            list.child(
                uniform_list(
                    "scr-roster-users",
                    item_count,
                    cx.processor(|this, range: Range<usize>, _, cx| {
                        let (members, selected) = this.session.scr().map_or_else(
                            || (Vec::new(), None),
                            |scr| {
                                let members = scr
                                    .visible_members()
                                    .into_iter()
                                    .cloned()
                                    .collect::<Vec<_>>();
                                (members, scr.selected_member())
                            },
                        );
                        range
                            .filter_map(|index| {
                                let row = if index == 0 {
                                    ui_scr_roster::segment_header(members.len()).into_any_element()
                                } else {
                                    let member = members.get(index - 1)?;
                                    this.scr_roster_row(member, selected == Some(member.handle), cx)
                                        .into_any_element()
                                };
                                Some(
                                    ui_roster_pattern::virtual_row_slot(
                                        row,
                                        SCR_ROSTER_ROW_HEIGHT,
                                        SCR_ROSTER_ROW_GAP,
                                    )
                                    .into_any_element(),
                                )
                            })
                            .collect::<Vec<_>>()
                    }),
                )
                .size_full()
                .track_scroll(&scroll),
            )
        };
        let list = list.vertical_scrollbar_in(
            &base_scroll,
            ui_shared_modal::ModalVariant::Remastered.scrollbar(),
            window,
            cx,
        );
        let model = ui_scr_roster::RosterHeaderModel::new(title, total, filtered, &filter, focused);
        let mut header = ui_scr_roster::RosterHeader::new(
            format!("scr-roster-header-{channel_id}"),
            model,
            focused,
        )
        .on_focus(cx.listener(|this, _, window, cx| {
            this.focus_scr_roster(window, cx);
        }));
        if !filter.is_empty() {
            header = header.on_clear(cx.listener(|this, _, _, cx| {
                if let Some(scr) = this.session.scr_mut() {
                    scr.roster_input.clear();
                    scr.set_roster_filter(String::new());
                }
                cx.stop_propagation();
                cx.notify();
            }));
        }
        ui_scr_roster::RosterPanel::new(header, list)
            .overlay(
                div()
                    .absolute()
                    .right(px(4.0))
                    .top(px(4.0))
                    .size(px(1.0))
                    .opacity(0.001)
                    .child(scr.roster_input.element()),
            )
            .focused(focused)
            .on_hover(cx.listener(|this, hovered, window, cx| {
                this.set_scr_roster_pointer_focus(*hovered, window, cx);
            }))
    }

    fn scr_roster_row(
        &self,
        user: &ScrMember,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> ui_scr_roster::RosterRow {
        let handle = user.handle;
        let group = format!("scr-roster-member-{handle}");
        ui_scr_roster::RosterRow::new(
            format!("scr-roster-member-{handle}"),
            group,
            shared_scr_roster_user(user),
            selected,
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            let input = this
                .session
                .scr()
                .expect("Remastered state")
                .roster_input
                .clone();
            if let Some(scr) = this.session.scr_mut() {
                scr.composer_focused = false;
                scr.roster.focused = true;
                scr.set_selected_member(Some(handle));
            }
            input.focus(window, cx);
            cx.stop_propagation();
            cx.notify();
        }))
    }

    /// Remastered's composer uses the shared text-editing primitive, but owns
    /// its field chrome and command policy.
    fn scr_composer(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let scr = self
            .session
            .scr()
            .expect("the Remastered surface has Remastered UI state");
        let has_channel = scr.channel.is_some();
        let focused = scr.composer_focused && scr.composer.is_focused(window);
        scr.composer.set_placeholder(if has_channel {
            "Press Enter to chat"
        } else {
            "Use /join to enter a channel"
        });
        scr.composer
            .set_ink(ui_inputs::field_ink(ui_inputs::ModalVariant::Remastered));
        div().w_full().flex_shrink_0().child(
            // the modern field in Remastered's tongue: a prompt line,
            // underline only — the box was never the console's idiom
            ui_inputs::dressed(
                div()
                    .id("scr-composer")
                    .h(px(COMPOSER_HEIGHT))
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(2.0))
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(13.5))
                    .text_color(rgb(0x00ff_d0c8)),
                ui_inputs::ModalVariant::Remastered,
                focused,
                false,
            )
            .child(ui_inputs::prompt_glyph(
                ui_inputs::ModalVariant::Remastered,
                focused,
                ui_inputs::FieldLife::Ready,
                false,
            ))
            .cursor(gpui::CursorStyle::IBeam)
            .on_hover(cx.listener(|this, hovered, window, cx| {
                this.set_scr_composer_pointer_focus(*hovered, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.set_scr_composer_pointer_focus(true, window, cx);
                }),
            )
            .child(scr.composer.element()),
        )
    }

    /// the composer row, spanning the window the way StarCraft II's does:
    /// the field, and Social seated to its right.
    fn scr_footer(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let social_open = self.overlays.active == Some(Overlay::Friends);
        let online_friends = self
            .session
            .social
            .friends
            .iter()
            .filter(|friend| friend.is_online())
            .count();
        // the lobby seat: a hairline-bordered readout in the console's own
        // idiom — bunker glyph, count beside it, no floating badge
        let social = div()
            .id("scr-social-toggle")
            .w(px(56.0))
            .h(px(COMPOSER_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .bg(if social_open {
                rgba(0xc93a_2c29)
            } else {
                rgba(0x1406_04cc)
            })
            .border_1()
            .border_color(if social_open {
                rgb(0x00ff_6a58)
            } else {
                rgba(0xc93a_2c99)
            })
            .rounded(px(2.0))
            .cursor_pointer()
            .hover(|button| {
                button
                    .bg(rgba(0xc93a_2c29))
                    .border_color(rgb(colour::ACCENT))
            })
            .active(|button| button.opacity(0.78))
            .child(scr_bunker_glyph())
            .child(
                div()
                    .font_family(ui_scr_theme::FONT_INTERFACE)
                    .text_size(px(10.5))
                    .text_color(rgb(colour::READOUT))
                    .child(online_friends.to_string()),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                if let Some(scr) = this.session.scr_mut() {
                    scr.composer_focused = false;
                    scr.roster.focused = false;
                }
                cx.stop_propagation();
                if this.overlays.active == Some(Overlay::Friends) && !this.overlays.closing {
                    this.dismiss_overlay(window, cx);
                } else {
                    this.session.social.social_detail_open = false;
                    this.session.social.social_pane_transition = None;
                    this.session.social.conversation_peer = None;
                    this.session.social.conversation_input.clear();
                    this.session.social.conversation_focused = false;
                    this.overlays.active = Some(Overlay::Friends);
                    this.overlays.closing = false;
                    cx.notify();
                }
            }));
        div()
            .flex()
            .gap(px(8.0))
            .flex_shrink_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(self.scr_composer(window, cx)),
            )
            .child(social)
    }

    pub(in crate::app::client) fn set_scr_composer_pointer_focus(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let can_focus = self.overlays.active.is_none()
            && !self.session.connection.dialog_visible
            && self.warnings.warning_dialog.is_none()
            && !self.updates.update_dialog_visible
            && self.session.scr().is_some_and(|scr| scr.channel.is_some());
        if hovered && can_focus {
            let input = self
                .session
                .scr()
                .expect("Remastered state")
                .composer
                .clone();
            if let Some(scr) = self.session.scr_mut() {
                scr.composer_focused = true;
                scr.roster.focused = false;
                scr.transcript.selection.clear();
            }
            input.focus(window, cx);
            cx.notify();
        } else if !hovered
            && self
                .session
                .scr()
                .is_some_and(|scr| scr.composer.is_focused(window))
        {
            if let Some(scr) = self.session.scr_mut() {
                scr.composer_focused = false;
            }
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn focus_scr_roster(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(scr) = self.session.scr() else {
            return;
        };
        if scr.channel.is_none() {
            return;
        }
        if scr.roster.focused && scr.roster_input.is_focused(window) {
            return;
        }
        let input = scr.roster_input.clone();
        if let Some(scr) = self.session.scr_mut() {
            scr.composer_focused = false;
            scr.roster.focused = true;
            scr.transcript.selection.clear();
        }
        input.focus(window, cx);
        cx.notify();
    }

    fn set_scr_roster_pointer_focus(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let can_focus = self.overlays.active.is_none()
            && !self.session.connection.dialog_visible
            && self.warnings.warning_dialog.is_none()
            && !self.updates.update_dialog_visible
            && self.session.scr().is_some_and(|scr| scr.channel.is_some());
        if hovered && can_focus {
            self.focus_scr_roster(window, cx);
        } else if !hovered
            && self
                .session
                .scr()
                .is_some_and(|scr| scr.roster_input.is_focused(window))
        {
            if let Some(scr) = self.session.scr_mut() {
                scr.roster.focused = false;
            }
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    pub(in crate::app::client) fn on_scr_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.platform
            && event.keystroke.key.eq_ignore_ascii_case("c")
            && !self.text_input_focused(window)
            && self
                .session
                .scr()
                .is_some_and(|scr| scr.transcript.selection.has_selection())
        {
            let rows = self
                .session
                .scr()
                .and_then(|scr| {
                    let channel = scr.channel.as_ref()?;
                    let online = channel.members.len();
                    Some(
                        channel
                            .transcript
                            .iter()
                            .enumerate()
                            .map(|(index, line)| {
                                let line =
                                    shared_scr_transcript_line(line, &channel.members, online);
                                (index, ui_scr_chat::transcript_text(&line))
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_default();
            if let Some(text) = self
                .session
                .scr()
                .and_then(|scr| scr.transcript.selection.selected_text(&rows))
            {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                cx.stop_propagation();
                return;
            }
        }
        if self.session.scr().is_some_and(|scr| scr.roster.focused) {
            let handled = match event.keystroke.key.as_str() {
                "escape" => {
                    let filter_active = self
                        .session
                        .scr()
                        .and_then(|scr| scr.channel.as_ref())
                        .is_some_and(|channel| !channel.roster_filter.is_empty());
                    if let Some(scr) = self.session.scr_mut() {
                        if filter_active {
                            scr.roster_input.clear();
                            scr.set_roster_filter(String::new());
                        } else {
                            scr.roster.focused = false;
                        }
                    }
                    if !filter_active {
                        self.focus_handle.focus(window, cx);
                    }
                    true
                }
                "up" => self
                    .session
                    .scr_mut()
                    .is_some_and(|scr| scr.move_member_selection(-1)),
                "down" => self
                    .session
                    .scr_mut()
                    .is_some_and(|scr| scr.move_member_selection(1)),
                "home" => self
                    .session
                    .scr_mut()
                    .is_some_and(|scr| scr.select_member_index(0)),
                "end" => {
                    let last = self
                        .session
                        .scr()
                        .and_then(|scr| scr.visible_members().len().checked_sub(1));
                    last.is_some_and(|last| {
                        self.session
                            .scr_mut()
                            .is_some_and(|scr| scr.select_member_index(last))
                    })
                }
                "pageup" | "pagedown" => {
                    let rows = self.session.scr().map_or(1, |scr| {
                        (f32::from(
                            scr.roster
                                .scroll
                                .0
                                .borrow()
                                .base_handle
                                .bounds()
                                .size
                                .height,
                        ) / (SCR_ROSTER_ROW_HEIGHT + SCR_ROSTER_ROW_GAP))
                            .floor()
                            .max(1.0) as isize
                    });
                    let delta = if event.keystroke.key == "pageup" {
                        -rows
                    } else {
                        rows
                    };
                    self.session
                        .scr_mut()
                        .is_some_and(|scr| scr.move_member_selection(delta))
                }
                _ => false,
            };
            if handled {
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        match event.keystroke.key.as_str() {
            "enter" | "return" => {
                self.send_scr_message(cx);
                cx.stop_propagation();
            }
            "escape" => {
                if let Some(scr) = self.session.scr_mut() {
                    scr.composer_focused = false;
                }
                self.focus_handle.focus(window, cx);
                cx.stop_propagation();
                cx.notify();
            }
            _ => {}
        }
    }

    fn send_scr_message(&mut self, cx: &mut Context<Self>) {
        let Some(scr) = self.session.scr() else {
            return;
        };
        let body = scr.composer.content().trim().to_owned();
        if body.is_empty() {
            return;
        }
        if let Some(target) = scr_join_target(&body) {
            if let Some(commands) = &self.session.commands {
                let _ = commands.send(ClientCommand::JoinClassic(target.to_owned()));
            }
            if let Some(scr) = self.session.scr_mut() {
                scr.composer.clear();
            }
            cx.notify();
            return;
        }
        if body.starts_with('/') {
            if let Some(commands) = &self.session.commands {
                let _ = commands.send(ClientCommand::SendClassicCommand(body));
            }
            if let Some(scr) = self.session.scr_mut() {
                scr.composer.clear();
            }
            cx.notify();
            return;
        }
        if scr.channel.is_none() {
            return;
        }
        if let Some(commands) = &self.session.commands {
            let _ = commands.send(ClientCommand::SendMessage {
                channel_index: 0,
                body,
            });
        }
        if let Some(scr) = self.session.scr_mut() {
            scr.composer.clear();
        }
        cx.notify();
    }
}

/// the lobby seat's mark: a bunker, drawn in two strokes — the block of the
/// roof, and the sloped hull line under it.
fn scr_bunker_glyph() -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let origin = bounds.origin;
            let mut roof = PathBuilder::stroke(px(1.5));
            roof.move_to(point(origin.x + px(4.5), origin.y + px(1.5)));
            roof.line_to(point(origin.x + px(9.5), origin.y + px(1.5)));
            roof.line_to(point(origin.x + px(9.5), origin.y + px(5.5)));
            roof.line_to(point(origin.x + px(4.5), origin.y + px(5.5)));
            roof.close();
            if let Ok(path) = roof.build() {
                window.paint_path(path, rgb(colour::ACCENT));
            }
            let mut hull = PathBuilder::stroke(px(1.5));
            hull.move_to(point(origin.x + px(1.5), origin.y + px(12.5)));
            hull.line_to(point(origin.x + px(1.5), origin.y + px(10.0)));
            hull.line_to(point(origin.x + px(4.0), origin.y + px(7.5)));
            hull.line_to(point(origin.x + px(10.0), origin.y + px(7.5)));
            hull.line_to(point(origin.x + px(12.5), origin.y + px(10.0)));
            hull.line_to(point(origin.x + px(12.5), origin.y + px(12.5)));
            if let Ok(path) = hull.build() {
                window.paint_path(path, rgb(colour::ACCENT));
            }
        },
    )
    .w(px(14.0))
    .h(px(14.0))
    .flex_shrink_0()
}

fn shared_scr_transcript_line(
    line: &ScrLine,
    members: &[ScrMember],
    online: usize,
) -> ScrTranscriptLine {
    match line {
        ScrLine::SessionStart { time, channel } => ScrTranscriptLine::SessionStart {
            time: time.clone(),
            channel: channel.clone(),
            online,
        },
        ScrLine::Message {
            time,
            kind,
            sender,
            text,
        } => {
            let portrait = members
                .iter()
                .find(|member| member.name.eq_ignore_ascii_case(sender))
                .and_then(|member| member.avatar_url.clone())
                .unwrap_or_else(avatar::default_source)
                .into();
            ScrTranscriptLine::Message {
                time: time.clone(),
                kind: match kind {
                    ScrMessageKind::Talk => ScrTranscriptMessageKind::Talk,
                    ScrMessageKind::Whisper => ScrTranscriptMessageKind::Whisper,
                    ScrMessageKind::Emote => ScrTranscriptMessageKind::Emote,
                },
                sender: ScrTranscriptSpeaker {
                    name: sender.clone(),
                    portrait,
                },
                text: text.clone(),
            }
        }
        ScrLine::Notice { time, kind, text } => ScrTranscriptLine::Notice {
            time: time.clone(),
            kind: match kind {
                ScrNoticeKind::Broadcast => ScrTranscriptNoticeKind::Broadcast,
                ScrNoticeKind::Information => ScrTranscriptNoticeKind::Information,
            },
            text: text.clone(),
        },
        ScrLine::Error { time, text } => ScrTranscriptLine::Error {
            time: time.clone(),
            text: text.clone(),
        },
    }
}

fn shared_scr_roster_user(member: &ScrMember) -> ScrRosterUser {
    ScrRosterUser {
        handle: member.handle,
        name: member.name.clone(),
        presence: scr_roster_presence(member.presence),
        portrait: member
            .avatar_url
            .clone()
            .unwrap_or_else(avatar::default_source)
            .into(),
        is_operator: member.is_operator,
    }
}

const fn scr_roster_presence(presence: ScrPresence) -> ScrRosterPresence {
    match presence {
        ScrPresence::Online => ScrRosterPresence::Online,
        ScrPresence::Away => ScrRosterPresence::Away,
        ScrPresence::Busy => ScrRosterPresence::Busy,
        ScrPresence::InGame => ScrRosterPresence::InGame,
        ScrPresence::InLobby => ScrRosterPresence::InLobby,
    }
}

fn scr_join_target(body: &str) -> Option<&str> {
    let (command, target) = body.trim().split_once(char::is_whitespace)?;
    matches!(command.to_ascii_lowercase().as_str(), "/join" | "/j")
        .then_some(target.trim())
        .filter(|target| !target.is_empty())
}

#[cfg(test)]
mod tests {
    use super::scr_join_target;

    #[test]
    fn remastered_join_commands_keep_the_whole_channel_name() {
        assert_eq!(
            scr_join_target("/join Op Superiority"),
            Some("Op Superiority")
        );
        assert_eq!(scr_join_target("/J public chat"), Some("public chat"));
        assert_eq!(scr_join_target("/join"), None);
        assert_eq!(scr_join_target("hello there"), None);
    }
}
