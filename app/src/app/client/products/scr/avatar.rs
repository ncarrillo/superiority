//! StarCraft: Remastered's profile-avatar catalogue.
//!
//! The profile service names images with ids such as
//! `avatar_terran_marine.jpg`; the application bundles the corresponding
//! canonical, unframed PNGs so roster rendering does not depend on a second
//! network request. Framed ids (`?f=1`) deliberately resolve to the same face
//! at roster size—the profile frame is presentation chrome, not identity.

use super::*;

const ROOT: &str = "images/products/scr/avatars";
const DEFAULT_ID: &str = "avatar_default_scrlogo";

const IDS: &[&str] = &[
    DEFAULT_ID,
    "avatar_neutral_contest_kerrigan",
    "avatar_neutral_protoss",
    "avatar_neutral_terran",
    "avatar_neutral_zerg",
    "avatar_protoss_advisor",
    "avatar_protoss_arbiter",
    "avatar_protoss_carrier",
    "avatar_protoss_darkarchon",
    "avatar_protoss_darktemplar",
    "avatar_protoss_dragoon",
    "avatar_protoss_interceptor",
    "avatar_protoss_observer",
    "avatar_protoss_probe",
    "avatar_protoss_templar",
    "avatar_protoss_zealot",
    "avatar_terran_advisor",
    "avatar_terran_battlecruiser",
    "avatar_terran_bomber",
    "avatar_terran_dropship",
    "avatar_terran_ghost",
    "avatar_terran_goliath",
    "avatar_terran_marine",
    "avatar_terran_medic",
    "avatar_terran_scv",
    "avatar_terran_spidermine",
    "avatar_terran_vulture",
    "avatar_zerg_advisor",
    "avatar_zerg_devourer",
    "avatar_zerg_drone",
    "avatar_zerg_guardian",
    "avatar_zerg_hydralisk",
    "avatar_zerg_lurker",
    "avatar_zerg_mutalisk",
    "avatar_zerg_overlord",
    "avatar_zerg_queen",
    "avatar_zerg_ultralisk",
    "avatar_zerg_zergling",
];

/// Produces a GPUI image source from either the profile service's URL or its
/// stable avatar id.
pub(super) fn source(value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with("https://") || value.starts_with("http://") {
        return Some(value.to_owned());
    }
    let id = value.split_once('?').map_or(value, |(id, _)| id);
    let id = id
        .strip_suffix(".jpg")
        .or_else(|| id.strip_suffix(".png"))
        .unwrap_or(id);
    IDS.contains(&id).then(|| format!("{ROOT}/{id}.png"))
}

#[must_use]
pub(super) fn default_source() -> String {
    format!("{ROOT}/{DEFAULT_ID}.png")
}

/// Some LegacyChat deployments attach the already-resolved profile value to
/// the member while others require ToonProfile hydration. Accept both shapes;
/// the latter is merged into the same member field by the session adapter.
pub(super) fn from_member(member: &ClassicChatUser) -> Option<String> {
    if let Some(avatar) = &member.avatar {
        if let Some(source) = avatar.id.as_deref().and_then(source) {
            return Some(source);
        }
        if let Some(source) = avatar.image_url.as_deref().and_then(source) {
            return Some(source);
        }
    }
    [
        "avatar_url",
        "avatar_id",
        "avatar",
        "profile_avatar",
        "toon_profile_avatar",
    ]
    .into_iter()
    .find_map(|name| member.attribute(name).and_then(source))
}

#[cfg(test)]
mod tests {
    use super::*;
    use superiority_core::games::scr::profile::Avatar;

    #[test]
    fn resolves_profile_ids_and_framed_ids_to_bundled_faces() {
        assert_eq!(
            source("avatar_terran_marine.jpg"),
            Some("images/products/scr/avatars/avatar_terran_marine.png".into())
        );
        assert_eq!(
            source("avatar_zerg_lurker.jpg?f=1"),
            Some("images/products/scr/avatars/avatar_zerg_lurker.png".into())
        );
        assert_eq!(source("not-an-avatar"), None);
    }

    #[test]
    fn preserves_profile_service_urls() {
        let url = "https://scrassets.classic.blizzard.com/avatar-icons/S1/example.png";
        assert_eq!(source(url), Some(url.into()));
    }

    #[test]
    fn a_hydrated_member_prefers_the_extracted_catalogue_asset() {
        let member = ClassicChatUser {
            name: "ncarrillo1".into(),
            flags: None,
            is_operator: false,
            avatar: Some(Avatar {
                image_url: Some(
                    "https://scrassets.classic.blizzard.com/avatar-icons/S1/hash.png".into(),
                ),
                id: Some("avatar_protoss_advisor.jpg".into()),
            }),
            attributes: Vec::new(),
        };
        assert_eq!(
            from_member(&member),
            Some("images/products/scr/avatars/avatar_protoss_advisor.png".into())
        );
    }

    #[test]
    fn every_catalogue_entry_has_an_extracted_png() {
        let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("macos/resources");
        for id in IDS {
            assert!(
                resources.join(format!("{ROOT}/{id}.png")).is_file(),
                "missing extracted SC:R avatar {id}"
            );
        }
        assert_eq!(IDS.len(), 38);
    }
}
