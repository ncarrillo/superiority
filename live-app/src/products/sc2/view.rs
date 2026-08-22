use std::{collections::HashMap, ops::Range, time::Duration};

use super::api::{
    ChannelSummary, RosterMember, Sender, StreamItem, fetch_overview, fetch_roster, fetch_stream,
};
use crate::shell::FeedProduct;
use gpui::{
    Animation, AnimationExt as _, AnyElement, ClipboardItem, Context, Div, DragMoveEvent,
    FocusHandle, Focusable, KeyDownEvent, MouseButton, MouseDownEvent, Render, ScrollStrategy,
    Stateful, UniformListScrollHandle, Window, div, ease_in_out, prelude::*, px, rgb, uniform_list,
};
use superiority_ui::{
    foundation::WithScrollbar as _,
    products::sc2::{
        Portrait, PresenceKind, RosterUser, Sc2Assets,
        components::{chat, navigation, roster, workspace},
        theme::{FONT_INTERFACE, FONT_NAVIGATION, MARGIN, MUTED, ONLINE, ROSTER_ROW_GAP},
    },
};

mod interaction;
mod model;
mod navigation_view;
mod polling;
mod roster_view;
mod workspace_view;

use model::*;

const CHANNEL_REFRESH_MS: f64 = 8_000.0;
const CHANNEL_FRESH_MS: f64 = 12_000.0;

type TabDragPayload = navigation::TabDragPayload;
type LiveWorkspace = workspace::WorkspaceState<String, RosterMember, f64>;

pub(crate) struct LiveView {
    focus_handle: FocusHandle,
    backend: String,
    feed_id: String,
    asset_root: String,
    ui_assets: Sc2Assets,
    feed_state: String,
    had_session: bool,
    session_id: Option<String>,
    channels: Vec<ChannelSummary>,
    selected: Option<String>,
    channel_data: HashMap<String, ChannelData>,
    roster_filters: HashMap<String, String>,
    channel_transition: Option<ChannelTransition>,
    workspace: LiveWorkspace,
    active: bool,
    error: Option<String>,
}

impl LiveView {
    pub fn new(
        backend: String,
        feed_id: String,
        asset_root: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let asset_root = asset_root.trim_end_matches('/').to_owned();
        let view = Self {
            focus_handle: cx.focus_handle(),
            backend: backend.trim_end_matches('/').to_owned(),
            feed_id,
            ui_assets: Sc2Assets::load(&superiority_ui::foundation::assets::WebAssetResolver::new(
                &asset_root,
            )),
            asset_root,
            feed_state: "connecting".into(),
            had_session: false,
            session_id: None,
            channels: Vec::new(),
            selected: None,
            channel_data: HashMap::new(),
            roster_filters: HashMap::new(),
            channel_transition: None,
            workspace: LiveWorkspace::default(),
            active: false,
            error: None,
        };
        view.start_polling(cx);
        view
    }

    /// The shell parks every view but the focused one; a parked view keeps its
    /// state and stops polling streams.
    pub(crate) fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active != active {
            self.active = active;
            cx.notify();
        }
    }
}

impl Focusable for LiveView {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for LiveView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.update_transition(window);
        let viewport = window.viewport_size();
        let layout = workspace::channel_layout_for_viewport(
            f32::from(viewport.width),
            f32::from(viewport.height),
        );
        workspace::root()
            .track_focus(&self.focus_handle)
            .on_mouse_down(MouseButton::Left, {
                let focus = self.focus_handle.clone();
                move |_, window, cx| focus.focus(window, cx)
            })
            .on_key_down(cx.listener(Self::on_key_down))
            .on_drag_move::<TabDragPayload>(cx.listener(Self::update_tab_drag))
            .on_drop(cx.listener(Self::finish_tab_drag))
            .child(
                workspace::ChannelWorkspace::new(
                    self.top_navigation(window, layout.stacked, cx),
                    self.chat_panel(),
                    self.roster_panel(layout.roster_width, window, cx),
                )
                .layout(layout)
                .background(self.asset("ui/deep-nebula.png")),
            )
    }
}
