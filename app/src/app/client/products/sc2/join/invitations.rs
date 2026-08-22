use super::*;

impl JoinComponent {
    fn invitation_card(
        &self,
        invitation: &UiInvitation,
        cx: &mut Context<SuperiorityView>,
    ) -> AnyElement {
        let id = invitation.id;
        let (kicker, preposition) = match invitation.kind {
            InvitationKind::Group { .. } => ("GROUP INVITATION", "invited you to join"),
            InvitationKind::Party { .. } => ("PARTY INVITATION", "invited you to"),
        };
        let accept = ui_buttons::button(
            ("invitation-accept", u64::from(id)),
            ui_buttons::ModalVariant::Sc2,
            ui_buttons::ButtonWeight::Primary,
            ui_buttons::ButtonTone::Chrome,
            ui_buttons::ButtonLife::Ready,
            "ACCEPT",
        )
        .w(px(104.0))
        .h(px(38.0))
        .on_click(cx.listener(move |this, _, window, cx| {
            this.answer_invitation(id, true, window, cx);
        }));
        let decline = ui_buttons::button(
            ("invitation-decline", u64::from(id)),
            ui_buttons::ModalVariant::Sc2,
            ui_buttons::ButtonWeight::Ghost,
            ui_buttons::ButtonTone::Chrome,
            ui_buttons::ButtonLife::Ready,
            "DECLINE",
        )
        .w(px(104.0))
        .h(px(38.0))
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
                    .left(px(18.0))
                    .top(px(27.0))
                    .w(px(48.0))
                    .h(px(50.0))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                div()
                    .absolute()
                    .left(px(80.0))
                    .top(px(24.0))
                    .w(px(90.0))
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(
                        div()
                            .h(px(12.0))
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(9.0))
                            .text_color(rgb(0x4bb8e8))
                            .child(kicker),
                    )
                    .child(
                        div()
                            .h(px(18.0))
                            .flex()
                            .items_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(14.0))
                            .text_color(rgb(0xdce7f5))
                            .child(invitation.inviter_label().to_owned()),
                    )
                    .child(
                        div()
                            .h(px(13.0))
                            .flex()
                            .items_center()
                            .text_size(px(10.0))
                            .text_color(rgb(0x8397b0))
                            .child(format!("{preposition} {}", invitation.destination_label())),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .right(px(18.0))
                    .top(px(33.0))
                    .h(px(38.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(decline)
                    .child(accept),
            );

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
        cx: &mut Context<SuperiorityView>,
    ) -> Option<Div> {
        // a party invitation arrives in the transcript, not as a card over it,
        // so the stack is left holding the group invitations only
        let carded = self
            .invitations
            .iter()
            .filter(|invitation| matches!(invitation.kind, InvitationKind::Group { .. }))
            .collect::<Vec<_>>();
        if carded.is_empty() {
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
                    carded
                        .into_iter()
                        .map(|invitation| self.invitation_card(invitation, cx)),
                ),
        )
    }
}
