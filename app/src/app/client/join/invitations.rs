use super::*;

impl JoinComponent {
    fn invitation_card(
        &self,
        invitation: &UiInvitation,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let id = invitation.id;
        let kicker = match invitation.kind {
            InvitationKind::Group { .. } => "Group invitation",
            InvitationKind::Party { .. } => "Party invitation",
        };
        let accept = chrome
            .action_button(
                format!("invitation-accept-{id}"),
                "ACCEPT",
                104.0,
                38.0,
                true,
            )
            .absolute()
            .left(px(284.0))
            .top(px(108.0))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.answer_invitation(id, true, window, cx);
            }));
        let decline = chrome
            .action_button(
                format!("invitation-decline-{id}"),
                "DECLINE",
                104.0,
                38.0,
                false,
            )
            .absolute()
            .left(px(174.0))
            .top(px(108.0))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.answer_invitation(id, false, window, cx);
            }));
        let card = div()
            .id(format!("invitation-card-{id}"))
            .relative()
            .w(px(INVITATION_WIDTH))
            .h(px(INVITATION_HEIGHT))
            .flex_shrink_0()
            .occlude()
            .font_family(FONT_INTERFACE)
            .child(
                img("images/toast/toast-background.png")
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .child(
                img("images/toast/toast-badge.png")
                    .absolute()
                    .left(px(24.0))
                    .top(px(31.0))
                    .w(px(56.0))
                    .h(px(58.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                div()
                    .absolute()
                    .left(px(98.0))
                    .top(px(24.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(12.5))
                    .text_color(rgb(0x42bff5))
                    .child(kicker),
            )
            .child(
                div()
                    .absolute()
                    .left(px(98.0))
                    .top(px(46.0))
                    .h(px(21.0))
                    .flex()
                    .items_center()
                    .text_size(px(13.0))
                    .text_color(rgb(0x7d8fa8))
                    .child(format!("{} invited you to", invitation.inviter_label())),
            )
            .child(
                div()
                    .absolute()
                    .left(px(98.0))
                    .top(px(69.0))
                    .right(px(24.0))
                    .h(px(27.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(16.0))
                    .text_color(rgb(0xd6e0f0))
                    .child(invitation.destination_label()),
            )
            .child(decline)
            .child(accept);

        if invitation.closing {
            card.with_animation(
                format!("invitation-close-{id}"),
                Animation::new(INVITATION_CLOSE_DURATION).with_easing(ease_in_out),
                |card, delta| {
                    card.left(px(INVITATION_TRAVEL * delta))
                        .opacity(1.0 - delta)
                },
            )
            .into_any_element()
        } else {
            card.with_animation(
                format!("invitation-open-{id}"),
                Animation::new(INVITATION_REVEAL_DURATION).with_easing(ease_in_out),
                |card, delta| {
                    card.left(px(INVITATION_TRAVEL * (1.0 - delta)))
                        .opacity(delta)
                },
            )
            .into_any_element()
        }
    }

    pub(in crate::app::client) fn invitation_stack(
        &self,
        chrome: &ChromeComponent,
        cx: &mut Context<SuperiorityView>,
    ) -> Option<Div> {
        if self.invitations.is_empty() {
            return None;
        }
        Some(
            div()
                .absolute()
                .right(px(24.0))
                .bottom(px(24.0))
                .w(px(INVITATION_WIDTH))
                .flex()
                .flex_col()
                .gap(px(INVITATION_GAP))
                .children(
                    self.invitations
                        .iter()
                        .map(|invitation| self.invitation_card(invitation, chrome, cx)),
                ),
        )
    }
}
