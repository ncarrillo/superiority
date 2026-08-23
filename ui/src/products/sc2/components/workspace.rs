use gpui::{
    AnyElement, App, Div, ImageSource, IntoElement, ObjectFit, RenderOnce, ScrollHandle,
    StyledImage as _, Window, div, img, prelude::*, px, rgba,
};

use crate::products::sc2::theme::MARGIN;

use super::{
    chat::{self, TranscriptSelection},
    navigation::TabStripState,
    roster, shell,
};

type HoverHandler = Box<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

pub use crate::patterns::workspace::{ChannelChromeLayout, ChannelTransition, RosterState};

#[must_use]
pub fn channel_layout_for_viewport(width: f32, height: f32) -> ChannelChromeLayout {
    ChannelChromeLayout::responsive(
        width,
        height,
        ChannelChromeLayout::wide(MARGIN, crate::products::sc2::theme::ROSTER_WIDTH),
    )
}

pub struct NavigationState<K, C> {
    pub tabs: TabStripState<K, C>,
    pub scroll: ScrollHandle,
}

impl<K, C> Default for NavigationState<K, C> {
    fn default() -> Self {
        Self {
            tabs: TabStripState::default(),
            scroll: ScrollHandle::new(),
        }
    }
}

pub struct TranscriptState {
    pub selection: TranscriptSelection,
    pub scroll: ScrollHandle,
}

impl Default for TranscriptState {
    fn default() -> Self {
        Self {
            selection: TranscriptSelection::default(),
            scroll: ScrollHandle::new(),
        }
    }
}

pub struct WorkspaceState<K, T, C> {
    pub navigation: NavigationState<K, C>,
    pub transcript: TranscriptState,
    pub roster: RosterState<K, T, C>,
}

impl<K, T, C> Default for WorkspaceState<K, T, C> {
    fn default() -> Self {
        Self {
            navigation: NavigationState::default(),
            transcript: TranscriptState::default(),
            roster: RosterState::default(),
        }
    }
}

#[derive(IntoElement)]
pub struct ChannelWorkspace {
    navigation: AnyElement,
    chat: AnyElement,
    roster: AnyElement,
    footer: Option<AnyElement>,
    background: Option<ImageSource>,
    layout: ChannelChromeLayout,
}

impl ChannelWorkspace {
    #[must_use]
    pub fn new(
        navigation: impl IntoElement,
        chat: impl IntoElement,
        roster: impl IntoElement,
    ) -> Self {
        Self {
            navigation: navigation.into_any_element(),
            chat: chat.into_any_element(),
            roster: roster.into_any_element(),
            footer: None,
            background: None,
            layout: ChannelChromeLayout::wide(MARGIN, crate::products::sc2::theme::ROSTER_WIDTH),
        }
    }

    #[must_use]
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    #[must_use]
    pub fn background(mut self, background: impl Into<ImageSource>) -> Self {
        self.background = Some(background.into());
        self
    }

    #[must_use]
    pub const fn layout(mut self, layout: ChannelChromeLayout) -> Self {
        self.layout = layout;
        self
    }
}

impl RenderOnce for ChannelWorkspace {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        channel_chrome_with_layout(
            self.navigation,
            self.chat,
            self.roster,
            self.footer,
            self.background,
            self.layout,
        )
    }
}

#[derive(IntoElement)]
pub struct ChannelRoster {
    current_header: AnyElement,
    current_rows: AnyElement,
    outgoing_header: Option<AnyElement>,
    outgoing_rows: Option<AnyElement>,
    progress: Option<f32>,
    width: Option<f32>,
    overlays: Vec<AnyElement>,
    focused: bool,
    on_hover: Option<HoverHandler>,
}

impl ChannelRoster {
    #[must_use]
    pub fn new(current_header: impl IntoElement, current_rows: impl IntoElement) -> Self {
        Self {
            current_header: current_header.into_any_element(),
            current_rows: current_rows.into_any_element(),
            outgoing_header: None,
            outgoing_rows: None,
            progress: None,
            width: Some(crate::products::sc2::theme::ROSTER_WIDTH),
            overlays: Vec::new(),
            focused: false,
            on_hover: None,
        }
    }

    #[must_use]
    pub fn outgoing(
        mut self,
        header: impl IntoElement,
        rows: impl IntoElement,
        progress: Option<f32>,
    ) -> Self {
        self.outgoing_header = Some(header.into_any_element());
        self.outgoing_rows = Some(rows.into_any_element());
        self.progress = progress;
        self
    }

    #[must_use]
    pub const fn width(mut self, width: Option<f32>) -> Self {
        self.width = width;
        self
    }

    #[must_use]
    pub fn overlay(mut self, overlay: impl IntoElement) -> Self {
        self.overlays.push(overlay.into_any_element());
        self
    }

