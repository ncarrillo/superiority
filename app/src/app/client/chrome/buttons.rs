use super::super::*;

pub(in crate::app::client) const BUTTON_ART_VERTICAL_BLEED: f32 = 6.0;

impl ChromeComponent {
    pub(in crate::app::client) fn action_button(
        &self,
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        width: f32,
        height: f32,
        warning: bool,
    ) -> Stateful<Div> {
        let id = id.into();
        let idle_image: ImageSource = self
            .button_frames
            .as_ref()
            .and_then(|frames| frames.image(width, height, warning, false))
            .map_or_else(
                || {
                    if warning {
                        self.ui_assets.warning_button_idle.clone()
                    } else {
                        self.ui_assets.button_idle.clone()
                    }
                },
                Into::into,
            );
        let active_image: ImageSource = self
            .button_frames
            .as_ref()
            .and_then(|frames| frames.image(width, height, warning, true))
            .map_or_else(
                || {
                    if warning {
                        self.ui_assets.warning_button_active.clone()
                    } else {
                        self.ui_assets.button_active.clone()
                    }
                },
                Into::into,
            );
        ui_controls::action_button(
            id,
            title,
            width,
            height,
            warning,
            ui_controls::ActionButtonImages {
                idle: idle_image,
                active: active_image,
            },
        )
    }
}
