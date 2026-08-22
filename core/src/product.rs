//! Which game a session is for.
//!
//! Battle.net keys everything by a product's `FourCC`: the logon says which
//! program it is signing in as, the account state lists what the account owns
//! by program, and the game service hands back an endpoint for that program and
//! no other. That fact used to live as the string `"S2"` written into three
//! call sites inside the shared Battle.net layer, which is why there was no
//! way to sign in as anything else. It lives here now.

use crate::bgs::fourcc;

/// A Blizzard product, as Battle.net names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Product {
    StarCraft2,
    Remastered,
    Warcraft3,
}

/// The client profile a product presents at logon, as its retail client sends
/// it. Recovered per product: `StarCraft II`'s from its own SDK build string,
/// Remastered's from build `1.23.10_2e031d5be4`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogonProfile {
    pub platform: &'static str,
    pub locale: &'static str,
    /// The SDK build string. `StarCraft II`'s client sends one and no numeric
    /// version; Remastered's sends a number and no string. Both fields exist on
    /// the request, so each product fills the one its client fills.
    pub sdk_version: Option<&'static str>,
    pub application_version: u32,
}

/// `StarCraft II`'s. The SDK string is the whole of what it presents as a
/// version — `application_version` rides along as zero.
const STARCRAFT2_LOGON: LogonProfile = LogonProfile {
    platform: "Mc64",
    locale: "enUS",
    sdk_version: Some(crate::bgs::SC2_BGS_SDK_VERSION),
    application_version: 0,
};

/// Remastered's, from `sc1-research`'s `protocol.rs`. Same platform `FourCC` as
/// `StarCraft II` despite being a different client.
const REMASTERED_LOGON: LogonProfile = LogonProfile {
    platform: "Mc64",
    locale: "enUS",
    sdk_version: None,
    application_version: 65559,
};

/// Reforged's profile, recovered from retail build `2.0.4.23745`.
///
/// WC3 carries these values through its JSON BGS v2 logon rather than the
/// protobuf Front request used by SC2, but keeping the traced profile here
/// makes product availability a fact about the product instead of a UI flag.
const WARCRAFT3_LOGON: LogonProfile = LogonProfile {
    platform: "Mc64",
    locale: "enUS",
    sdk_version: None,
    application_version: 131_072,
};

impl Product {
    pub const ALL: [Self; 3] = [Self::StarCraft2, Self::Remastered, Self::Warcraft3];

    /// The `FourCC` as it reads on the wire.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::StarCraft2 => "S2",
            Self::Remastered => "S1",
            Self::Warcraft3 => "W3",
        }
    }

    /// The product's display name, for tracing and for anything that has to
    /// name it without reaching into the UI's palettes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::StarCraft2 => "StarCraft II",
            Self::Remastered => "StarCraft: Remastered",
            Self::Warcraft3 => "Warcraft III",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|product| product.code() == code)
    }

    /// The lowercase slug Live's wire contract and its viewer know the product
    /// by. The `FourCC` is Battle.net's name for it; this is ours, and it is
    /// spelled independently in the Worker and the browser build, which cannot
    /// depend on this crate — so it is pinned by test on both sides.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::StarCraft2 => "sc2",
            Self::Remastered => "scr",
            Self::Warcraft3 => "wc3",
        }
    }

    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|product| product.slug() == slug)
    }

    /// The `FourCC` packed the way the services want it.
    #[must_use]
    pub fn fourcc(self) -> u32 {
        fourcc(self.code())
    }

    /// How this product signs in.
    #[must_use]
    pub const fn logon(self) -> Option<LogonProfile> {
        match self {
            Self::StarCraft2 => Some(STARCRAFT2_LOGON),
            Self::Remastered => Some(REMASTERED_LOGON),
            Self::Warcraft3 => Some(WARCRAFT3_LOGON),
        }
    }

    /// Whether this client can currently sign in as it at all.
    #[must_use]
    pub const fn can_sign_in(self) -> bool {
        self.logon().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::Product;

    #[test]
    fn a_product_is_known_by_its_fourcc() {
        assert_eq!(Product::StarCraft2.code(), "S2");
        assert_eq!(Product::Remastered.code(), "S1");
        assert_eq!(Product::Warcraft3.code(), "W3");

        // the code is the round trip: it is what the account state reports and
        // what a game card is matched against
        for product in Product::ALL {
            assert_eq!(Product::from_code(product.code()), Some(product));
        }
        assert_eq!(Product::from_code("XX"), None);
    }

    #[test]
    fn the_live_slug_round_trips_and_matches_the_worker_contract() {
        // `live/worker/events.ts` accepts exactly these three strings; a change
        // here without one there would silently turn every envelope away
        assert_eq!(Product::StarCraft2.slug(), "sc2");
        assert_eq!(Product::Remastered.slug(), "scr");
        assert_eq!(Product::Warcraft3.slug(), "wc3");
        for product in Product::ALL {
            assert_eq!(Product::from_slug(product.slug()), Some(product));
        }
        assert_eq!(Product::from_slug("S2"), None);
    }

    #[test]
    fn every_enabled_product_has_a_traced_logon() {
        // a profile is recovered from a real client, never guessed: a wrong one
        // fails at the service, which is a worse place to find out
        assert!(Product::StarCraft2.can_sign_in());
        assert!(Product::Remastered.can_sign_in());
        assert!(Product::Warcraft3.can_sign_in());
    }

    #[test]
    fn each_client_presents_the_version_field_its_own_client_fills() {
        let sc2 = Product::StarCraft2.logon().unwrap();
        assert!(sc2.sdk_version.is_some());
        assert_eq!(sc2.application_version, 0);

        let remastered = Product::Remastered.logon().unwrap();
        assert!(remastered.sdk_version.is_none());
        assert_eq!(remastered.application_version, 65559);

        let reforged = Product::Warcraft3.logon().unwrap();
        assert!(reforged.sdk_version.is_none());
        assert_eq!(reforged.application_version, 131_072);
    }
}
