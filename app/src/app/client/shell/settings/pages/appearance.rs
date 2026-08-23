use super::*;

const BACKGROUND_COLUMNS: usize = 4;
const BACKGROUND_TILE_WIDTH: f32 = 156.0;
const BACKGROUND_TILE_HEIGHT: f32 = 132.0;
const BACKGROUND_COLUMN_GAP: f32 = 10.0;
const BACKGROUND_ROW_GAP: f32 = 10.0;
const BACKGROUND_VIEWPORT_WIDTH: f32 = 666.0;
const BACKGROUND_VIEWPORT_HEIGHT: f32 = 286.0;

impl SettingsComponent {
    pub(in crate::app::client) fn appearance_settings_page(
        &self,
        mut page: Stateful<Div>,
        product: Product,
        variant: ui_shared_modal::ModalVariant,
        window: &mut Window,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let skin = SettingsSkin::for_variant(variant);
        page = page
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(100.0))
                    .w(px(400.0))
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .font_family(skin.heading_font)
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(20.0))
                    .child("Appearance"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(160.0))
                    .w(px(260.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(12.5))
                    .text_color(rgb(skin.label))
                    .child("Chat background"),
            )
            .child(
                div()
                    .absolute()
                    .left(px(22.0))
                    .top(px(186.0))
                    .w(px(540.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .text_size(px(11.2))
                    .text_color(rgb(skin.muted))
                    .child("This choice is saved separately for each game."),
            );
        let row_count = BACKGROUNDS.len().div_ceil(BACKGROUND_COLUMNS);
        let grid_height = row_count as f32 * BACKGROUND_TILE_HEIGHT
            + row_count.saturating_sub(1) as f32 * BACKGROUND_ROW_GAP;
        let mut background_grid = div()
            .id("settings-background-grid")
            .relative()
            .w(px(BACKGROUND_VIEWPORT_WIDTH - 12.0))
            .h(px(grid_height))
            .flex_shrink_0();
        for (index, background) in BACKGROUNDS.into_iter().enumerate() {
            let title = background.title;
            let path = background.path;
            let column = (index % BACKGROUND_COLUMNS) as f32;
            let row = (index / BACKGROUND_COLUMNS) as f32;
            let selected = self.background(product) == path;
            background_grid = background_grid.child(
                div()
                    .id(("settings-background", index))
                    .absolute()
                    .left(px(column * (BACKGROUND_TILE_WIDTH + BACKGROUND_COLUMN_GAP)))
                    .top(px(row * (BACKGROUND_TILE_HEIGHT + BACKGROUND_ROW_GAP)))
                    .w(px(BACKGROUND_TILE_WIDTH))
                    .h(px(BACKGROUND_TILE_HEIGHT))
                    .bg(rgb(if selected {
                        skin.tile_selected_fill
                    } else {
                        skin.tile_fill
                    }))
                    .border_1()
                    .border_color(if selected {
                        rgb(skin.tile_selected_edge)
                    } else {
                        rgba(skin.structural_edge)
                    })
                    .rounded(px(2.0))
                    .cursor_pointer()
                    // selection reads as the bright stroke plus its glow — the
                    // stroke stays 1px in every state so nothing shifts.
                    .when(selected, |tile| tile.shadow(skin.selection_glow()))
                    .hover(move |style| style.border_color(rgb(skin.tile_hover_edge)))
                    .active(|style| style.opacity(0.82))
                    .on_hover(cx.listener(move |this, hovered, _, cx| {
                        this.settings
                            .set_tooltip(SETTINGS_TOOLTIP_BACKGROUND_START + index, *hovered);
                        cx.notify();
                    }))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.settings.set_background(product, path);
                        preferences::save_background(product, index);
                        cx.notify();
                    }))
                    .child(
                        img(path)
                            .absolute()
                            .left(px(4.0))
                            .top(px(4.0))
                            .w(px(BACKGROUND_TILE_WIDTH - 8.0))
                            .h(px(100.0))
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(6.0))
                            .bottom(px(7.0))
                            .w(px(BACKGROUND_TILE_WIDTH - 12.0))
                            .h(px(16.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(10.2))
                            .child(title),
                    ),
            );
        }
        let background_list = div()
            .id("settings-background-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.background_scroll)
            .child(background_grid);
        let background_viewport = div()
            .id("settings-background-viewport")
            .absolute()
            .left(px(14.0))
            .top(px(218.0))
            .w(px(BACKGROUND_VIEWPORT_WIDTH))
            .h(px(BACKGROUND_VIEWPORT_HEIGHT))
            .overflow_hidden()
            .child(background_list)
            .vertical_scrollbar_in(&self.background_scroll, variant.scrollbar(), window, cx);
        page.child(background_viewport)
    }
}