    #[must_use]
    pub const fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    #[must_use]
    pub fn on_hover(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ChannelRoster {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let header = roster::header_layer().child(transition_layers(
            self.current_header,
            self.outgoing_header,
            self.progress,
        ));
        let rows = roster::rows().child(transition_layers(
            self.current_rows,
            self.outgoing_rows,
            self.progress,
        ));
        let mut panel = roster::RosterPanel::new(header, rows)
            .width(self.width)
            .focused(self.focused);
        if let Some(on_hover) = self.on_hover {
            panel = panel.on_hover(on_hover);
        }
        for overlay in self.overlays {
            panel = panel.overlay(overlay);
        }
        panel
    }
}

#[derive(IntoElement)]
pub struct ChannelChat {
    current: AnyElement,
    outgoing: Option<AnyElement>,
    progress: Option<f32>,
    overlays: Vec<AnyElement>,
}

impl ChannelChat {
    #[must_use]
    pub fn new(current: impl IntoElement) -> Self {
        Self {
            current: current.into_any_element(),
            outgoing: None,
            progress: None,
            overlays: Vec::new(),
        }
    }

    #[must_use]
    pub fn outgoing(mut self, outgoing: impl IntoElement, progress: Option<f32>) -> Self {
        self.outgoing = Some(outgoing.into_any_element());
        self.progress = progress;
        self
    }

    #[must_use]
    pub fn overlay(mut self, overlay: impl IntoElement) -> Self {
        self.overlays.push(overlay.into_any_element());
        self
    }
}

impl RenderOnce for ChannelChat {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let mut panel = chat::ChatPanel::new().child(transition_layers(
            self.current,
            self.outgoing,
            self.progress,
        ));
        for overlay in self.overlays {
            panel = panel.child(overlay);
        }
        panel
    }
}

#[must_use]
pub fn root() -> Div {
    shell::root().relative()
}

#[must_use]
fn crossfade_layers(incoming: impl IntoElement, outgoing: impl IntoElement, progress: f32) -> Div {
    let progress = (progress).clamp(0.0, 1.0);
    div()
        .absolute()
        .inset_0()
        .child(div().absolute().inset_0().opacity(progress).child(incoming))
        .child(
            div()
                .absolute()
                .inset_0()
                .opacity(1.0 - progress)
                .child(outgoing),
        )
}

#[must_use]
fn transition_layers(
    current: impl IntoElement,
    outgoing: Option<AnyElement>,
    progress: Option<f32>,
) -> AnyElement {
    let current = current.into_any_element();
    let Some(outgoing) = outgoing else {
        return current;
    };
    match progress {
        None => outgoing,
        Some(progress) if progress < 1.0 => {
            crossfade_layers(current, outgoing, progress).into_any_element()
        }
        Some(_) => current,
    }
}

fn channel_chrome_with_layout(
    navigation: impl IntoElement,
    chat: impl IntoElement,
    roster: impl IntoElement,
    footer: Option<AnyElement>,
    background: Option<ImageSource>,
    layout: ChannelChromeLayout,
) -> Div {
    let mut panels = div()
        .flex()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap(px(layout.gap));
    if layout.stacked {
        panels = panels.flex_col().child(chat).child(
            div()
                .relative()
                .w_full()
                .h(px(layout.roster_height.unwrap_or(240.0)))
                .flex_shrink_0()
                .child(roster),
        );
    } else {
        panels = panels.child(chat).child(roster);
    }

    let mut content = div()
        .relative()
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap(px(layout.gap))
        .pt(px(layout.top_padding))
        .px(px(layout.margin))
        .pb(px(layout.margin));
    if let Some(background) = background {
        content = content.child(
            div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .child(img(background).size_full().object_fit(ObjectFit::Fill))
                .child(div().absolute().inset_0().bg(rgba(0x0407_0bc7))),
        );
    }
    content = content.child(panels).children(footer);

    div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .child(navigation)
        .child(content)
}

#[cfg(test)]
mod tests {
    use super::channel_layout_for_viewport;

    #[test]
    fn channel_layout_stacks_on_phone_sized_viewports() {
        let layout = channel_layout_for_viewport(390.0, 844.0);
        assert!(layout.stacked);
        assert_eq!(layout.roster_width, None);
        assert_eq!(layout.roster_height, Some(320.0));
        assert_eq!(layout.margin, 12.0);
    }

    #[test]
    fn channel_layout_uses_a_narrower_tablet_roster() {
        let layout = channel_layout_for_viewport(820.0, 1_180.0);
        assert!(!layout.stacked);
        assert_eq!(layout.roster_width, Some(260.0));
        assert_eq!(layout.margin, 16.0);
    }
}
