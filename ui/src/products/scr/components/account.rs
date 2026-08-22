//! SC:R's closed account control and open account menu.
//!
//! These deliberately follow the shared desktop account interaction—portrait
//! button, identity card, Settings, Games List—without borrowing SC2's blue
//! frames or nine-patch buttons.

use gpui::{
    App, ClickEvent, Div, IntoElement, MouseButton, ObjectFit, RenderOnce, SharedString, Stateful,
    Window, div, img, linear_color_stop, linear_gradient, prelude::*, px, rgb, rgba,
};

use crate::products::scr::{
    AccountIdentity,
    theme::{
        ACCENT, BORDER_FOCUSED, BORDER_STRUCTURAL, FONT_INTERFACE, FONT_INTERNATIONAL, MUTED,
        PANEL_BACKGROUND, PANEL_SHELL, TEXT,
    },
};

pub const MENU_WIDTH: f32 = 292.0;
pub const MENU_HEIGHT: f32 = 180.0;
const DIVIDER_TOP: f32 = 66.0;
const BUTTON_TOP: f32 = 74.0;
const BUTTON_HEIGHT: f32 = 42.0;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct AccountButton {
    portrait: gpui::ImageSource,
    name: SharedString,
    open: bool,
    on_click: Option<ClickHandler>,
}

impl AccountButton {
    #[must_use]
    pub fn new(
        portrait: impl Into<gpui::ImageSource>,
        name: impl Into<SharedString>,
        open: bool,
    ) -> Self {
        Self {
            portrait: portrait.into(),
            name: name.into(),
            open,
            on_click: None,
        }
    }

    #[must_use]
    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for AccountButton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        // an account plaque, not a corner tile: name and portrait in one
        // hairline-bordered readout at the titlebar's end
        let mut control = div()
            .id("scr-account")
            .flex_shrink_0()
            .h(px(30.0))
            .pl(px(10.0))
            .pr(px(3.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .cursor_pointer()
            .border_1()
            .border_color(if self.open {
                rgb(BORDER_FOCUSED).into()
            } else {
                rgba(0xc93a_2c99)
            })
            .bg(if self.open {
                rgba(0xc93a_2c29)
            } else {
                rgba(0x1406_04cc)
            })
            .hover(|style| style.border_color(rgb(ACCENT)).bg(rgba(0xc93a_2c29)))
            .active(|style| style.opacity(0.78))
            // The surrounding SC:R header is a drag surface. The plaque is
            // a control, so its press must never start a window drag.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(11.0))
                    .text_color(rgb(ACCENT))
                    .whitespace_nowrap()
                    .child(self.name),
            )
            .child(
                div()
                    .size(px(24.0))
                    .flex_shrink_0()
                    .border_1()
                    .border_color(rgb(0x00c9_3a2c))
                    .bg(rgb(PANEL_SHELL))
                    .child(
                        img(self.portrait)
                            .size(px(22.0))
                            .object_fit(ObjectFit::Cover),
                    ),
            );
        if let Some(on_click) = self.on_click {
            control = control.on_click(move |event, window, cx| on_click(event, window, cx));
        }
        control
    }
}

#[derive(IntoElement)]
pub struct AccountMenu {
    identity: AccountIdentity,
    on_settings: Option<ClickHandler>,
    on_games_list: Option<ClickHandler>,
}

impl AccountMenu {
    #[must_use]
    pub const fn new(identity: AccountIdentity) -> Self {
        Self {
            identity,
            on_settings: None,
            on_games_list: None,
        }
    }

    #[must_use]
    pub fn on_settings(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_settings = Some(Box::new(handler));
        self
    }

