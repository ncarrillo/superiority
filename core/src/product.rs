use crate::bgs::fourcc;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Product {
    StarCraft2,
    Remastered,
    Warcraft3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogonProfile {
    pub platform: &'static str,
    pub locale: &'static str,
    pub sdk_version: Option<&'static str>,
    pub application_version: u32,
}

const STARCRAFT2_LOGON: LogonProfile = LogonProfile {
    platform: "Mc64",
    locale: "enUS",
    sdk_version: Some(crate::bgs::SC2_BGS_SDK_VERSION),
    application_version: 0,
};

const REMASTERED_LOGON: LogonProfile = LogonProfile {
    platform: "Mc64",
    locale: "enUS",
    sdk_version: None,
    application_version: 65559,
};

const WARCRAFT3_LOGON: LogonProfile = LogonProfile {
    platform: "Mc64",
    locale: "enUS",
    sdk_version: None,
    application_version: 131_072,
};

impl Product {
    pub const ALL: [Self; 3] = [Self::StarCraft2, Self::Remastered, Self::Warcraft3];

    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::StarCraft2 => "S2",
            Self::Remastered => "S1",
            Self::Warcraft3 => "W3",
        }
    }

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

    #[must_use]
    pub fn fourcc(self) -> u32 {
        fourcc(self.code())
    }

    #[must_use]
    pub const fn logon(self) -> Option<LogonProfile> {
        match self {
            Self::StarCraft2 => Some(STARCRAFT2_LOGON),
            Self::Remastered => Some(REMASTERED_LOGON),
            Self::Warcraft3 => Some(WARCRAFT3_LOGON),
        }
    }

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

        for product in Product::ALL {
            assert_eq!(Product::from_code(product.code()), Some(product));
        }
        assert_eq!(Product::from_code("XX"), None);
    }

    #[test]
    fn the_live_slug_round_trips_and_matches_the_worker_contract() {
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
