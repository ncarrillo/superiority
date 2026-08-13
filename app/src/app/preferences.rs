#[cfg(target_os = "macos")]
mod macos {
    use std::collections::BTreeMap;

    use objc2::AnyThread as _;
    use objc2::rc::Retained;
    use objc2_foundation::{NSBundle, NSString, NSUserDefaults};

    use crate::{chat::ChatChannel, native::protocol::MAX_JOINED_CHANNELS};

    const PREFERENCE_SUITE: &str = "com.superiority.sc2-chat";
    const BACKGROUND_KEY: &str = "chatBackgroundIndex";
    const SHOW_TIMESTAMPS_KEY: &str = "showChatTimestamps";
    const SHOW_MEMBERSHIP_KEY: &str = "showJoinLeaveNotifications";
    const LIVE_ENABLED_KEY: &str = "liveUplinkEnabled";
    const OPEN_CHANNELS_KEY: &str = "openChannels";
    const GROUP_NAMES_KEY: &str = "groupNames";
    const LEGACY_HOME_CHANNELS_KEY: &str = "homeChannels";

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Background {
        pub title: &'static str,
        pub path: &'static str,
    }

    pub const BACKGROUNDS: [Background; 8] = [
        Background {
            title: "Deep Nebula",
            path: "images/backgrounds/deep-nebula.png",
        },
        Background {
            title: "Shakuras Nebula",
            path: "images/backgrounds/shakuras-nebula.png",
        },
        Background {
            title: "Aiur Dusk",
            path: "images/backgrounds/aiur-dusk.png",
        },
        Background {
            title: "Swarm Horizon",
            path: "images/backgrounds/swarm-horizon.png",
        },
        Background {
            title: "Frozen Moon",
            path: "images/backgrounds/frozen-moon.png",
        },
        Background {
            title: "Midnight Front",
            path: "images/backgrounds/midnight-front.png",
        },
        Background {
            title: "Solar Fury",
            path: "images/backgrounds/solar-fury.png",
        },
        Background {
            title: "Last Stand",
            path: "images/backgrounds/last-stand.png",
        },
    ];

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct UserPreferences {
        pub background_index: usize,
        pub show_timestamps: bool,
        pub show_membership: bool,
        pub live_enabled: bool,
    }

    impl UserPreferences {
        pub fn load() -> Self {
            let defaults = app_defaults();
            let standard = NSUserDefaults::standardUserDefaults();
            let background_index = integer(&defaults, BACKGROUND_KEY)
                .or_else(|| integer(&standard, BACKGROUND_KEY))
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index < BACKGROUNDS.len())
                .unwrap_or_default();
            Self {
                background_index,
                show_timestamps: boolean_with_fallback(
                    &defaults,
                    &standard,
                    SHOW_TIMESTAMPS_KEY,
                    true,
                ),
                show_membership: boolean_with_fallback(
                    &defaults,
                    &standard,
                    SHOW_MEMBERSHIP_KEY,
                    true,
                ),
                live_enabled: boolean_with_fallback(&defaults, &standard, LIVE_ENABLED_KEY, false),
            }
        }

