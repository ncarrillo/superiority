use super::*;

mod actions;
mod command;
mod people;
mod popup;

pub(in crate::app::client) use command::{
    COMMAND_RESULTS, ChatCommand, CommandAction, CommandKind, CommandResults, CommandRow,
    CommandTint, accents, command_prefix, command_word, match_span, parse_command,
    unknown_command_notice,
};
pub(in crate::app::client) use people::{
    PersonRow, WhisperPeer, mention_candidates, split_whisper, whisper_candidates,
};

pub(in crate::app::client) const COMMAND_OPEN_DURATION: Duration = Duration::from_millis(140);
pub(in crate::app::client) const COMMAND_CLOSE_DURATION: Duration = Duration::from_millis(120);
/// the fade only starts on the frame after it is asked for, so the popup is
/// kept a couple of frames past its own duration.
pub(in crate::app::client) const COMMAND_CLOSE_SLACK: Duration = Duration::from_millis(40);

pub(super) struct ComposerComponent {
    pub(super) composer_focused: bool,
    pub(super) composer: ui_text_input::TextInput,
    /// the line the command state was last derived from. the field is polled
    /// rather than subscribed to, so this is what tells a keystroke apart from
    /// a repaint.
    pub(super) command_line: String,
    pub(super) command_selected: usize,
    /// set by escape and cleared by the next keystroke: the popup stays shut
    /// for the line you dismissed it on, not for the command itself.
    pub(super) command_dismissed: bool,
    /// what a closing popup is still drawing. the list outlives the text by one
    /// animation, so dismissing it fades out instead of blinking away — and it
    /// fades out holding the rows it had, not rows recomputed against a channel
    /// list the join has already changed.
    pub(super) command_closing: Option<CommandResults>,
    /// guards the close timer, so a popup reopened mid-fade is not torn down by
    /// the fade it interrupted.
    pub(super) command_close_epoch: u64,
    /// bumped every time the popup opens, so the rise replays for each command
    /// rather than only the first.
    pub(super) command_epoch: usize,
    /// whether the field held the keyboard last frame. focus is owned by the
    /// window, not by us, so the edges have to be noticed rather than hooked.
    pub(super) command_focused: bool,
    /// where the caret was when the line was last read. a mention is the token
    /// under it, which is not always the last one on the line.
    pub(super) command_cursor: usize,
}

impl ComposerComponent {
    pub(super) fn view(
        &self,
        window: &Window,
        has_channel: bool,
        online_friends: usize,
        results: Option<CommandResults>,
        assets: &UiAssets,
        cx: &mut Context<SuperiorityView>,
    ) -> Div {
        let focused = self.composer_focused && self.composer.is_focused(window);
        // the field only ever addresses the channel — a whisper is a
        // conversation in the social panel, not a mode this field wears
        self.composer.set_placeholder(if has_channel {
            "Press Enter to chat"
        } else {
            "Use + to join a channel"
        });
        self.composer.set_accents(accents(&self.command_line));
        // a closing popup draws the results it was closed on, which the field
        // itself no longer holds
        let live = focused.then_some(results).flatten();
        let popup = match (live.as_ref(), self.command_closing.as_ref()) {
            (Some(results), _) => Some(self.command_popup(results, false, assets, cx)),
            (None, Some(results)) => Some(self.command_popup(results, true, assets, cx)),
            (None, None) => None,
        };
        let text_color = if self.composer.is_empty() {
            rgb(0x5e8291)
        } else {
            rgb(0xd6e0f0)
        };
        let border = if focused {
            rgb(BORDER_FOCUSED)
        } else {
            rgba(BORDER_STRUCTURAL)
        };

        div()
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
                    .bg(rgb(0x080d13))
                    .border_1()
                    .border_color(border)
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
                            if this.channels.active().is_some() {
                                this.overlays.active = None;
                                this.chat.transcript.selection.clear();
                                this.composer.composer_focused = true;
                                this.roster.roster.focused = false;
                                this.composer.composer.focus(window, cx);
                                cx.notify();
                            }
                        }),
                    )
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
                                this.composer.composer_focused = false;
                                this.roster.roster.focused = false;
                                cx.stop_propagation();
                                if this.overlays.active == Some(Overlay::Friends)
                                    && !this.overlays.closing
                                {
                                    this.dismiss_overlay(window, cx);
                                } else {
                                    this.social.social_detail_open = false;
                                    this.social.social_pane_transition = None;
                                    this.social.conversation_peer = None;
                                    this.social.conversation_input.clear();
                                    this.social.conversation_focused = false;
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
            )
    }
}
