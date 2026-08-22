use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn present_group_invitation(&mut self, club_id: u32) {
        let destination = self
            .session
            .join
            .groups
            .get(&club_id)
            .map(|group| group.name.clone());
        self.present_invitation(UiInvitation {
            id: self.session.join.next_invitation_id,
            kind: InvitationKind::Group { club_id },
            inviter: None,
            destination,
            closing: false,
        });
    }

    /// a party invitation lands in the transcript, in front of whatever you are
    /// reading. it is not a modal: declining it notifies nobody, and ignoring
    /// it costs nothing but a line.
    pub(in crate::app::client) fn present_party_invitation(
        &mut self,
        inviter: Option<String>,
        channel_index: u8,
    ) {
        let id = self.session.join.next_invitation_id;
        self.session.join.next_invitation_id = self.session.join.next_invitation_id.wrapping_add(1);
        // the record is what answering needs; the transcript row is only its
        // face. unanswered invitations are bounded so a spammer cannot grow
        // this without end.
        while self.session.join.invitations.len() >= INVITATION_RECORDS {
            self.session.join.invitations.remove(0);
        }
        self.session.join.invitations.push(UiInvitation {
            id,
            kind: InvitationKind::Party { channel_index },
            inviter: inviter.clone(),
            destination: Some("a party".to_owned()),
            closing: false,
        });
        let line = ChatLine::Invitation {
            time: Self::current_timestamp(),
            id,
            inviter: inviter.unwrap_or_else(|| "A player".to_owned()),
            detail: "invites you to a party".to_owned(),
            answered: None,
        };
        if self.session.channels.active_tab < self.session.channels.tabs.len() {
            self.append_chat_line(self.session.channels.active_tab, line);
        }
    }

    pub(in crate::app::client) fn present_invitation(&mut self, invitation: UiInvitation) {
        self.session.join.next_invitation_id = self.session.join.next_invitation_id.wrapping_add(1);
        while self.session.join.invitations.len() >= INVITATION_LIMIT {
            self.session.join.invitations.remove(0);
        }
        self.session.join.invitations.push(invitation);
    }

    pub(in crate::app::client) fn answer_invitation(
        &mut self,
        id: u64,
        accept: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        // the line stays where it is and settles: a row that vanished would
        // leave a hole where the thing you answered used to be
        for tab in &mut self.session.channels.tabs {
            for line in &mut tab.transcript {
                if let ChatLine::Invitation {
                    id: line_id,
                    answered,
                    ..
                } = line
                    && *line_id == id
                {
                    *answered = Some(accept);
                }
            }
        }
        let Some(invitation) = self
            .session
            .join
            .invitations
            .iter()
            .find(|invitation| invitation.id == id)
            .cloned()
        else {
            return;
        };
        if let Some(commands) = &self.session.commands {
            let command = match invitation.kind {
                InvitationKind::Group { club_id } => {
                    ClientCommand::AnswerGroupInvitation { club_id, accept }
                }
                InvitationKind::Party { channel_index } => ClientCommand::AnswerPartyInvitation {
                    channel_index,
                    accept,
                },
            };
            let _ = commands.send(command);
        }
        self.begin_invitation_close(id, window, cx);
    }

    pub(in crate::app::client) fn begin_invitation_close(
        &mut self,
        id: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(invitation) = self
            .session
            .join
            .invitations
            .iter_mut()
            .find(|invitation| invitation.id == id)
        else {
            return;
        };
        if invitation.closing {
            return;
        }
        invitation.closing = true;
        cx.notify();
        let executor = cx.background_executor().clone();
        cx.spawn_in(window, async move |entity, cx| {
            executor.timer(INVITATION_CLOSE_DURATION).await;
            entity
                .update_in(cx, |this, _, cx| {
                    this.session
                        .join
                        .invitations
                        .retain(|invitation| invitation.id != id);
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}