        pub fn background(self) -> &'static Background {
            &BACKGROUNDS[self.background_index]
        }
    }

    pub fn save_background(index: usize) {
        let Some(index) = BACKGROUNDS
            .get(index)
            .and_then(|_| isize::try_from(index).ok())
        else {
            return;
        };
        let defaults = app_defaults();
        defaults.setInteger_forKey(index, &NSString::from_str(BACKGROUND_KEY));
        defaults.synchronize();
        let standard = NSUserDefaults::standardUserDefaults();
        standard.setInteger_forKey(index, &NSString::from_str(BACKGROUND_KEY));
        standard.synchronize();
    }

    pub fn save_show_timestamps(value: bool) {
        save_boolean(SHOW_TIMESTAMPS_KEY, value);
    }

    pub fn save_show_membership(value: bool) {
        save_boolean(SHOW_MEMBERSHIP_KEY, value);
    }

    pub fn save_live_enabled(value: bool) {
        save_boolean(LIVE_ENABLED_KEY, value);
    }

    pub fn load_open_channels(default_channel: u16) -> Vec<ChatChannel> {
        let defaults = app_defaults();
        let standard = NSUserDefaults::standardUserDefaults();
        let stored = string(&defaults, OPEN_CHANNELS_KEY)
            .or_else(|| string(&standard, OPEN_CHANNELS_KEY))
            .or_else(|| string(&defaults, LEGACY_HOME_CHANNELS_KEY))
            .or_else(|| string(&standard, LEGACY_HOME_CHANNELS_KEY));
        let mut channels = stored
            .map(|value| {
                value
                    .lines()
                    .filter_map(parse_channel)
                    .fold(Vec::new(), |mut channels, channel| {
                        if !channels.contains(&channel) {
                            channels.push(channel);
                        }
                        channels
                    })
            })
            .unwrap_or_default();
        channels.truncate(MAX_JOINED_CHANNELS);
        if channels.is_empty() {
            channels.push(ChatChannel::Public(default_channel));
        }
        channels
    }

    pub fn save_open_channels(channels: &[ChatChannel]) {
        let value = channels
            .iter()
            .take(MAX_JOINED_CHANNELS)
            .map(serialize_channel)
            .collect::<Vec<_>>()
            .join("\n");
        save_string(OPEN_CHANNELS_KEY, &value);
    }

    pub fn load_group_names() -> BTreeMap<u32, String> {
        let defaults = app_defaults();
        let standard = NSUserDefaults::standardUserDefaults();
        string(&defaults, GROUP_NAMES_KEY)
            .or_else(|| string(&standard, GROUP_NAMES_KEY))
            .map(|value| {
                value
                    .lines()
                    .filter_map(|line| {
                        let (id, name) = line.split_once('\t')?;
                        let id = id.parse().ok()?;
                        (!name.is_empty()).then(|| (id, name.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn remember_group_name(club_id: u32, name: &str) {
        if name.is_empty() {
            return;
        }
        let mut names = load_group_names();
        if names.get(&club_id).is_some_and(|known| known == name) {
            return;
        }
        names.insert(club_id, name.to_owned());
        let value = names
            .iter()
            .map(|(id, name)| format!("{id}\t{name}"))
            .collect::<Vec<_>>()
            .join("\n");
        save_string(GROUP_NAMES_KEY, &value);
    }

    fn app_defaults() -> Retained<NSUserDefaults> {
        let standard = NSUserDefaults::standardUserDefaults();
        if NSBundle::mainBundle()
            .bundleIdentifier()
            .is_some_and(|identifier| identifier.to_string() == PREFERENCE_SUITE)
        {
            return standard;
        }
        suite_defaults(PREFERENCE_SUITE).unwrap_or_else(NSUserDefaults::standardUserDefaults)
    }

    fn suite_defaults(suite: &str) -> Option<Retained<NSUserDefaults>> {
        NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&NSString::from_str(suite)))
    }

    #[cfg(test)]
    fn load_from(defaults: &NSUserDefaults) -> UserPreferences {
        let background_index =
            usize::try_from(defaults.integerForKey(&NSString::from_str(BACKGROUND_KEY)))
                .ok()
                .filter(|index| *index < BACKGROUNDS.len())
                .unwrap_or_default();
        UserPreferences {
            background_index,
            show_timestamps: boolean(defaults, SHOW_TIMESTAMPS_KEY, true),
            show_membership: boolean(defaults, SHOW_MEMBERSHIP_KEY, true),
            live_enabled: boolean(defaults, LIVE_ENABLED_KEY, false),
        }
    }

    fn boolean(defaults: &NSUserDefaults, key: &str, fallback: bool) -> bool {
        let key = NSString::from_str(key);
        if defaults.objectForKey(&key).is_some() {
            defaults.boolForKey(&key)
        } else {
            fallback
        }
    }

    fn boolean_with_fallback(
        defaults: &NSUserDefaults,
        fallback_defaults: &NSUserDefaults,
        key: &str,
        fallback: bool,
    ) -> bool {
        let key_value = NSString::from_str(key);
        if defaults.objectForKey(&key_value).is_some() {
            defaults.boolForKey(&key_value)
        } else {
            boolean(fallback_defaults, key, fallback)
        }
    }

    fn integer(defaults: &NSUserDefaults, key: &str) -> Option<isize> {
        let key = NSString::from_str(key);
        defaults
            .objectForKey(&key)
            .map(|_| defaults.integerForKey(&key))
    }

    fn string(defaults: &NSUserDefaults, key: &str) -> Option<String> {
        defaults
            .stringForKey(&NSString::from_str(key))
            .map(|value| value.to_string())
    }

    fn save_string(key: &str, value: &str) {
        let defaults = app_defaults();
        unsafe {
            defaults.setObject_forKey(Some(&NSString::from_str(value)), &NSString::from_str(key));
        }
        defaults.synchronize();
        let standard = NSUserDefaults::standardUserDefaults();
        unsafe {
            standard.setObject_forKey(Some(&NSString::from_str(value)), &NSString::from_str(key));
        }
        standard.synchronize();
    }

    fn parse_channel(value: &str) -> Option<ChatChannel> {
        if let Some(value) = value.strip_prefix("public:") {
            value.parse().ok().map(ChatChannel::Public)
        } else if let Some(value) = value.strip_prefix("club:") {
            value.parse().ok().map(ChatChannel::Club)
        } else {
            value
                .strip_prefix("private:")
                .filter(|name| !name.is_empty())
                .map(|name| ChatChannel::Private(name.to_owned()))
        }
    }

    fn serialize_channel(channel: &ChatChannel) -> String {
        match channel {
            ChatChannel::Public(identifier) => format!("public:{identifier}"),
            ChatChannel::Private(name) => format!("private:{name}"),
            ChatChannel::Club(club_id) => format!("club:{club_id}"),
            ChatChannel::Party => "party".into(),
        }
    }

    fn save_boolean(key: &str, value: bool) {
        let defaults = app_defaults();
        defaults.setBool_forKey(value, &NSString::from_str(key));
        defaults.synchronize();
        let standard = NSUserDefaults::standardUserDefaults();
        standard.setBool_forKey(value, &NSString::from_str(key));
        standard.synchronize();
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn all_settings_survive_store_recreation() {
            let suite = format!("com.superiority.settings-test.{}", std::process::id());
            let suite_name = NSString::from_str(&suite);
            let defaults = suite_defaults(&suite).expect("test preference suite");
            defaults.removePersistentDomainForName(&suite_name);
            defaults.setInteger_forKey(6, &NSString::from_str(BACKGROUND_KEY));
            defaults.setBool_forKey(false, &NSString::from_str(SHOW_TIMESTAMPS_KEY));
            defaults.setBool_forKey(false, &NSString::from_str(SHOW_MEMBERSHIP_KEY));
            defaults.setBool_forKey(true, &NSString::from_str(LIVE_ENABLED_KEY));
            defaults.synchronize();
            drop(defaults);

            let reloaded = suite_defaults(&suite).expect("reloaded test preference suite");
            assert_eq!(
                load_from(&reloaded),
                UserPreferences {
                    background_index: 6,
                    show_timestamps: false,
                    show_membership: false,
                    live_enabled: true,
                }
            );
            reloaded.removePersistentDomainForName(&suite_name);
            reloaded.synchronize();
        }

        #[test]
        fn channel_preferences_round_trip() {
            for channel in [
                ChatChannel::Public(1),
                ChatChannel::Club(535_220),
                ChatChannel::Private("Nova".to_owned()),
            ] {
                assert_eq!(parse_channel(&serialize_channel(&channel)), Some(channel));
            }
        }

        #[test]
        fn invalid_channel_preferences_are_ignored() {
            for value in ["public:nope", "club:", "private:", "unknown:1"] {
                assert_eq!(parse_channel(value), None);
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "windows")]
mod windows {
    use std::{collections::BTreeMap, fs, path::PathBuf};

    use serde_json::{Map, Value};

    use crate::{chat::ChatChannel, native::protocol::MAX_JOINED_CHANNELS};

    const BACKGROUND_KEY: &str = "chatBackgroundIndex";
    const SHOW_TIMESTAMPS_KEY: &str = "showChatTimestamps";
    const SHOW_MEMBERSHIP_KEY: &str = "showJoinLeaveNotifications";
    const LIVE_ENABLED_KEY: &str = "liveUplinkEnabled";
    const OPEN_CHANNELS_KEY: &str = "openChannels";
    const GROUP_NAMES_KEY: &str = "groupNames";
    const LEGACY_HOME_CHANNELS_KEY: &str = "homeChannels";

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Background {
        pub title: &'static str,
        pub path: &'static str,
    }

    pub const BACKGROUNDS: [Background; 8] = [
        Background {
            title: "Deep Nebula",
            path: "images/backgrounds/deep-nebula.png",
        },
        Background {
            title: "Shakuras Nebula",
            path: "images/backgrounds/shakuras-nebula.png",
        },
        Background {
            title: "Aiur Dusk",
            path: "images/backgrounds/aiur-dusk.png",
        },
        Background {
            title: "Swarm Horizon",
            path: "images/backgrounds/swarm-horizon.png",
        },
        Background {
            title: "Frozen Moon",
            path: "images/backgrounds/frozen-moon.png",
        },
        Background {
            title: "Midnight Front",
            path: "images/backgrounds/midnight-front.png",
        },
        Background {
            title: "Solar Fury",
            path: "images/backgrounds/solar-fury.png",
        },
        Background {
            title: "Last Stand",
            path: "images/backgrounds/last-stand.png",
        },
    ];

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct UserPreferences {
        pub background_index: usize,
        pub show_timestamps: bool,
        pub show_membership: bool,
        pub live_enabled: bool,
    }

    impl UserPreferences {
        pub fn load() -> Self {
            let values = load_values();
            let background_index = values
                .get(BACKGROUND_KEY)
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index < BACKGROUNDS.len())
                .unwrap_or_default();
            Self {
                background_index,
                show_timestamps: boolean(&values, SHOW_TIMESTAMPS_KEY, true),
                show_membership: boolean(&values, SHOW_MEMBERSHIP_KEY, true),
                live_enabled: boolean(&values, LIVE_ENABLED_KEY, false),
            }
        }

        pub fn background(self) -> &'static Background {
            &BACKGROUNDS[self.background_index]
        }
    }

    pub fn save_background(index: usize) {
        if index < BACKGROUNDS.len() {
            save_value(BACKGROUND_KEY, Value::from(index));
        }
    }

    pub fn save_show_timestamps(value: bool) {
        save_value(SHOW_TIMESTAMPS_KEY, Value::Bool(value));
    }

    pub fn save_show_membership(value: bool) {
        save_value(SHOW_MEMBERSHIP_KEY, Value::Bool(value));
    }

    pub fn save_live_enabled(value: bool) {
        save_value(LIVE_ENABLED_KEY, Value::Bool(value));
    }

    pub fn load_open_channels(default_channel: u16) -> Vec<ChatChannel> {
        let values = load_values();
        let stored = string(&values, OPEN_CHANNELS_KEY)
            .or_else(|| string(&values, LEGACY_HOME_CHANNELS_KEY));
        let mut channels = stored
            .map(|value| {
                value
                    .lines()
                    .filter_map(parse_channel)
                    .fold(Vec::new(), |mut channels, channel| {
                        if !channels.contains(&channel) {
                            channels.push(channel);
                        }
                        channels
                    })
            })
            .unwrap_or_default();
        channels.truncate(MAX_JOINED_CHANNELS);
        if channels.is_empty() {
            channels.push(ChatChannel::Public(default_channel));
        }
        channels
    }

    pub fn save_open_channels(channels: &[ChatChannel]) {
        let value = channels
            .iter()
            .take(MAX_JOINED_CHANNELS)
            .map(serialize_channel)
            .collect::<Vec<_>>()
            .join("\n");
        save_value(OPEN_CHANNELS_KEY, Value::String(value));
    }

    pub fn load_group_names() -> BTreeMap<u32, String> {
        let values = load_values();
        string(&values, GROUP_NAMES_KEY)
            .map(|value| {
                value
                    .lines()
                    .filter_map(|line| {
                        let (id, name) = line.split_once('\t')?;
                        let id = id.parse().ok()?;
                        (!name.is_empty()).then(|| (id, name.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn remember_group_name(club_id: u32, name: &str) {
        if name.is_empty() {
            return;
        }
        let mut names = load_group_names();
        if names.get(&club_id).is_some_and(|known| known == name) {
            return;
        }
        names.insert(club_id, name.to_owned());
        let value = names
            .iter()
            .map(|(id, name)| format!("{id}\t{name}"))
            .collect::<Vec<_>>()
            .join("\n");
        save_value(GROUP_NAMES_KEY, Value::String(value));
    }

    fn preferences_path() -> Option<PathBuf> {
        std::env::var_os("APPDATA").map(|app_data| {
            PathBuf::from(app_data)
                .join("Superiority")
                .join("preferences.json")
        })
    }

    fn load_values() -> Map<String, Value> {
        preferences_path()
            .and_then(|path| fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default()
    }

    fn save_value(key: &str, value: Value) {
        let Some(path) = preferences_path() else {
            return;
        };
        let mut values = load_values();
        values.insert(key.to_owned(), value);
        let Some(directory) = path.parent() else {
            return;
        };
        if fs::create_dir_all(directory).is_err() {
            return;
        }
        if let Ok(encoded) = serde_json::to_vec_pretty(&values) {
            let _ = fs::write(path, encoded);
        }
    }

    fn boolean(values: &Map<String, Value>, key: &str, fallback: bool) -> bool {
        values.get(key).and_then(Value::as_bool).unwrap_or(fallback)
    }

    fn string(values: &Map<String, Value>, key: &str) -> Option<String> {
        values.get(key).and_then(Value::as_str).map(str::to_owned)
    }

    fn parse_channel(value: &str) -> Option<ChatChannel> {
        if let Some(value) = value.strip_prefix("public:") {
            value.parse().ok().map(ChatChannel::Public)
        } else if let Some(value) = value.strip_prefix("club:") {
            value.parse().ok().map(ChatChannel::Club)
        } else {
            value
                .strip_prefix("private:")
                .filter(|name| !name.is_empty())
                .map(|name| ChatChannel::Private(name.to_owned()))
        }
    }

    fn serialize_channel(channel: &ChatChannel) -> String {
        match channel {
            ChatChannel::Public(identifier) => format!("public:{identifier}"),
            ChatChannel::Private(name) => format!("private:{name}"),
            ChatChannel::Club(club_id) => format!("club:{club_id}"),
            ChatChannel::Party => "party".into(),
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows::*;
