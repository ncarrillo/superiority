use super::*;

mod actions;
mod command;
mod party;
mod people;
mod popup;

pub(in crate::app::client) use command::{
    COMMAND_RESULTS, ChatCommand, CommandAction, CommandKind, CommandResults, CommandRow,
    CommandTint, accents, command_prefix, command_word, match_span, no_party_notice, parse_command,
    unknown_command_notice,
};
use party::{PARTY_DOCK_GAP, PARTY_DOCK_HEIGHT, party_dock};
pub(in crate::app::client) use people::{
    PersonRow, WhisperPeer, mention_candidates, split_whisper, whisper_candidates,
};

/// the field in party scope: a green rule, a green-black fill, and a bloom the
/// same colour, so a line about to go to the party never looks like a line
/// about to go to the channel.
const PARTY_BORDER: u32 = 0x47d1_8499;
const PARTY_FILL: u32 = 0x0002_0a06;
const PARTY_GLOW: u32 = 0x47d1_8433;
const PARTY_TEXT: u32 = 0x0047_d184;

pub(in crate::app::client) const COMMAND_OPEN_DURATION: Duration = Duration::from_millis(140);
pub(in crate::app::client) const COMMAND_CLOSE_DURATION: Duration = Duration::from_millis(120);
/// the fade only starts on the frame after it is asked for, so the popup is
/// kept a couple of frames past its own duration.
pub(in crate::app::client) const COMMAND_CLOSE_SLACK: Duration = Duration::from_millis(40);

pub(in crate::app::client) struct ComposerComponent {
    pub(in crate::app::client) composer_focused: bool,
    pub(in crate::app::client) composer: ui_text_input::TextInput,
    /// the line the command state was last derived from. the field is polled
    /// rather than subscribed to, so this is what tells a keystroke apart from
    /// a repaint.
    pub(in crate::app::client) command_line: String,
    pub(in crate::app::client) command_selected: usize,
    /// set by escape and cleared by the next keystroke: the popup stays shut
    /// for the line you dismissed it on, not for the command itself.
    pub(in crate::app::client) command_dismissed: bool,
    /// what a closing popup is still drawing. the list outlives the text by one
    /// animation, so dismissing it fades out instead of blinking away — and it
    /// fades out holding the rows it had, not rows recomputed against a channel
    /// list the join has already changed.
    pub(in crate::app::client) command_closing: Option<CommandResults>,
    /// guards the close timer, so a popup reopened mid-fade is not torn down by
    /// the fade it interrupted.
    pub(in crate::app::client) command_close_epoch: u64,
    /// bumped every time the popup opens, so the rise replays for each command
    /// rather than only the first.
    pub(in crate::app::client) command_epoch: usize,
    /// whether the field held the keyboard last frame. focus is owned by the
    /// window, not by us, so the edges have to be noticed rather than hooked.
    pub(in crate::app::client) command_focused: bool,
    /// where the caret was when the line was last read. a mention is the token
    /// under it, which is not always the last one on the line.
    pub(in crate::app::client) command_cursor: usize,
    /// set while the field is addressed to the party rather than the channel.
    /// a party has no window of its own — the talk stays inline in whatever
    /// transcript you are reading — so the scope has to live on the field.
    pub(in crate::app::client) party_scope: bool,
}

