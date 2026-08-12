use super::*;

impl SettingsComponent {
    pub(in crate::app::client) fn set_tooltip(&mut self, index: usize, hovered: bool) {
        if hovered {
            self.settings_tooltip = Some(index);
        } else if self.settings_tooltip == Some(index) {
            self.settings_tooltip = None;
        }
    }

    pub(super) fn tooltip_card(&self, assets: &UiAssets) -> Option<AnyElement> {
        let index = self.settings_tooltip?;
        let (title, detail, anchor_x, anchor_top, anchor_width, anchor_height, checkbox) =
            match index {
                SETTINGS_TOOLTIP_TIMESTAMPS => (
                    "Show Timestamps:",
                    "Displays the local receive time beside every transcript entry.",
                    270.0,
                    173.0,
                    30.0,
                    30.0,
                    true,
                ),
                SETTINGS_TOOLTIP_MEMBERSHIP => (
                    "Join / Leave Notifications:",
                    "Shows when members enter or leave the active channel.",
                    270.0,
                    221.0,
                    30.0,
                    30.0,
                    true,
                ),
                SETTINGS_TOOLTIP_LIVE_ENABLED => (
                    "Enable Live:",
                    "Streams your open channels to your live link. Nothing is sent while this is off.",
                    270.0,
                    226.0,
                    30.0,
                    30.0,
                    true,
                ),
                index if index >= SETTINGS_TOOLTIP_BACKGROUND_START => {
                    let background = index - SETTINGS_TOOLTIP_BACKGROUND_START;
                    let title = BACKGROUNDS.get(background)?.title;
                    let column = (background % 4) as f32;
                    let row = background / 4;
                    (
                        title,
                        "Use this scene beneath chat messages.",
                        248.0 + column * 166.0,
                        if row == 0 { 236.0 } else { 378.0 },
                        156.0,
                        132.0,
                        false,
                    )
                }
                _ => return None,
            };
        let mut left = if checkbox {
            944.0 - 380.0 - 22.0
        } else {
            anchor_x + anchor_width + 10.0
        };
        if !checkbox && left + 380.0 > 944.0 - 14.0 {
            left = anchor_x - 380.0 - 10.0;
        }
        left = left.clamp(14.0, 944.0 - 380.0 - 14.0);
        let top = (anchor_top + anchor_height / 2.0_f32 - 47.0).clamp(22.0, 504.0);
        let from_x = if left < anchor_x { 8.0 } else { -8.0 };
        let animation_id = match index {
            SETTINGS_TOOLTIP_TIMESTAMPS => "settings-tip-timestamps",
            SETTINGS_TOOLTIP_MEMBERSHIP => "settings-tip-membership",
            SETTINGS_TOOLTIP_LIVE_ENABLED => "settings-tip-live",
            10 => "settings-tip-background-0",
            11 => "settings-tip-background-1",
            12 => "settings-tip-background-2",
            13 => "settings-tip-background-3",
            14 => "settings-tip-background-4",
            15 => "settings-tip-background-5",
            16 => "settings-tip-background-6",
            _ => "settings-tip-background-7",
        };
        let tooltip = ui_controls::tooltip_shell(380.0, 94.0, assets.tooltip_fill.clone())
            .absolute()
            .font_family(FONT_INTERNATIONAL)
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .top(px(15.0))
                    .w(px(344.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .text_color(rgb(0xd6e0f0))
                    .child(title),
            )
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .top(px(44.0))
                    .w(px(344.0))
                    .h(px(36.0))
                    .text_size(px(12.0))
                    .line_height(px(15.0))
                    .text_color(rgb(0x85d1ff))
                    .child(detail),
            );
        Some(ui_controls::animated_tooltip(
            tooltip,
            animation_id,
            left,
            top,
            from_x,
        ))
    }
}
