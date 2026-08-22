use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
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
    linear_gradient, prelude::*, px, relative, rgb, rgba, size, uniform_list,
};
use superiority_ui::{
    foundation::{WithScrollbar as _, animation as ui_animation, text_input as ui_text_input},
    patterns::{
        account as ui_account_pattern, roster as ui_roster_pattern,
        workspace as ui_workspace_pattern,
    },
    products::buttons as ui_buttons,
    products::inputs as ui_inputs,
    products::modal as ui_shared_modal,
    products::sc2::{
        DigestEvent, MembershipEvent, MembershipKind, Portrait, PresenceKind, RosterChannelKind,
        RosterPresentation, RosterRelationship, RosterSegment, RosterUser, RosterUserTone,
        Sc2Assets, TranscriptLine,
        components::{
            chat as ui_chat, controls as ui_controls, modal as ui_modal,
            navigation as ui_navigation, release_notes as ui_release_notes, roster as ui_roster,
            workspace as ui_workspace,
        },
        theme::{
            BORDER_FOCUSED, BORDER_STRUCTURAL, COMPOSER_HEIGHT, FONT_INTERFACE, FONT_INTERNATIONAL,
            FONT_NAVIGATION, MUTED, NOTICE, ROSTER_ROW_GAP, ROSTER_ROW_HEIGHT, TAB_BAR_HEIGHT,
            TAB_LEADING_SPACE, TEXT, WINDOW_HEIGHT, WINDOW_WIDTH, edge_glow, popup_lift,
            scope_glow,
        },
    },
    products::scr::{
        AccountIdentity as ScrAccountIdentity, RosterPresence as ScrRosterPresence,
        RosterUser as ScrRosterUser, TranscriptLine as ScrTranscriptLine,
        TranscriptMessageKind as ScrTranscriptMessageKind,
        TranscriptNoticeKind as ScrTranscriptNoticeKind, TranscriptSpeaker as ScrTranscriptSpeaker,
        components::{account as ui_scr_account, chat as ui_scr_chat, roster as ui_scr_roster},
        theme::{
            self as ui_scr_theme, ROSTER_ROW_GAP as SCR_ROSTER_ROW_GAP,
            ROSTER_ROW_HEIGHT as SCR_ROSTER_ROW_HEIGHT,
        },
    },
    products::wc3::{
        RosterPresence as Wc3RosterPresence, RosterUser as Wc3RosterUser,
        TranscriptLine as Wc3TranscriptLine,
        components::{chat as ui_wc3_chat, roster as ui_wc3_roster},
        theme as ui_wc3_theme,
    },
};

use crate::{
    chat::{
        BlockedAccount, ChatChannel, ChatEvent, ChatFriend, ChatUser, JOIN_LOCALE, RosterSnapshot,
        channel_title, strip_character_code,
    },
    connection::DEFAULT_PUBLIC_CHANNEL,
    product::Product,
    uplink,
};
// one game's wire protocol, named in full so the dependency is visible
use superiority_core::native::{
    ImageTableEntry, PresenceState, WhisperTarget, protocol::MAX_JOINED_CHANNELS,
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
mod interaction;
pub(in crate::app) mod platform;
mod products;
mod root;
mod sessions;
mod shell;
mod state;
#[cfg(test)]
mod tests;

pub(in crate::app) use products::sc2::chrome;
pub(in crate::app::client) use products::sc2::{channel, chat, composer, join, roster, social};
pub(in crate::app::client) use shell::{dialogs, games_list, modal_preview, overlays, settings};

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
use games_list::{GamesComponent, GamesScreen, STATE_DWELL};
use join::{
    InvitationKind, JoinComponent, JoinRow, JoinSource, UiGroupSummary, UiInvitation, count_color,
};
use modal_preview::ModalPreviewComponent;
use overlays::{MODAL_CLOSE_DURATION, Overlay, OverlayComponent};
use products::scr::{SCR_CHAT_ENTRY_REVEAL_DURATION, ScrSessionUi};
use products::wc3::Wc3SessionUi;
use roster::{
    ROSTER_DEBOUNCE_BASE, ROSTER_DEBOUNCE_MAX_LATENCY, ROSTER_DEBOUNCE_MAX_WINDOW,
    ROSTER_HOVER_DEFER_MAX, ROSTER_HOVER_RECHECK,
};
use roster::{
    RosterAffinity, RosterComponent, UiUser, presence_kind, presented_roster_entries,
    shared_roster_user,
};
use sessions::{ClientRuntime, ProductSession};
// one game's wire protocol, named in full so the dependency is visible
use settings::{SETTINGS_PAGE_CROSSFADE_DURATION, SettingsComponent};
use social::{
    ConversationLine, SOCIAL_PANE_SLIDE_DURATION, SocialComponent, SocialPaneTransition, UiFriend,
};
#[cfg(test)]
use social::{friend_order, online_summary};
use state::*;

pub use application::run;

use platform::{
    About, AppMenuCommand, CheckForUpdates, NativeAppMenuTarget, OpenProtocolViewer, OpenSettings,
    Quit,
};
