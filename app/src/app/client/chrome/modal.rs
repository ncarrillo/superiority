use super::super::*;

impl ChromeComponent {
    pub(in crate::app::client) fn modal_chrome(&self, width: f32, height: f32) -> Div {
        let frame = self
            .modal_frame
            .as_ref()
            .map(|frame| ImageSource::from(frame.image(width, height)));
        ui_modal::chrome(width, height, frame, &self.ui_assets)
    }

    pub(in crate::app::client) fn modal_header(
        &self,
        layout: ui_modal::HeaderLayout,
        title: &'static str,
    ) -> Div {
        ui_modal::header(layout, title, &self.ui_assets)
    }

    pub(in crate::app::client) fn join_asset_warmup(&self) -> Div {
        let mut warmup = div()
            .absolute()
            .top_0()
            .left_0()
            .size(px(1.0))
            .overflow_hidden()
            .opacity(0.0);

        if let Some(frame) = &self.modal_frame {
            warmup = warmup.child(
                img(frame.image(640.0, 580.0))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size(px(1.0))
                    .object_fit(ObjectFit::Fill),
            );
        }

        for (index, path) in [
            "images/dialogs/modal-title-band.png",
            "images/dialogs/modal-hex.png",
            "images/dialogs/modal-glow-left.png",
            "images/dialogs/modal-glow-right.png",
            "images/dialogs/modal-glow-top.png",
            "images/dialogs/modal-glow-bottom.png",
            "images/nine-patch/controls/button-idle.png",
            "images/nine-patch/controls/warning-button-idle.png",
            "images/icons/channel.png",
        ]
        .into_iter()
        .enumerate()
        {
            warmup = warmup.child(
                img(path)
                    .absolute()
                    .top_0()
                    .left(px(index as f32))
                    .size(px(1.0))
                    .object_fit(ObjectFit::Fill),
            );
        }

        warmup
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(640.0))
                    .font_family(FONT_NAVIGATION)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(20.0))
                    .child("JOIN A CHANNEL  CANCEL  JOIN"),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .w(px(640.0))
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(14.0))
                    .child("Search, or type a channel name  No channels match."),
            )
    }
}
