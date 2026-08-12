use super::*;

impl SuperiorityView {
    pub(in crate::app::client) fn present_group_invitation(&mut self, club_id: u32) {
        let destination = self
            .join
            .groups
            .get(&club_id)
            .map(|group| group.name.clone());
        self.present_invitation(UiInvitation {
            id: self.join.next_invitation_id,
            kind: InvitationKind::Group { club_id },
            inviter: None,
            destination,
            closing: false,
        });
    }

    pub(in crate::app::client) fn present_party_invitation(
        &mut self,
        inviter: Option<String>,
        channel_index: u8,
    ) {
        self.present_invitation(UiInvitation {
            id: self.join.next_invitation_id,
            kind: InvitationKind::Party { channel_index },
            inviter,
            destination: Some("a party".to_owned()),
            closing: false,
        });
    }

    pub(in crate::app::client) fn present_invitation(&mut self, invitation: UiInvitation) {
        self.join.next_invitation_id = self.join.next_invitation_id.wrapping_add(1);
        while self.join.invitations.len() >= INVITATION_LIMIT {
            self.join.invitations.remove(0);
        }
        self.join.invitations.push(invitation);
    }

    pub(in crate::app::client) fn answer_invitation(
        &mut self,
        id: u64,
        accept: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(invitation) = self
            .join
            .invitations
            .iter()
            .find(|invitation| invitation.id == id)
            .cloned()
        else {
            return;
        };
        if let Some(commands) = &self.runtime.commands {
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
                    this.join
                        .invitations
                        .retain(|invitation| invitation.id != id);
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}