impl ComposerComponent {
    pub(in crate::app::client) fn view(
        &self,
        window: &Window,
        has_channel: bool,
        online_friends: usize,
        results: Option<CommandResults>,
        party: Option<(&[UiUser], Option<u32>)>,
        assets: &Sc2Assets,
        cx: &mut Context<SuperiorityView>,
    ) -> Div {
        let focused = self.composer_focused && self.composer.is_focused(window);
        // a whisper is a conversation in the social panel, not a mode this
        // field wears; the party has no panel of its own, so it is the one
        // scope the field does carry
        self.composer
            .set_placeholder(match (self.party_scope, has_channel) {
                (true, _) => "Message your party",
                (false, true) => "Press Enter to chat",
                (false, false) => "Use + to join a channel",
            });
        self.composer.set_accents(accents(&self.command_line));
        // a closing popup draws the results it was closed on, which the field
        // itself no longer holds
        let live = focused.then_some(results).flatten();
        // the popup clears the dock rather than covering it: the party's state
        // is not something to hide while you look for a channel
        let clearance = party.map_or(0.0, |_| PARTY_DOCK_HEIGHT + PARTY_DOCK_GAP);
        let popup = match (live.as_ref(), self.command_closing.as_ref()) {
            (Some(results), _) => Some(self.command_popup(results, false, clearance, assets, cx)),
            (None, Some(results)) => Some(self.command_popup(results, true, clearance, assets, cx)),
            (None, None) => None,
        };
        let text_color = if self.composer.is_empty() {
            rgb(0x5e8291)
        } else {
            rgb(0xd6e0f0)
        };
        self.composer
            .set_ink(ui_inputs::field_ink(ui_inputs::ModalVariant::Sc2));

        let field = div()
            .relative()
            .flex()
            .gap(px(8.0))
            .h(px(COMPOSER_HEIGHT))
            .flex_shrink_0()
            .children(popup)
            .child(
                div()
                    .id("composer")
                    .flex()
                    .items_center()
                    .flex_1()
                    .px(px(12.0))
                    // the modern field is the base; the party scope's own
                    // dress wins over it — the border and the token together
                    // are what make the scope unmissable before you press
                    // enter
                    .map(|field| {
                        if self.party_scope {
                            field
                                .bg(rgb(PARTY_FILL))
                                .border_1()
                                .border_color(rgba(PARTY_BORDER))
                                .shadow(scope_glow(PARTY_GLOW))
                        } else {
                            ui_inputs::dressed(field, ui_inputs::ModalVariant::Sc2, focused, false)
                        }
                    })
                    .rounded(px(2.0))
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(13.5))
                    .text_color(text_color)
                    .cursor(gpui::CursorStyle::IBeam)
                    .gap(px(8.0))
                    .on_hover(cx.listener(|this, hovered, window, cx| {
                        this.set_composer_pointer_focus(*hovered, window, cx);
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            if this.session.channels.active().is_some() {
                                this.overlays.active = None;
                                this.session.chat.transcript.selection.clear();
                                this.session.composer.composer_focused = true;
                                this.session.roster.roster.focused = false;
                                this.session.composer.composer.focus(window, cx);
                                cx.notify();
                            }
                        }),
                    )
                    .when(self.party_scope, |field| field.child(party_token(cx)))
                    .child(self.composer.element()),
            )
            .child(
                div()
                    .relative()
                    .w(px(56.0))
                    .h_full()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("friends")
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(0x101a2a))
                            .border_1()
                            .border_color(rgb(0x2c425d))
                            .rounded(px(2.0))
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x16273e)).border_color(rgb(0x3e6e9e)))
                            .active(|style| style.bg(rgb(0x1d3a5c)).border_color(rgb(0x4e8fc8)))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.session.composer.composer_focused = false;
                                this.session.roster.roster.focused = false;
                                cx.stop_propagation();
                                if this.overlays.active == Some(Overlay::Friends)
                                    && !this.overlays.closing
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
                            }))
                            .child(
                                img("images/icons/friends.png")
                                    .size(px(28.0))
                                    .object_fit(ObjectFit::Contain),
                            ),
                    )
                    .child(
                        div()
                            .absolute()
                            .right(px(-5.0))
                            .top(px(-8.0))
                            .min_w(px(19.0))
                            .h(px(18.0))
                            .px(px(4.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(0x4579b7))
                            .border_1()
                            .border_color(rgb(0x78b8ec))
                            .rounded(px(2.0))
                            .text_size(px(10.5))
                            .text_color(rgb(0xe6f9ff))
                            .child(online_friends.to_string()),
                    ),
            );
        div()
            .flex()
            .flex_col()
            .gap(px(PARTY_DOCK_GAP))
            .flex_shrink_0()
            .children(party.map(|(members, local)| party_dock(members, local, assets, cx)))
            .child(field)
    }
}

/// the token that says where the next line is going. it is the same object the
/// scope is — clicking it leaves, the way escape and a second `/p` do.
fn party_token(cx: &mut Context<SuperiorityView>) -> Stateful<Div> {
    div()
        .id("composer-party-token")
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(5.0))
        .py(px(2.0))
        .bg(rgb(PARTY_TEXT))
        .rounded(px(2.0))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x006b_e8a4)))
        .child(
            div()
                .font_family(FONT_NAVIGATION)
                .font_weight(FontWeight::BOLD)
                .text_size(px(9.0))
                .text_color(rgb(0x000a_1f12))
                .child("PARTY"),
        )
        .child(
            div()
                .font_family(FONT_INTERFACE)
                .text_size(px(10.0))
                .text_color(rgba(0x0a1f_12b3))
                .child("\u{00d7}"),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.leave_party_scope(cx);
        }))
}
