//! The Remastered Live view: a read-only [`Console`] host over the product's
//! feed. It owns one channel's console, adapts the product routes' DTOs into the
//! shared presentation models, and polls stream and roster while it is the
//! focused product.

use std::{
    collections::HashSet,
    hash::{DefaultHasher, Hash as _, Hasher as _},
};

use gpui::{
    ClipboardItem, Context, FocusHandle, Focusable, KeyDownEvent, Render, Window, div, prelude::*,
    px, rgb,
};
use superiority_ui::{
    foundation::text_input::TextInput,
    products::scr::{
        Console, ConsoleHost, RosterPresence, RosterUser, TranscriptLine, TranscriptMessageKind,
        TranscriptNoticeKind, TranscriptSpeaker, theme,
    },
};
use wasm_bindgen::JsValue;

use super::api::{
    ChannelSummary, Message, RosterMember, Sender, fetch_overview, fetch_roster, fetch_stream,
};

pub(crate) struct ScrLiveView {
    focus_handle: FocusHandle,
    backend: String,
    feed_id: String,
    asset_root: String,
    console: Console<f64>,
    roster_input: TextInput,
    active: bool,
    feed_state: String,
    had_session: bool,
    session_id: Option<String>,
    channels: Vec<ChannelSummary>,
    selected: Option<String>,
    latest_roster: Vec<RosterMember>,
    error: Option<String>,
}

impl ScrLiveView {
    pub(crate) fn new(
        backend: String,
        feed_id: String,
        asset_root: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = Self {
            focus_handle: cx.focus_handle(),
            backend: backend.trim_end_matches('/').to_owned(),
            feed_id,
            asset_root: asset_root.trim_end_matches('/').to_owned(),
            console: Console::new(),
            roster_input: TextInput::new("", cx),
            active: false,
            feed_state: "connecting".into(),
            had_session: false,
            session_id: None,
            channels: Vec::new(),
            selected: None,
            latest_roster: Vec::new(),
            error: None,
        };
        view.start_polling(cx);
        view
    }

