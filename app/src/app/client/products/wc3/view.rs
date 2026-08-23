//! Reforged's realm hall, corrected to the design language (`Realm Chat
//! Windows`, WC3:R SHELL): a plain OS border with the stone living inside,
//! a chromeless titlebar whose identity plaque IS the menu, square rank
//! tiles in the roster, and zero web-blue anywhere.
//!
//! The header buttons are gone — Settings and Games List live in the account
//! popout behind the plaque, in the order the other two realms keep. The public-realm rail went
//! with them: a single-channel realm's window is the channel, and `/join`
//! in the composer is the door.

use gpui::{PathBuilder, canvas, point};

use super::*;

const WC3_WORKSPACE_MARGIN: f32 = 18.0;

impl SuperiorityView {
    pub(in crate::app::client) fn wc3_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.sync_text_inputs(cx);
        div()
            .id("wc3-window")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(ui_wc3_theme::SHELL))
            .font_family(ui_wc3_theme::FONT_TITLE)
            .text_color(rgb(ui_wc3_theme::PARCHMENT))
            // the hall's own light: ember rising from the lower-right, a
            // touch of moonlight at the upper-left — the doc's radials,
            // said with the gradients gpui has
            .child(div().absolute().inset_0().bg(linear_gradient(
                315.0,
                gpui::linear_color_stop(rgba(0xd878_2822), 0.0),
                gpui::linear_color_stop(rgba(0xd878_2800), 0.65),
            )))
            .child(div().absolute().inset_0().bg(linear_gradient(
                135.0,
                gpui::linear_color_stop(rgba(0x5a6e_8c12), 0.0),
                gpui::linear_color_stop(rgba(0x5a6e_8c00), 0.6),
            )))
            .child(self.wc3_titlebar(window, cx))
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(WC3_WORKSPACE_MARGIN))
                    .pt(px(19.0))
                    .px(px(WC3_WORKSPACE_MARGIN))
                    .pb(px(WC3_WORKSPACE_MARGIN))
                    // Reforged participates in the same selected backdrop as
                    // the other realms; the warm stone scrim keeps its own
                    // parchment-and-gold voice over any catalogue image.
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .overflow_hidden()
                            .child(
                                img(self.settings.background(Product::Warcraft3))
                                    .size_full()
                                    .object_fit(ObjectFit::Fill),
                            )
                            .child(div().absolute().inset_0().bg(rgba(0x0b08_05c7)))
                            .child(div().absolute().inset_0().bg(linear_gradient(
                                315.0,
                                gpui::linear_color_stop(rgba(0xd878_281a), 0.0),
                                gpui::linear_color_stop(rgba(0xd878_2800), 0.7),
                            ))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .gap(px(WC3_WORKSPACE_MARGIN))
                            .child(
                                div()
                                    .relative()
                                    .flex()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .overflow_hidden()
                                    // the workspace scrim already provides
                                    // contrast. Keep the transcript itself
                                    // clear so it never exposes a rectangular
                                    // top/bottom gradient over the backdrop.
                                    .child(self.wc3_transcript()),
                            )
                            .child(self.wc3_roster(window, cx)),
                    )
                    .child(self.wc3_footer(window, cx)),
            )
            .into_any_element()
    }

    /// the titlebar is the channel-tab strip, in carved stone: StarCraft II's
    /// tab geometry in Reforged's material (the `Channel Tabs` design), with
    /// the identity plaque at its end. The plaque is the menu. Each joined
    /// AuroraChat room owns a tab; + starts a `/join`.
    fn wc3_titlebar(&self, _window: &Window, cx: &mut Context<Self>) -> Stateful<Div> {
        let now = Instant::now();
        let effect = [
            self.wc3_tab_effect_opacity(1, now),
            self.wc3_tab_effect_opacity(2, now),
            self.wc3_tab_effect_opacity(3, now),
        ];
        let wc3 = self
            .session
            .wc3()
            .expect("the Reforged surface has Reforged UI state");
        let active_channel = wc3.active_channel;
        let drag_offsets = wc3.navigation.tabs.offsets(now, wc3.channels.len());
        let (closing_index, closing_progress) =
            wc3.tab_close.as_ref().map_or((None, 0.0), |closing| {
                let elapsed = closing.started.map_or(0.0, |started| {
                    now.saturating_duration_since(started).as_secs_f32()
                });
                (
                    Some(closing.index),
                    ease_in_out((elapsed / TAB_CLOSE_DURATION.as_secs_f32()).clamp(0.0, 1.0)),
                )
            });
        let identity = self
            .session
            .account_battle_tag
            .clone()
            .unwrap_or_else(|| "Adventurer".to_owned());
        let portrait = self.wc3_local_portrait();
        let region =
            GamesComponent::region_name(self.session.account_region).unwrap_or("Battle.net");
        let account_open = self.overlays.active == Some(Overlay::Account);

        // every hall is a tab, and the tab does everything StarCraft II's
        // does: click to choose, drag to reorder, hover to read a long name,
        // × to fold it closed and leave
        let items = wc3
            .channels
            .iter()
            .enumerate()
            .map(|(index, channel)| {
                let id = u64::from(channel.id);
                ui_navigation::ChannelTab::new(
                    format!("wc3-channel-tab-{id}"),
                    channel.name.clone(),
                )
                .active(index == active_channel)
                .unread(channel.unread)
                .hovered(wc3.hovered_tab == Some(id))
                .marquee_offset(wc3.navigation.tabs.name_offset(&id, now))
                .drag_offset(drag_offsets[index])
                .dragged_travel(
                    wc3.navigation
                        .tabs
                        .is_dragging(index)
                        .then(|| wc3.navigation.tabs.dragged_travel()),
                )
                .close_progress((closing_index == Some(index)).then_some(closing_progress))
                .effect_opacity(effect)
                .on_mouse_down(cx.listener(move |this, event, _, cx| {
                    this.begin_wc3_tab_pointer(index, event, cx);
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.click_wc3_tab(index, cx);
                }))
                .on_hover(cx.listener(
                    move |this, event: &ui_navigation::TabHoverEvent, window, cx| {
                        this.set_wc3_tab_name_hover(id, event.hovered, event.travel, window, cx);
                    },
                ))
                .on_close(cx.listener(move |this, _, window, cx| {
                    this.begin_wc3_tab_close(index, window, cx);
                }))
            })
            .collect();
        let tab_leading = if cfg!(target_os = "windows") {
            TAB_BAR_HEIGHT
        } else {
            TAB_LEADING_SPACE
        };
        let tabs = ui_navigation::ChannelTabs::new(items, self.chrome.ui_assets.clone())
            .variant(ui_navigation::ModalVariant::Reforged)
            .leading(tab_leading)
            // + starts a join the way the composer does it: the command is
            // typed for you, the hall's name is yours to finish
            .on_add(cx.listener(|this, _, window, cx| {
                let Some(wc3) = this.session.wc3() else {
                    return;
                };
                let composer = wc3.composer.clone();
                composer.set_content("/join ");
                composer.set_cursor("/join ".len());
                if let Some(wc3) = this.session.wc3_mut() {
                    wc3.roster.focused = false;
                    wc3.composer_focused = true;
                }
                composer.focus(window, cx);
                cx.notify();
            }));
        let control_end = tab_leading + tabs.tail() + 47.0;

        #[cfg(target_os = "windows")]
        let drag_right = platform::WINDOW_CONTROLS_WIDTH + 200.0;
        #[cfg(target_os = "macos")]
        let drag_right = 200.0;
        let drag_region = div()
            .id("wc3-window-drag-region")
            .absolute()
            .top_0()
            .left(px(control_end))
            .right(px(drag_right))
            .h(px(TAB_BAR_HEIGHT));
        #[cfg(target_os = "windows")]
        let drag_region = drag_region.window_control_area(WindowControlArea::Drag);
        #[cfg(target_os = "macos")]
        let drag_region = drag_region.on_mouse_down(MouseButton::Left, |_, window, cx| {
            cx.stop_propagation();
            platform::begin_window_drag(window);
        });

        let plaque = wc3_identity_plaque(&identity, region, account_open, &portrait)
            .id("wc3-account-plaque")
            .on_click(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                if let Some(wc3) = this.session.wc3_mut() {
                    wc3.composer_focused = false;
                    wc3.roster.focused = false;
                    wc3.transcript.selection.clear();
                }
                if this.overlays.active == Some(Overlay::Account) {
                    this.dismiss_overlay(window, cx);
                } else {
                    this.overlays.active = Some(Overlay::Account);
                    this.overlays.closing = false;
                    cx.notify();
                }
            }));
        #[cfg(target_os = "windows")]
        let plaque_right = platform::WINDOW_CONTROLS_WIDTH + 16.0;
        #[cfg(target_os = "macos")]
        let plaque_right = 16.0;

        let header = ui_navigation::stone_bar()
            .id("wc3-titlebar")
            .child(tabs)
            .child(drag_region)
            .child(
                div()
                    .absolute()
                    .right(px(plaque_right))
                    .top_0()
                    .bottom_0()
                    .flex()
                    .items_center()
                    .child(plaque),
            );
        #[cfg(target_os = "windows")]
        let header = header.child(platform::window_controls(_window));
        header
    }

    /// the portrait the realm shows for this account: its member entry in
    /// whichever hall it stands in (the active one first), or Reforged's
    /// canonical fallback before any hall has answered.
    fn wc3_local_portrait(&self) -> String {
        let battle_tag = self.session.account_battle_tag.as_deref();
        self.session
            .wc3()
            .and_then(|wc3| {
                let battle_tag = battle_tag?;
                let account = strip_character_code(battle_tag);
                wc3.active()
                    .into_iter()
                    .chain(wc3.channels.iter())
                    .flat_map(|channel| channel.members.iter())
                    .find(|member| {
                        member.name.eq_ignore_ascii_case(battle_tag)
                            || member.visible_name().eq_ignore_ascii_case(account)
                    })
                    .map(|member| member.portrait.clone())
            })
            .unwrap_or_else(|| avatar::source(None))
    }

    /// the account popout: stone panel, micro corner studs, the plaque's own
    /// portrait header, then the menu the header buttons used to be — Settings,
    /// then Games List, the order StarCraft II and Remastered keep. Signing out
    /// belongs to the picker's top bar, not to a realm's menu.
    pub(in crate::app::client) fn wc3_account_popout(&self, cx: &mut Context<Self>) -> AnyElement {
        let identity = self
            .session
            .account_battle_tag
            .clone()
            .unwrap_or_else(|| "Adventurer".to_owned());
        let portrait = self.wc3_local_portrait();
        let channel = self
            .session
            .wc3()
            .and_then(Wc3SessionUi::active)
            .map(|channel| channel.name.clone());
        let presence = channel.map_or_else(
            || "Online".to_owned(),
            |channel| format!("Online · {channel}"),
        );
        let name = battle_tag_name(&identity);
        let stud = |left: bool, top: bool| {
            let stud = div()
                .absolute()
                .w(px(4.0))
                .h(px(4.0))
                .bg(rgb(ui_wc3_theme::GOLD));
            let stud = if left {
                stud.left(px(-2.0))
            } else {
                stud.right(px(-2.0))
            };
            if top {
                stud.top(px(-2.0))
            } else {
                stud.bottom(px(-2.0))
            }
        };
        let row = |id: &'static str, label: &'static str| {
            div()
                .id(id)
                .h(px(32.0))
                .flex()
                .items_center()
                .px(px(14.0))
                .text_size(px(13.5))
                .text_color(rgb(ui_wc3_theme::GOLD_DIM))
                .cursor_pointer()
                .hover(|row| {
                    row.bg(rgba(0xe8c8_741a))
                        .text_color(rgb(ui_wc3_theme::GOLD_BRIGHT))
                })
                .child(label)
        };
        let menu = div()
            .id("wc3-account-menu")
            .absolute()
            .right(px(14.0))
            .top(px(ui_wc3_theme::TITLEBAR_HEIGHT + 4.0))
            .w(px(250.0))
            .bg(rgba(0x130d_07f7))
            .border_1()
            .border_color(rgb(ui_wc3_theme::STONE_BRIGHT))
            .shadow(vec![
                gpui::BoxShadow::new(px(0.0), px(10.0), rgba(0x0000_00b3).into())
                    .blur_radius(px(30.0)),
                gpui::BoxShadow::new(px(0.0), px(0.0), rgba(0xe8c8_741a).into())
                    .blur_radius(px(14.0)),
            ])
            .font_family(ui_wc3_theme::FONT_TITLE)
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(stud(true, true))
            .child(stud(false, true))
            .child(stud(true, false))
            .child(stud(false, false))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .p(px(12.0))
                    .bg(linear_gradient(
                        180.0,
                        gpui::linear_color_stop(rgba(0xe8c8_7414), 0.0),
                        gpui::linear_color_stop(rgba(0xe8c8_7400), 1.0),
                    ))
                    .child(wc3_portrait_tile(&portrait, 38.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .flex()
                                    .text_size(px(13.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(ui_wc3_theme::PARCHMENT))
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.0))
                                    .text_size(px(10.5))
                                    .text_color(rgb(ui_wc3_theme::MUTED))
                                    .child(
                                        div()
                                            .size(px(5.0))
                                            .rounded_full()
                                            .bg(rgb(ui_wc3_theme::MOSS))
                                            .shadow(vec![
                                                gpui::BoxShadow::new(
                                                    px(0.0),
                                                    px(0.0),
                                                    rgba(0x7cc4_6acc).into(),
                                                )
                                                .blur_radius(px(5.0)),
                                            ]),
                                    )
                                    .child(presence),
                            ),
                    ),
            )
            .child(div().h(px(1.0)).mx(px(12.0)).bg(linear_gradient(
                90.0,
                gpui::linear_color_stop(rgba(0x8a6d_3bff), 0.0),
                gpui::linear_color_stop(rgba(0x8a6d_3b00), 1.0),
            )))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .py(px(6.0))
                    .child(row("wc3-menu-settings", "Settings").on_click(cx.listener(
                        |this, _, _, cx| {
                            cx.stop_propagation();
                            this.overlays.active = Some(Overlay::Settings);
                            this.overlays.closing = false;
                            cx.notify();
                        },
                    )))
                    .child(row("wc3-menu-games", "Games List").on_click(cx.listener(
                        |this, _, window, cx| {
                            cx.stop_propagation();
                            this.show_games_list(window, cx);
                        },
                    ))),
            );
        let overlay = div()
            .id("wc3-account-dismiss")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, window, cx| {
                this.dismiss_overlay(window, cx);
            }))
            .child(menu);
        self.overlays.animated(
            overlay,
            "wc3-account-overlay-open",
            "wc3-account-overlay-close",
            true,
        )
    }

    fn wc3_transcript(&self) -> ui_wc3_chat::TranscriptViewport {
        let wc3 = self
            .session
            .wc3()
            .expect("the Reforged surface has Reforged UI state");
        let mut transcript = ui_wc3_chat::TranscriptViewport::new(
            "wc3-chat-transcript",
            wc3.transcript.selection.clone(),
        )
        .scroll(&wc3.transcript.scroll);
        let Some(channel) = wc3.active() else {
            return transcript.empty("NO HALL ENTERED · /JOIN <REALM> OPENS A DOOR");
        };
        let scope = channel.name.to_lowercase();
        for (index, line) in channel.transcript.iter().enumerate() {
            transcript = transcript.child(ui_wc3_chat::selectable_transcript_row(
                line,
                &scope,
                index,
                &wc3.transcript.selection,
            ));
        }
        transcript
    }

    fn wc3_roster(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> ui_wc3_roster::RosterPanel {
        let now = Instant::now();
        let wc3 = self
            .session
            .wc3()
            .expect("the Reforged surface has Reforged UI state");
        let total = wc3.active().map_or(0, |channel| channel.members.len());
        let members = wc3
            .visible_members()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let filter = wc3
            .active()
            .map_or_else(String::new, |channel| channel.roster_filter.clone());
        let focused = wc3.roster.focused && wc3.roster_input.is_focused(window);
        let selected = wc3.selected_member();
        let scroll = wc3.roster.scroll.0.borrow().base_handle.clone();
        let roster_input = wc3.roster_input.clone();
        let animation = wc3.roster_animation(now);
        let filtered = members.len();
        // the same slots the other two rosters use: a row that left folds
        // away, one that arrived grows in. a row on its way out is drawn but
        // answers to nothing — whoever it was has gone
        let rows = ui_roster_pattern::animated_rows(
            members,
            animation,
            now,
            |member| member.handle,
            ui_wc3_theme::ROSTER_ROW_HEIGHT,
            ui_wc3_theme::ROSTER_ROW_GAP,
            |member, motion| {
                let handle = member.handle;
                let group = format!("wc3-roster-member-{handle}");
                if motion == ui_roster_pattern::RowMotion::Removed {
                    return ui_wc3_roster::RosterRow::new(
                        format!("wc3-roster-member-removed-{handle}"),
                        group,
                        shared_wc3_roster_user(member),
                        selected == Some(handle),
                    )
                    .into_any_element();
                }
                ui_wc3_roster::RosterRow::new(
                    group.clone(),
                    group,
                    shared_wc3_roster_user(member),
                    selected == Some(handle),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let input = this
                        .session
                        .wc3()
                        .expect("Reforged state")
                        .roster_input
                        .clone();
                    if let Some(wc3) = this.session.wc3_mut() {
                        wc3.composer_focused = false;
                        wc3.roster.focused = true;
                        wc3.set_selected_member(Some(handle));
                    }
                    input.focus(window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }))
                .into_any_element()
            },
        );
        let list = ui_wc3_roster::list_layer("wc3-roster-list")
            .overflow_y_scroll()
            .track_scroll(&scroll)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.focus_wc3_roster(window, cx);
                }),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(wc3) = this.session.wc3_mut() {
                    wc3.set_selected_member(None);
                }
                cx.notify();
            }))
            // a wheel turn mid-transition takes over: the slots settle at once
            // rather than fighting the scroll
            .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                if let Some(wc3) = this.session.wc3_mut()
                    && wc3.roster.animation.take().is_some()
                {
                    cx.notify();
                }
            }))
            .child(ui_wc3_roster::segment_header(filtered))
            .children(rows);
        let list = list.vertical_scrollbar_in(
            &scroll,
            ui_shared_modal::ModalVariant::Reforged.scrollbar(),
            window,
            cx,
        );
        let model =
            ui_wc3_roster::RosterHeaderModel::new("Players", total, filtered, &filter, focused);
        let mut header = ui_wc3_roster::RosterHeader::new("wc3-roster-header", model, focused)
            .on_focus(cx.listener(|this, _, window, cx| {
                this.focus_wc3_roster(window, cx);
            }));
        if !filter.is_empty() {
            header = header.on_clear(cx.listener(|this, _, _, cx| {
                if let Some(wc3) = this.session.wc3_mut() {
                    wc3.roster_input.clear();
                    wc3.set_roster_filter(String::new());
                }
                cx.stop_propagation();
                cx.notify();
            }));
        }
        ui_wc3_roster::RosterPanel::new(header, list)
            .overlay(
                div()
                    .absolute()
                    .right(px(4.0))
                    .top(px(4.0))
                    .size(px(1.0))
                    .opacity(0.001)
                    .child(roster_input.element()),
            )
            .focused(focused)
            .on_hover(cx.listener(|this, hovered, window, cx| {
                this.set_wc3_roster_pointer_focus(*hovered, window, cx);
            }))
    }

    fn focus_wc3_roster(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(wc3) = self.session.wc3() else {
            return;
        };
        if wc3.active().is_none() || (wc3.roster.focused && wc3.roster_input.is_focused(window)) {
            return;
        }
        let input = wc3.roster_input.clone();
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.composer_focused = false;
            wc3.roster.focused = true;
            wc3.transcript.selection.clear();
        }
        input.focus(window, cx);
        cx.notify();
    }

    fn set_wc3_roster_pointer_focus(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let can_focus = self.overlays.active.is_none()
            && !self.session.connection.dialog_visible
            && self.warnings.warning_dialog.is_none()
            && !self.updates.update_dialog_visible
            && self.session.wc3().is_some_and(|wc3| wc3.active().is_some());
        if hovered && can_focus {
            self.focus_wc3_roster(window, cx);
        } else if !hovered
            && self
                .session
                .wc3()
                .is_some_and(|wc3| wc3.roster_input.is_focused(window))
        {
            if let Some(wc3) = self.session.wc3_mut() {
                wc3.roster.focused = false;
            }
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn wc3_composer(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let wc3 = self
            .session
            .wc3()
            .expect("the Reforged surface has Reforged UI state");
        let has_channel = wc3.active().is_some();
        wc3.composer.set_placeholder(if has_channel {
            "Speak to the realm…"
        } else {
            "/join <realm> to enter a hall"
        });
        let focused = wc3.composer_focused && wc3.composer.is_focused(window);
        wc3.composer
            .set_ink(ui_inputs::field_ink(ui_inputs::ModalVariant::Reforged));
        div().w_full().flex_shrink_0().child(
            ui_inputs::dressed(
                div()
                    .id("wc3-composer")
                    .h(px(ui_wc3_theme::COMPOSER_HEIGHT))
                    .flex()
                    .items_center()
                    .px(px(12.0))
                    .font_family(ui_wc3_theme::FONT_INTERFACE)
                    .text_size(px(13.5))
                    .text_color(rgb(ui_wc3_theme::PARCHMENT)),
                ui_inputs::ModalVariant::Reforged,
                focused,
                false,
            )
            .cursor(gpui::CursorStyle::IBeam)
            .on_hover(cx.listener(|this, hovered, window, cx| {
                this.set_wc3_composer_pointer_focus(*hovered, window, cx);
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    this.set_wc3_composer_pointer_focus(true, window, cx);
                }),
            )
            .child(wc3.composer.element()),
        )
    }

    /// the shared workspace footer geometry, spoken in Reforged's stone and
    /// gold: the realm composer spans the remaining width and Social occupies
    /// the same fixed seat it does in SC:R and SC2.
    fn wc3_footer(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let clan_open = self.overlays.active == Some(Overlay::Clan);
        let social_open = self.overlays.active == Some(Overlay::Friends);
        let online_friends = self
            .session
            .social
            .friends
            .iter()
            .filter(|friend| friend.is_online())
            .count();
        // the realm's own bottom-bar seats: a stone plate with a bronze
        // border, hand-stroked gold icons, and — on SOCIAL — Superiority's
        // shell-orange signal badge. From the `Clan Hall` design.
        let social = div()
            .relative()
            .flex_shrink_0()
            .child(
                wc3_bar_seat("wc3-social-toggle", social_open)
                    .child(wc3_kin_glyph())
                    .on_click(cx.listener(|this, _, window, cx| {
                        if let Some(wc3) = this.session.wc3_mut() {
                            wc3.composer_focused = false;
                            wc3.roster.focused = false;
                            wc3.transcript.selection.clear();
                        }
                        cx.stop_propagation();
                        if this.overlays.active == Some(Overlay::Friends) && !this.overlays.closing
                        {
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
                    })),
            )
            .child(wc3_signal_badge(online_friends));
        let clan = wc3_bar_seat("wc3-clan-toggle", clan_open)
            .child(wc3_banner_glyph())
            .on_click(cx.listener(|this, _, window, cx| {
                if let Some(wc3) = this.session.wc3_mut() {
                    wc3.composer_focused = false;
                    wc3.roster.focused = false;
                    wc3.transcript.selection.clear();
                }
                cx.stop_propagation();
                if this.overlays.active == Some(Overlay::Clan) && !this.overlays.closing {
                    this.dismiss_overlay(window, cx);
                } else {
                    this.overlays.active = Some(Overlay::Clan);
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
                    .child(self.wc3_composer(window, cx)),
            )
            .child(clan)
            .child(social)
    }

    fn set_wc3_composer_pointer_focus(
        &mut self,
        hovered: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let can_focus = self.overlays.active.is_none()
            && !self.session.connection.dialog_visible
            && self.warnings.warning_dialog.is_none()
            && !self.updates.update_dialog_visible
            && self.session.wc3().is_some();
        if hovered && can_focus {
            let input = self.session.wc3().expect("Reforged state").composer.clone();
            if let Some(wc3) = self.session.wc3_mut() {
                wc3.roster.focused = false;
                wc3.composer_focused = true;
                wc3.transcript.selection.clear();
            }
            input.focus(window, cx);
            cx.notify();
        } else if !hovered
            && self
                .session
                .wc3()
                .is_some_and(|wc3| wc3.composer.is_focused(window))
        {
            if let Some(wc3) = self.session.wc3_mut() {
                wc3.composer_focused = false;
            }
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    pub(in crate::app::client) fn on_wc3_key_down(
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
                .wc3()
                .is_some_and(|wc3| wc3.transcript.selection.has_selection())
        {
            let rows = self
                .session
                .wc3()
                .and_then(|wc3| {
                    let channel = wc3.active()?;
                    Some(
                        channel
                            .transcript
                            .iter()
                            .enumerate()
                            .map(|(index, line)| (index, ui_wc3_chat::transcript_text(line)))
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_default();
            if let Some(text) = self
                .session
                .wc3()
                .and_then(|wc3| wc3.transcript.selection.selected_text(&rows))
            {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                cx.stop_propagation();
                return;
            }
        }
        if self.session.wc3().is_some_and(|wc3| wc3.roster.focused) {
            if event.keystroke.key == "escape" {
                let filter_active = self
                    .session
                    .wc3()
                    .and_then(Wc3SessionUi::active)
                    .is_some_and(|channel| !channel.roster_filter.is_empty());
                if let Some(wc3) = self.session.wc3_mut() {
                    if filter_active {
                        wc3.roster_input.clear();
                        wc3.set_roster_filter(String::new());
                    } else {
                        wc3.roster.focused = false;
                        self.focus_handle.focus(window, cx);
                    }
                }
                cx.stop_propagation();
                cx.notify();
            }
            return;
        }
        match event.keystroke.key.as_str() {
            "enter" | "return" => {
                self.send_wc3_message(cx);
                cx.stop_propagation();
            }
            "escape" => {
                if let Some(wc3) = self.session.wc3_mut() {
                    wc3.composer_focused = false;
                }
                self.focus_handle.focus(window, cx);
                cx.stop_propagation();
                cx.notify();
            }
            _ => {}
        }
    }

    pub(in crate::app::client) fn select_wc3_channel(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.overlays.active.is_some() {
            return;
        }
        let changed = self.session.wc3_mut().is_some_and(|wc3| {
            let changed = wc3.select_channel(index);
            if changed {
                wc3.composer_focused = false;
                wc3.roster.focused = false;
            }
            changed
        });
        if changed {
            cx.notify();
        }
    }

    pub(in crate::app::client) fn close_wc3_channel(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let channel_id = self
            .session
            .wc3_mut()
            .and_then(|wc3| wc3.remove_channel(index));
        let Some(channel_id) = channel_id else {
            return;
        };
        if let Some(commands) = &self.session.commands {
            let _ = commands.send(ClientCommand::LeaveChannel {
                channel_index: channel_id,
            });
        }
        cx.notify();
    }

    fn send_wc3_message(&mut self, cx: &mut Context<Self>) {
        let Some(wc3) = self.session.wc3() else {
            return;
        };
        let body = wc3.composer.content().trim().to_owned();
        if body.is_empty() {
            return;
        }
        // the verb answers in any case, the way the console's does
        let join_target = ["/join ", "/j "].into_iter().find_map(|verb| {
            body.get(..verb.len())
                .filter(|head| head.eq_ignore_ascii_case(verb))
                .map(|_| body[verb.len()..].trim())
        });
        if let Some(target) = join_target.filter(|target| !target.is_empty()) {
            let joined = wc3
                .channels
                .iter()
                .position(|channel| channel.name.eq_ignore_ascii_case(target));
            if let Some(index) = joined {
                if let Some(wc3) = self.session.wc3_mut() {
                    wc3.composer.clear();
                }
                self.select_wc3_channel(index, cx);
                return;
            }
            if let Some(commands) = &self.session.commands {
                let _ = commands.send(ClientCommand::JoinWarcraft(target.to_owned()));
            }
            if let Some(wc3) = self.session.wc3_mut() {
                wc3.composer.clear();
            }
            cx.notify();
            return;
        }
        if body.starts_with('/') {
            if let Some(wc3) = self.session.wc3_mut() {
                wc3.append_error(
                    Self::current_timestamp(),
                    "Reforged supports /join <realm> in this client.".into(),
                );
                wc3.composer.clear();
            }
            cx.notify();
            return;
        }
        let Some(channel_id) = wc3.active().map(|channel| channel.id) else {
            return;
        };
        if let Some(commands) = &self.session.commands {
            let _ = commands.send(ClientCommand::SendMessage {
                channel_index: channel_id,
                body,
            });
        }
        if let Some(wc3) = self.session.wc3_mut() {
            wc3.composer.clear();
        }
        cx.notify();
    }
}

/// a bottom-bar seat in the realm's stone: bronze border over a stone
/// gradient, warming at the pointer. The caller gives it a glyph and a click.
fn wc3_bar_seat(id: &'static str, open: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(42.0))
        .h(px(ui_wc3_theme::COMPOSER_HEIGHT))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(rgb(if open {
            ui_wc3_theme::GOLD
        } else {
            ui_wc3_theme::STONE
        }))
        .bg(linear_gradient(
            180.0,
            gpui::linear_color_stop(rgba(0x1a12_06ff), 0.0),
            gpui::linear_color_stop(rgba(0x0f0a_06ff), 1.0),
        ))
        .cursor_pointer()
        .hover(|seat| {
            seat.border_color(rgb(ui_wc3_theme::STONE_BRIGHT))
                .bg(linear_gradient(
                    180.0,
                    gpui::linear_color_stop(rgba(0x2418_08ff), 0.0),
                    gpui::linear_color_stop(rgba(0x140d_06ff), 1.0),
                ))
        })
        .active(|seat| seat.border_color(rgb(ui_wc3_theme::GOLD_BRIGHT)))
}

/// the CLAN mark: a banner hanging from its rod, a cross-guard on the cloth.
fn wc3_banner_glyph() -> AnyElement {
    // the design's 20-unit drawing, set at 18px
    let scale = 18.0 / 20.0;
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let at = |x: f32, y: f32| {
                point(
                    bounds.origin.x + px(x * scale),
                    bounds.origin.y + px(y * scale),
                )
            };
            let gold = rgb(ui_wc3_theme::GOLD);
            // the cloth, filled faintly then stroked
            let mut cloth = PathBuilder::fill();
            cloth.add_polygon(
                &[
                    at(5.5, 2.5),
                    at(14.5, 2.5),
                    at(14.5, 15.0),
                    at(10.0, 18.5),
                    at(5.5, 15.0),
                ],
                true,
            );
            if let Ok(path) = cloth.build() {
                window.paint_path(path, rgba(0xe8c8_7424));
            }
            let mut edge = PathBuilder::stroke(px(1.5 * scale));
            edge.add_polygon(
                &[
                    at(5.5, 2.5),
                    at(14.5, 2.5),
                    at(14.5, 15.0),
                    at(10.0, 18.5),
                    at(5.5, 15.0),
                ],
                true,
            );
            // the rod and its ends
            edge.move_to(at(3.0, 2.5));
            edge.line_to(at(17.0, 2.5));
            edge.move_to(at(3.0, 1.0));
            edge.line_to(at(3.0, 4.0));
            edge.move_to(at(17.0, 1.0));
            edge.line_to(at(17.0, 4.0));
            if let Ok(path) = edge.build() {
                window.paint_path(path, gold);
            }
            let mut guard = PathBuilder::stroke(px(1.3 * scale));
            guard.move_to(at(8.2, 7.0));
            guard.line_to(at(11.8, 7.0));
            guard.move_to(at(10.0, 7.0));
            guard.line_to(at(10.0, 11.0));
            if let Ok(path) = guard.build() {
                window.paint_path(path, gold);
            }
        },
    )
    .w(px(18.0))
    .h(px(18.0))
    .into_any_element()
}

/// the SOCIAL mark: two kin figures, the second a step back in bronze.
fn wc3_kin_glyph() -> AnyElement {
    // the design's 22×20 drawing, set at 18×16
    let scale = 18.0 / 22.0;
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let at = |x: f32, y: f32| {
                point(
                    bounds.origin.x + px(x * scale),
                    bounds.origin.y + px(y * scale),
                )
            };
            let ring = |builder: &mut PathBuilder, cx: f32, cy: f32, r: f32| {
                const STEPS: usize = 28;
                let points: Vec<_> = (0..STEPS)
                    .map(|step| {
                        let angle = std::f32::consts::TAU * step as f32 / STEPS as f32;
                        at(r.mul_add(angle.cos(), cx), r.mul_add(angle.sin(), cy))
                    })
                    .collect();
                builder.add_polygon(&points, true);
            };
            let mut kin = PathBuilder::stroke(px(1.5 * scale));
            ring(&mut kin, 8.0, 6.0, 3.0);
            kin.move_to(at(2.5, 17.5));
            kin.cubic_bezier_to(at(8.0, 11.5), at(2.5, 13.5), at(5.0, 11.5));
            kin.cubic_bezier_to(at(13.5, 17.5), at(11.0, 11.5), at(13.5, 13.5));
            if let Ok(path) = kin.build() {
                window.paint_path(path, rgb(ui_wc3_theme::GOLD));
            }
            let mut other = PathBuilder::stroke(px(1.3 * scale));
            ring(&mut other, 16.0, 7.5, 2.3);
            other.move_to(at(13.5, 16.0));
            other.cubic_bezier_to(at(17.0, 12.3), at(14.0, 13.5), at(15.5, 12.3));
            other.cubic_bezier_to(at(20.0, 16.0), at(18.7, 12.3), at(20.0, 13.7));
            if let Ok(path) = other.build() {
                window.paint_path(path, rgb(ui_wc3_theme::MUTED));
            }
        },
    )
    .w(px(18.0))
    .h(px(16.0))
    .into_any_element()
}

/// the signal badge on SOCIAL: Superiority speaking, not the game — the
/// shell's alert orange with a dark count, lifted off the seat's corner.
fn wc3_signal_badge(count: usize) -> Div {
    div()
        .absolute()
        .right(px(-7.0))
        .top(px(-7.0))
        .min_w(px(15.0))
        .h(px(15.0))
        .px(px(3.0))
        .flex()
        .items_center()
        .justify_center()
        .bg(rgb(0x00f0_a030))
        .border_1()
        .border_color(rgb(0x00ff_c25e))
        .shadow(vec![
            gpui::BoxShadow::new(px(0.0), px(0.0), rgba(0xf0a0_308c).into()).blur_radius(px(7.0)),
        ])
        .font_family(ui_wc3_theme::FONT_INTERFACE)
        .font_weight(FontWeight::BOLD)
        .text_size(px(8.0))
        .text_color(rgb(0x0014_0d05))
        .child(count.to_string())
}

/// the identity plaque: name and presence stacked against the rank tile,
/// under a gold breath that deepens when reached for. The caller wires the
/// click — the plaque is the account menu's handle.
fn wc3_identity_plaque(identity: &str, region: &'static str, open: bool, portrait: &str) -> Div {
    let name = battle_tag_name(identity);
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .py(px(4.0))
        .pl(px(12.0))
        .pr(px(4.0))
        .cursor_pointer()
        .bg(linear_gradient(
            90.0,
            gpui::linear_color_stop(rgba(0xe8c8_7400), 0.0),
            gpui::linear_color_stop(rgba(0xe8c8_7410), 1.0),
        ))
        .when(open, |plaque| {
            plaque.bg(linear_gradient(
                90.0,
                gpui::linear_color_stop(rgba(0xe8c8_7400), 0.0),
                gpui::linear_color_stop(rgba(0xe8c8_7421), 1.0),
            ))
        })
        .hover(|plaque| {
            plaque.bg(linear_gradient(
                90.0,
                gpui::linear_color_stop(rgba(0xe8c8_7400), 0.0),
                gpui::linear_color_stop(rgba(0xe8c8_7421), 1.0),
            ))
        })
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap(px(1.0))
                .child(
                    div()
                        .flex()
                        .text_size(px(12.5))
                        .text_color(rgb(ui_wc3_theme::PARCHMENT))
                        .child(name),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .text_size(px(10.0))
                        .text_color(rgb(ui_wc3_theme::MUTED))
                        .child(
                            div()
                                .size(px(6.0))
                                .rounded_full()
                                .bg(rgb(ui_wc3_theme::MOSS))
                                .shadow(vec![
                                    gpui::BoxShadow::new(
                                        px(0.0),
                                        px(0.0),
                                        rgba(0x7cc4_6acc).into(),
                                    )
                                    .blur_radius(px(5.0)),
                                ]),
                        )
                        .child(region),
                ),
        )
        .child(wc3_portrait_tile(portrait, 32.0))
}

/// a square portrait tile: the account's Reforged portrait in the same
/// stone-and-bronze frame the roster gives every member — square, never
/// rounded. The initial it used to carry was a placeholder for rank icons
/// that never came; the portrait is what the realm actually knows you by.
fn wc3_portrait_tile(portrait: &str, size: f32) -> Div {
    let inset = 2.0;
    div()
        .relative()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .border_1()
        .border_color(rgb(ui_wc3_theme::STONE_BRIGHT))
        .bg(linear_gradient(
            180.0,
            gpui::linear_color_stop(rgba(0x2a20_12ff), 0.0),
            gpui::linear_color_stop(rgba(0x1710_09ff), 1.0),
        ))
        .child(
            img(portrait.to_owned())
                .absolute()
                .left(px(inset))
                .top(px(inset))
                .size(px(size - 2.0 * (inset + 1.0)))
                .object_fit(ObjectFit::Cover),
        )
}

fn shared_wc3_roster_user(member: &Wc3Member) -> Wc3RosterUser {
    Wc3RosterUser {
        handle: member.handle,
        name: member.visible_name().to_owned(),
        presence: wc3_roster_presence(member.presence),
        portrait: member.portrait.clone().into(),
        clan_abbreviation: member.clan_abbreviation.clone(),
    }
}

const fn wc3_roster_presence(presence: Wc3Presence) -> Wc3RosterPresence {
    match presence {
        Wc3Presence::Offline => Wc3RosterPresence::Offline,
        Wc3Presence::Online => Wc3RosterPresence::Online,
        Wc3Presence::Away => Wc3RosterPresence::Away,
        Wc3Presence::Busy => Wc3RosterPresence::Busy,
    }
}

/// the name half of a battle tag: the realm knows you as `NelsonTest91`, and
/// the `#1458` is an address, not a name — it stays off the plaque.
fn battle_tag_name(identity: &str) -> String {
    strip_character_code(identity).to_owned()
}
