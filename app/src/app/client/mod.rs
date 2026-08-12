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

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, AssetSource, Bounds, ClipboardItem, Context,
    Div, DragMoveEvent, ElementId, FocusHandle, Focusable, FontWeight, ImageSource, KeyBinding,
    KeyDownEvent, Menu, MenuItem, MouseButton, MouseDownEvent, ObjectFit, RenderImage,
    ScrollHandle, ScrollStrategy, SharedString, Stateful, StyledImage, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, ease_in_out, img, prelude::*, px,
    rgb, rgba, size, uniform_list,
};
use superiority_ui::{
    Portrait, PresenceKind, RosterUser, TranscriptLine, UiAssets, animation as ui_animation,
    components::{
        chat as ui_chat, controls as ui_controls, modal as ui_modal, navigation as ui_navigation,
        release_notes as ui_release_notes, roster as ui_roster, text_input as ui_text_input,
        workspace as ui_workspace,
    },
    theme::{
        COMPOSER_HEIGHT, FONT_INTERFACE, FONT_INTERNATIONAL, FONT_NAVIGATION, ROSTER_ROW_GAP,
        ROSTER_ROW_HEIGHT, TAB_BAR_HEIGHT, TAB_LEADING_SPACE, WINDOW_HEIGHT, WINDOW_WIDTH,
    },
};

use crate::{
    chat::{
        BlockedAccount, ChatChannel, ChatEvent, ChatFriend, ChatUser, RosterSnapshot,
        channel_title, public_channel_name, strip_character_code,
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
    Assets, BUTTON_ART_VERTICAL_BLEED, ButtonFrames, ChromeComponent, ModalFrame, PortraitRegistry,
    load_top_nav_background,
};
use composer::ComposerComponent;
use dialogs::{
    CONNECTION_RAIL, CONNECTION_STEPS, ConnectionComponent, UpdateComponent, WarningComponent,
    WarningDialog,
};
use join::{InvitationKind, JoinComponent, UiGroupSummary, UiInvitation};
use overlays::{MODAL_CLOSE_DURATION, Overlay, OverlayComponent};
use roster::{
    ROSTER_BOTTOM_TOLERANCE, ROSTER_DEBOUNCE_BASE, ROSTER_DEBOUNCE_MAX_LATENCY,
    ROSTER_DEBOUNCE_MAX_WINDOW, ROSTER_HOVER_DEFER_MAX, ROSTER_HOVER_RECHECK,
};
use roster::{RosterComponent, UiUser, filtered_roster_users, presence_kind, shared_roster_user};
use session::ClientRuntime;
use settings::{SETTINGS_PAGE_CROSSFADE_DURATION, SettingsComponent};
use social::{SOCIAL_PANE_SLIDE_DURATION, SocialComponent, UiFriend};
use state::*;

pub use application::run;

use platform::{
    About, AppMenuCommand, CheckForUpdates, NativeAppMenuTarget, OpenProtocolViewer, OpenSettings,
    Quit,
};