    /// The shell parks every view but the focused one; a parked view keeps its
    /// console and stops polling streams.
    pub(crate) fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active != active {
            self.active = active;
            cx.notify();
        }
    }

    fn asset(&self, path: &str) -> String {
        format!("{}/{}", self.asset_root, path.trim_start_matches('/'))
    }

    fn avatar_source(&self, id: Option<&str>) -> gpui::ImageSource {
        let id = id.map_or("avatar_default_scrlogo", |id| {
            if id.is_empty() {
                "avatar_default_scrlogo"
            } else {
                id
            }
        });
        self.asset(&format!("ui/products/scr/avatars/{id}.png"))
            .into()
    }

    fn roster_user(&self, member: &RosterMember) -> RosterUser {
        RosterUser {
            handle: member.handle,
            name: member
                .name
                .clone()
                .unwrap_or_else(|| format!("User {}", member.handle)),
            presence: scr_presence(&member.presence),
            portrait: self.avatar_source(member.avatar.as_deref()),
            is_operator: member.is_operator,
        }
    }

    fn message_line(&self, message: &Message, roster: &[RosterMember]) -> Option<TranscriptLine> {
        let time = stamp_label(message.ts);
        match message.kind.as_str() {
            "member_joined" | "member_left" => {
                let who = message
                    .sender
                    .as_ref()
                    .map_or_else(|| "Someone".to_owned(), Sender::display_name);
                let verb = if message.kind == "member_left" {
                    "left"
                } else {
                    "entered"
                };
                Some(TranscriptLine::Notice {
                    time,
                    kind: TranscriptNoticeKind::Information,
                    text: format!("{who} {verb} the channel."),
                })
            }
            "broadcast" => Some(TranscriptLine::Notice {
                time,
                kind: TranscriptNoticeKind::Broadcast,
                text: message.body.clone(),
            }),
            "information" => Some(TranscriptLine::Notice {
                time,
                kind: TranscriptNoticeKind::Information,
                text: message.body.clone(),
            }),
            _ => {
                let Some(sender) = message.sender.as_ref() else {
                    // a senderless line the console cannot attribute; keep the
                    // words as a server notice rather than invent a speaker
                    return (!message.body.is_empty()).then(|| TranscriptLine::Notice {
                        time,
                        kind: TranscriptNoticeKind::Information,
                        text: message.body.clone(),
                    });
                };
                let portrait = roster
                    .iter()
                    .find(|member| member.handle == sender.handle)
                    .map_or_else(
                        || self.avatar_source(None),
                        |member| self.avatar_source(member.avatar.as_deref()),
                    );
                Some(TranscriptLine::Message {
                    time,
                    kind: if message.kind == "emote" {
                        TranscriptMessageKind::Emote
                    } else {
                        TranscriptMessageKind::Talk
                    },
                    sender: TranscriptSpeaker {
                        name: sender.display_name(),
                        portrait,
                    },
                    text: message.body.clone(),
                })
            }
        }
    }

    fn start_polling(&self, cx: &mut Context<Self>) {
        let backend = self.backend.clone();
        let feed_id = self.feed_id.clone();
        cx.spawn(async move |this, cx| {
            let mut tick = 0_u64;
            let mut applied: Option<String> = None;
            let mut cursor: Option<u64> = None;
            let mut appended: HashSet<u64> = HashSet::new();

            loop {
                let active = match this.read_with(cx, |view, _| view.active) {
                    Ok(active) => active,
                    Err(_) => break,
                };
                if !active {
                    gloo_timers::future::TimeoutFuture::new(500).await;
                    tick = tick.wrapping_add(1);
                    continue;
                }

                if tick.is_multiple_of(4) {
                    match fetch_overview(&backend, &feed_id).await {
                        Ok((status, channels)) => {
                            let session_id = status.session.as_ref().map(|s| s.id.clone());
                            let session_changed = this
                                .read_with(cx, |view, _| view.session_id != session_id)
                                .unwrap_or(false);
                            if this
                                .update(cx, |view, cx| {
                                    if view.session_id != session_id && session_id.is_some() {
                                        view.session_id = session_id.clone();
                                        view.console.clear();
                                    }
                                    view.feed_state = status.state.clone();
                                    view.had_session = status.session.is_some();
                                    view.channels = channels;
                                    if view.selected.as_ref().is_none_or(|selected| {
                                        !view.channels.iter().any(|c| &c.key == selected)
                                    }) {
                                        view.selected =
                                            view.channels.first().map(|c| c.key.clone());
                                    }
                                    view.error = None;
                                    cx.notify();
                                })
                                .is_err()
                            {
                                break;
                            }
                            if session_changed {
                                applied = None;
                                cursor = None;
                                appended.clear();
                            }
                        }
                        Err(error) => {
                            let _ = this.update(cx, |view, cx| {
                                view.error = Some(error);
                                cx.notify();
                            });
                        }
                    }
                }

                let selected = match this.read_with(cx, |view, _| view.selected.clone()) {
                    Ok(selected) => selected,
                    Err(_) => break,
                };
                if selected.as_deref() != applied.as_deref() {
                    cursor = None;
                    appended.clear();
                }

                if let Some(channel) = selected {
                    let id = channel_id(&channel);
                    match fetch_roster(&backend, &feed_id, &channel).await {
                        Ok(members) => {
                            if this
                                .update(cx, |view, cx| {
                                    view.latest_roster = members;
                                    let title = view
                                        .channels
                                        .iter()
                                        .find(|c| c.key == channel)
                                        .map_or_else(|| channel.clone(), ChannelSummary::label);
                                    let users = view
                                        .latest_roster
                                        .iter()
                                        .map(|member| view.roster_user(member))
                                        .collect::<Vec<_>>();
                                    view.console.apply_channel(
                                        id,
                                        title,
                                        users,
                                        stamp_label(now_ms()),
                                        js_sys::Date::now(),
                                    );
                                    cx.notify();
                                })
                                .is_err()
                            {
                                break;
                            }
                            applied = Some(channel.clone());
                        }
                        Err(error) => {
                            let _ = this.update(cx, |view, cx| {
                                view.error = Some(error);
                                cx.notify();
                            });
                        }
                    }

                    if applied.as_deref() == Some(channel.as_str())
                        && (cursor.is_none() || tick.is_multiple_of(6))
                    {
                        match fetch_stream(&backend, &feed_id, &channel, cursor).await {
                            Ok((messages, next)) => {
                                let fresh = this
                                    .update(cx, |view, cx| {
                                        let now = js_sys::Date::now();
                                        let mut fresh = Vec::new();
                                        for message in &messages {
                                            if appended.contains(&message.id) {
                                                continue;
                                            }
                                            if let Some(line) =
                                                view.message_line(message, &view.latest_roster)
                                            {
                                                view.console.append_line(line, now);
                                            }
                                            fresh.push(message.id);
                                        }
                                        view.error = None;
                                        cx.notify();
                                        fresh
                                    })
                                    .unwrap_or_default();
                                for id in fresh {
                                    appended.insert(id);
                                }
                                cursor = Some(next);
                            }
                            Err(error) => {
                                let _ = this.update(cx, |view, cx| {
                                    view.error = Some(error);
                                    cx.notify();
                                });
                            }
                        }
                    }
                }

                tick = tick.wrapping_add(1);
                gloo_timers::future::TimeoutFuture::new(500).await;
            }
        })
        .detach();
    }

    fn status_chip(&self) -> (String, u32) {
        if self.feed_state == "online" {
            ("LIVE".to_owned(), theme::ACCENT)
        } else if self.feed_state == "reconnecting" {
            ("RECONNECTING".to_owned(), theme::MUTED)
        } else if self.had_session {
            ("OFFLINE".to_owned(), theme::MUTED)
        } else {
            ("CONNECTING".to_owned(), theme::MUTED)
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if (event.keystroke.modifiers.platform || event.keystroke.modifiers.control)
            && event.keystroke.key.eq_ignore_ascii_case("c")
        {
            let selection = self.console.transcript_selection();
            if selection.has_selection() {
                let rows = self.console.transcript_rows_text();
                if let Some(text) = selection.selected_text(&rows) {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                    cx.stop_propagation();
                }
                return;
            }
        }
        if self.console.roster_focused() && event.keystroke.key == "escape" {
            if self.console.filter().is_empty() {
                self.console.set_roster_focused(false);
                self.focus_handle.focus(window, cx);
            } else {
                self.roster_input.clear();
                self.console.set_filter(String::new(), js_sys::Date::now());
            }
            cx.stop_propagation();
            cx.notify();
        }
    }
}