    #[must_use]
    pub fn on_games_list(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_games_list = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for AccountMenu {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let AccountIdentity {
            name,
            detail,
            detail_color,
            portrait,
        } = self.identity;
        let mut settings = menu_button("scr-account-settings", "SETTINGS", false);
        if let Some(on_settings) = self.on_settings {
            settings = settings.on_click(move |event, window, cx| on_settings(event, window, cx));
        }
        let mut games = menu_button("scr-account-games-list", "GAMES LIST", true);
        if let Some(on_games_list) = self.on_games_list {
            games = games.on_click(move |event, window, cx| on_games_list(event, window, cx));
        }
        div()
            .id("scr-account-menu")
            .relative()
            .w(px(MENU_WIDTH))
            .h(px(MENU_HEIGHT))
            .overflow_hidden()
            .bg(rgba(0x0803_02fc))
            .border_1()
            .border_color(rgba(0xc93a_2cd9))
            .rounded(px(2.0))
            .shadow(vec![
                gpui::BoxShadow::new(px(0.0), px(8.0), rgba(0x0000_00a6).into())
                    .blur_radius(px(24.0)),
                gpui::BoxShadow::new(px(0.0), px(0.0), rgba(0xc93a_2c29).into())
                    .blur_radius(px(12.0)),
            ])
            .font_family(FONT_INTERFACE)
            .on_click(|_, _, cx| cx.stop_propagation())
            .child(
                // A restrained scan-line wash keeps the menu in the same
                // console family without sacrificing the account text.
                div().absolute().inset_0().bg(linear_gradient(
                    180.0,
                    linear_color_stop(rgba(0x1b0a_0873), 0.0),
                    linear_color_stop(rgba(0x0803_0200), 0.55),
                )),
            )
            .child(
                div()
                    .absolute()
                    .left(px(16.0))
                    .top(px(9.0))
                    .size(px(52.0))
                    .border_1()
                    .border_color(rgba(BORDER_STRUCTURAL))
                    .bg(rgb(PANEL_SHELL))
                    .child(
                        img(portrait)
                            .absolute()
                            .inset(px(4.0))
                            .size(px(42.0))
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset(px(2.0))
                            .border_1()
                            .border_color(rgba(0xc93a_2c52)),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .left(px(80.0))
                    .top(px(11.0))
                    .right(px(18.0))
                    .h(px(23.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_family(FONT_INTERNATIONAL)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_size(px(14.0))
                    .text_color(rgb(TEXT))
                    .child(name),
            )
            .child(
                div()
                    .absolute()
                    .left(px(80.0))
                    .top(px(36.0))
                    .right(px(18.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_size(px(11.5))
                    .text_color(rgb(detail_color))
                    .child(detail),
            )
            .child(
                div()
                    .absolute()
                    .left(px(18.0))
                    .right(px(18.0))
                    .top(px(DIVIDER_TOP))
                    .h(px(1.0))
                    .bg(rgba(BORDER_STRUCTURAL)),
            )
            .child(settings.absolute().left(px(16.0)).top(px(BUTTON_TOP)))
            .child(
                games
                    .absolute()
                    .left(px(16.0))
                    .top(px(BUTTON_TOP + BUTTON_HEIGHT + 10.0)),
            )
    }
}

fn menu_button(id: &'static str, label: &'static str, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(260.0))
        .h(px(BUTTON_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .border_1()
        .border_color(if primary {
            rgba(0xc93a_2cd9)
        } else {
            rgba(BORDER_STRUCTURAL)
        })
        .bg(if primary {
            rgba(0x3d12_0ecc)
        } else {
            rgb(PANEL_BACKGROUND).into()
        })
        .font_family(FONT_INTERFACE)
        .font_weight(gpui::FontWeight::BOLD)
        .text_size(px(12.0))
        .text_color(if primary { rgb(TEXT) } else { rgb(MUTED) })
        .hover(|style| {
            style
                .border_color(rgb(ACCENT))
                .bg(rgba(0x4e12_0ee6))
                .text_color(rgb(0xffffff))
        })
        .active(|style| style.opacity(0.72))
        .child(label)
}
