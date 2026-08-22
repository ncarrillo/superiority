use super::*;

impl UpdateComponent {
    pub(in crate::app::client) fn overlay(
        &self,
        variant: ui_shared_modal::ModalVariant,
        chrome: &ChromeComponent,
        window: &mut Window,
        cx: &mut Context<SuperiorityView>,
    ) -> Stateful<Div> {
        let (status, progress, primary_title, secondary_title, primary_enabled) =
            self.update_model.status();
        let status = match self.update_model.stage {
            UpdateStage::Downloading(progress) => {
                format!("Downloading update… {:.0}%", progress * 100.0)
            }
            UpdateStage::Extracting(progress) => {
                format!("Verifying and extracting… {:.0}%", progress * 100.0)
            }
            _ => status.to_owned(),
        };
        let status_color = match self.update_model.stage {
            UpdateStage::Ready | UpdateStage::Installing | UpdateStage::Current => rgb(0x47d185),
            UpdateStage::Error => rgb(0xf2705c),
            _ => rgb(0x33a8f0),
        };
        let mut primary = ui_buttons::button(
            "update-primary",
            variant,
            ui_buttons::ButtonWeight::Primary,
            ui_buttons::ButtonTone::Chrome,
            if primary_enabled {
                ui_buttons::ButtonLife::Ready
            } else {
                ui_buttons::ButtonLife::Disabled
            },
            ui_buttons::worded(variant, primary_title),
        )
        .w(px(148.0))
        .h(px(42.0))
        .absolute()
        .left(px(594.0))
        .top(px(523.0));
        if primary_enabled {
            primary = primary.on_click(cx.listener(|this, _, _, cx| {
                this.perform_update_primary_action(cx);
            }));
        }
        let panel = div()
            .id("update-modal")
            .relative()
            .w(px(780.0))
            .h(px(590.0))
            .font_family(FONT_INTERFACE)
            .text_color(rgb(0xd6e0f0))
            .on_click(|_, _, cx| cx.stop_propagation())
            // the shared modal shell, dressed as whichever realm it is opened
            // over
            .child(ui_shared_modal::frame(
                variant,
                780.0,
                590.0,
                &chrome.modal_textures,
            ))
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .right(px(18.0))
                    .top(px(32.0))
                    .child(ui_shared_modal::title(variant, "SOFTWARE UPDATE")),
            )
            .child(
                div()
                    .absolute()
                    .left(px(38.0))
                    .top(px(87.0))
                    .w(px(704.0))
                    .h(px(30.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(19.0))
                    .child(self.update_model.headline.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left(px(38.0))
                    .top(px(122.0))
                    .w(px(704.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(rgb(0x7d8fa8))
                    .child(self.update_model.summary.clone()),
            )
            .child(
                div()
                    .absolute()
                    .left(px(38.0))
                    .top(px(166.0))
                    .w(px(704.0))
                    .h(px(258.0))
                    .bg(rgb(0x060a0f))
                    .border_1()
                    .border_color(rgba(BORDER_STRUCTURAL))
                    .rounded(px(1.0)),
            )
            .child(
                div()
                    .id("update-notes-viewport")
                    .absolute()
                    .left(px(41.0))
                    .top(px(169.0))
                    .w(px(698.0))
                    .h(px(252.0))
                    .child(
                        div()
                            .id("update-notes-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.update_notes_scroll)
                            .w_full()
                            .min_h(px(252.0))
                            .px(px(22.0))
                            .py(px(20.0))
                            .child(ui_release_notes::view(
                                &self.update_model.notes,
                                &self.update_notes_selection,
                            )),
                    )
                    .vertical_scrollbar_in(
                        &self.update_notes_scroll,
                        variant.scrollbar(),
                        window,
                        cx,
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px(38.0))
                    .top(px(439.0))
                    .w(px(704.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(12.0))
                    .text_color(status_color)
                    .child(status),
            )
            .child(
                div()
                    .absolute()
                    .left(px(38.0))
                    .top(px(472.0))
                    .w(px(704.0))
                    .h(px(4.0))
                    .bg(rgb(0x091016)),
            )
            .child(
                div()
                    .absolute()
                    .left(px(38.0))
                    .top(px(472.0))
                    .w(px(704.0 * progress.clamp(0.0, 1.0)))
                    .h(px(4.0))
                    .bg(status_color),
            )
            .child(
                div()
                    .absolute()
                    .left(px(38.0))
                    .top(px(493.0))
                    .w(px(420.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .text_size(px(10.5))
                    .text_color(rgb(0x7d8fa8))
                    .child("The signed update is verified before installation."),
            )
            .child(
                ui_buttons::button(
                    "update-secondary",
                    variant,
                    ui_buttons::ButtonWeight::Ghost,
                    ui_buttons::ButtonTone::Chrome,
                    ui_buttons::ButtonLife::Ready,
                    ui_buttons::worded(variant, secondary_title),
                )
                .w(px(136.0))
                .h(px(42.0))
                .absolute()
                .left(px(448.0))
                .top(px(523.0))
                .on_click(cx.listener(|this, _, _, cx| this.close_update_dialog(cx))),
            )
            .child(primary)
            .child(
                ui_shared_modal::close_glyph(variant)
                    .right(px(26.0))
                    .top(px(32.0))
                    .id("update-close")
                    .on_click(cx.listener(|this, _, _, cx| this.close_update_dialog(cx))),
            );
        let dimmer = div().absolute().inset_0().bg(rgba(0x000305b3));
        let dimmer = if self.update_dialog_closing {
            dimmer
                .with_animation(
                    "update-dimmer-close",
                    Animation::new(Duration::from_millis(140)).with_easing(ease_in_out),
                    |dimmer, delta| dimmer.opacity(1.0 - delta),
                )
                .into_any_element()
        } else {
            dimmer
                .with_animation(
                    "update-dimmer-open",
                    Animation::new(Duration::from_millis(160)).with_easing(ease_in_out),
                    |dimmer, delta| dimmer.opacity(delta),
                )
                .into_any_element()
        };
        div()
            .id("update-dialog")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(dimmer)
            .child(ui_shared_modal::animated(
                variant,
                panel,
                self.update_dialog_closing,
                platform::reduce_motion(),
                780.0,
                590.0,
            ))
    }
}