impl Focusable for ScrLiveView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ScrLiveView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = js_sys::Date::now();
        // the roster filter follows the hidden input, so paste and IME work
        let filter = self.roster_input.content();
        self.console.set_filter(filter, now);

        let title = self.console.title().unwrap_or("No channel").to_uppercase();
        let (status, status_color) = self.status_chip();

        div()
            .id("scr-live-window")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(theme::PANEL_BACKGROUND))
            .font_family(theme::FONT_INTERFACE)
            .text_color(rgb(theme::TEXT))
            .child(
                div()
                    .flex_shrink_0()
                    .h(px(38.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .px(px(16.0))
                    .bg(rgb(theme::PANEL_HEADER))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_size(px(12.0))
                            .text_color(rgb(theme::ACCENT))
                            .child(title),
                    )
                    .child(div().flex_1())
                    .children(self.error.clone().map(|error| {
                        div()
                            .text_size(px(10.0))
                            .text_color(rgb(0x00e3_5f4e))
                            .child(error)
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .text_size(px(10.0))
                            .text_color(rgb(status_color))
                            .child(div().size(px(7.0)).rounded_full().bg(rgb(status_color)))
                            .child(status),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .p(px(16.0))
                    .gap(px(16.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(self.console.transcript_viewport(now)),
                    )
                    .child(
                        self.console
                            .roster_panel(self.roster_input.clone(), now, window, cx),
                    ),
            )
    }
}

impl ConsoleHost<f64> for ScrLiveView {
    fn scr_console(&self) -> Option<&Console<f64>> {
        Some(&self.console)
    }

    fn scr_console_mut(&mut self) -> Option<&mut Console<f64>> {
        Some(&mut self.console)
    }

    fn scr_focus_roster(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.console.has_channel() {
            return;
        }
        if self.console.roster_focused() && self.roster_input.is_focused(window) {
            return;
        }
        self.console.set_roster_focused(true);
        self.console.clear_transcript_selection();
        self.roster_input.focus(window, cx);
        cx.notify();
    }

    fn scr_roster_pointer(&mut self, hovered: bool, window: &mut Window, cx: &mut Context<Self>) {
        if hovered && self.console.has_channel() {
            self.scr_focus_roster(window, cx);
        } else if !hovered && self.roster_input.is_focused(window) {
            self.console.set_roster_focused(false);
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn scr_clear_roster_filter(&mut self, cx: &mut Context<Self>) {
        self.roster_input.clear();
        self.console.set_filter(String::new(), js_sys::Date::now());
        cx.notify();
    }
}

fn scr_presence(value: &str) -> RosterPresence {
    match value {
        "away" => RosterPresence::Away,
        "busy" => RosterPresence::Busy,
        "in_game" => RosterPresence::InGame,
        "in_lobby" => RosterPresence::InLobby,
        // channel members are present; offline/unknown reads as online
        _ => RosterPresence::Online,
    }
}

/// A stable console id for a channel key: the numeric public id when the key
/// carries one, otherwise a hash of the key.
fn channel_id(key: &str) -> u32 {
    if let Some(rest) = key.strip_prefix("public:")
        && let Ok(id) = rest.parse::<u32>()
    {
        return id;
    }
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish() as u32
}

fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

fn stamp_label(timestamp: u64) -> String {
    let date = js_sys::Date::new(&JsValue::from_f64(timestamp as f64));
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    let suffix = if hours < 12 { "AM" } else { "PM" };
    let hour = match hours % 12 {
        0 => 12,
        hour => hour,
    };
    format!("{hour}:{minutes:02} {suffix}")
}
