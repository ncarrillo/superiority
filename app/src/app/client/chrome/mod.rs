use super::*;

mod assets;
mod buttons;
mod modal;

#[cfg(test)]
pub(in crate::app::client) use assets::resize_button_frame;
pub(in crate::app) use assets::{Assets, load_fonts};
pub(in crate::app::client) use assets::{
    ButtonFrames, ModalFrame, PortraitRegistry, load_top_nav_background,
};
pub(in crate::app::client) use buttons::BUTTON_ART_VERTICAL_BLEED;

pub(in crate::app::client) struct ChromeComponent {
    pub(in crate::app::client) modal_frame: Option<ModalFrame>,
    pub(in crate::app::client) button_frames: Option<ButtonFrames>,
    pub(in crate::app::client) top_nav_background: Option<Arc<RenderImage>>,
    pub(in crate::app::client) ui_assets: UiAssets,
}
