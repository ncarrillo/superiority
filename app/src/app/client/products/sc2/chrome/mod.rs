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
    #[expect(
        dead_code,
        reason = "legacy button kept until the modern one is validated"
    )]
    pub(in crate::app::client) button_frames: Option<ButtonFrames>,
    pub(in crate::app::client) top_nav_background: Option<Arc<RenderImage>>,
    pub(in crate::app::client) ui_assets: Sc2Assets,
    /// modal textures are decoded on first use, which would otherwise happen
    /// while the first dialog is animating open.
    pub(in crate::app::client) modal_assets_warming: bool,
    pub(in crate::app::client) modal_warmup_started: bool,
    /// The shared modal shell's baked light, cached per size. Dialogs that
    /// have moved to the new frame draw from here; the nine-patch assets
    /// above stay until the last legacy dialog does.
    pub(in crate::app::client) modal_textures: ui_shared_modal::ModalTextures,
}
