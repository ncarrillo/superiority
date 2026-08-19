use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    ops::Range,
    path::PathBuf,
    sync::{
        Arc,
        mpsc::{Receiver, Sender},
    },
    time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use gpui::WindowControlArea;
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, AssetSource, Bounds, ClipboardItem, Context,
    Div, DragMoveEvent, ElementId, FocusHandle, Focusable, FontWeight, ImageSource, KeyBinding,
    KeyDownEvent, Menu, MenuItem, MouseButton, MouseDownEvent, ObjectFit, RenderImage,
    ScrollHandle, ScrollStrategy, SharedString, Stateful, StyledImage, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, ease_in_out, img, linear_color_stop,
    linear_gradient, prelude::*, px, rgb, rgba, size, uniform_list,
};
use superiority_ui::{
    DigestEvent, MembershipEvent, MembershipKind, Portrait, PresenceKind, RosterChannelKind,
    RosterPresentation, RosterRelationship, RosterSegment, RosterUser, RosterUserTone,
    TranscriptLine, UiAssets, WithScrollbar as _, animation as ui_animation,
    components::{
        chat as ui_chat, controls as ui_controls, modal as ui_modal, navigation as ui_navigation,
        release_notes as ui_release_notes, roster as ui_roster, text_input as ui_text_input,
        workspace as ui_workspace,
    },
    theme::{
        BORDER_FOCUSED, BORDER_STRUCTURAL, COMPOSER_HEIGHT, FONT_INTERFACE, FONT_INTERNATIONAL,
        FONT_NAVIGATION, MUTED, NOTICE, ROSTER_ROW_GAP, ROSTER_ROW_HEIGHT, TAB_BAR_HEIGHT,
        TAB_LEADING_SPACE, TEXT, WINDOW_HEIGHT, WINDOW_WIDTH, edge_glow, focus_glow, popup_lift,
        selection_glow,
    },
};

use crate::{
    chat::{
        BlockedAccount, ChatChannel, ChatEvent, ChatFriend, ChatUser, JOIN_LOCALE, RosterSnapshot,
        channel_title, strip_character_code,
    },
    connection::DEFAULT_PUBLIC_CHANNEL,
    native::{ImageTableEntry, PresenceState, WhisperTarget, protocol::MAX_JOINED_CHANNELS},
    uplink,
};

use super::{
    ClientCommand, ClientEvent, ConnectionStage,
    preferences::{self, BACKGROUNDS},
    preview::{self, FixtureLine},
    spawn_client,
    update::{
        StartupCheckDisposition, UpdateModel, UpdatePrimaryAction, UpdateService, UpdateStage,
        startup_check_disposition,
    },
    web_auth::{WebAuthenticator, WebAuthenticatorHandle},
};

mod application;
mod channel;
mod chat;
pub(in crate::app) mod chrome;
mod composer;
mod dialogs;
mod interaction;
mod join;
mod navigation;
mod overlays;
pub(in crate::app) mod platform;
mod root;
mod roster;
mod session;
mod settings;
mod social;
mod state;
#[cfg(test)]
mod tests;

use channel::{ChannelComponent, ChannelState, TAB_CLOSE_DURATION, TabDragPayload};
use chat::{
    CHAT_ENTRY_REVEAL_DURATION, ChatComponent, ChatEntryReveal, ChatLine, shared_transcript_line,
};
use chrome::{
    Assets, ButtonFrames, ChromeComponent, ModalFrame, PortraitRegistry, load_top_nav_background,
};
use composer::{CommandKind, ComposerComponent, WhisperPeer, parse_command};
use dialogs::{
    CONNECTION_RAIL, CONNECTION_STEPS, ConnectionComponent, UpdateComponent, WarningComponent,
    WarningDialog,
};
use join::{
    InvitationKind, JoinComponent, JoinRow, JoinSource, UiGroupSummary, UiInvitation, count_color,
};
use overlays::{MODAL_CLOSE_DURATION, Overlay, OverlayComponent};
use roster::{
    ROSTER_DEBOUNCE_BASE, ROSTER_DEBOUNCE_MAX_LATENCY, ROSTER_DEBOUNCE_MAX_WINDOW,
    ROSTER_HOVER_DEFER_MAX, ROSTER_HOVER_RECHECK,
};
use roster::{
    RosterAffinity, RosterComponent, UiUser, presence_kind, presented_roster_entries,
    shared_roster_user,
};
use session::ClientRuntime;
use settings::{SETTINGS_PAGE_CROSSFADE_DURATION, SettingsComponent};
use social::{ConversationLine, SOCIAL_PANE_SLIDE_DURATION, SocialComponent, UiFriend};
#[cfg(test)]
use social::{friend_order, online_summary};
use state::*;

pub use application::run;

use platform::{
    About, AppMenuCommand, CheckForUpdates, NativeAppMenuTarget, OpenProtocolViewer, OpenSettings,
    Quit,
};
