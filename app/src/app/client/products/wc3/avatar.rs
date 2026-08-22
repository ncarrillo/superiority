//! Reforged's official WebUI portrait catalogue.
//!
//! Aurora presence supplies stable ids such as `p126`. The bundled retail
//! client contains the corresponding portrait PNGs. WC3's canonical fallback
//! is the Orc Peon (`p003`); the retail `defaultportraits.png` file is a UI icon
//! atlas, not a member portrait.

const ROOT: &str = "images/products/wc3/portraits";
const DEFAULT_ID: &str = "p003";

fn is_catalogue_id(id: &str) -> bool {
    let Some(number) = id.strip_prefix('p') else {
        return false;
    };
    if number.len() != 3 || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let Ok(number) = number.parse::<u16>() else {
        return false;
    };
    (1..=62).contains(&number) || (68..=207).contains(&number)
}

#[must_use]
pub(super) fn source(id: Option<&str>) -> String {
    let id = id
        .map(str::trim)
        .filter(|id| is_catalogue_id(id))
        .unwrap_or(DEFAULT_ID);
    format!("{ROOT}/{id}.png")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_presence_ids_and_rejects_non_catalogue_values() {
        assert_eq!(
            source(Some("p126")),
            "images/products/wc3/portraits/p126.png"
        );
        assert_eq!(
            source(Some("p063")),
            "images/products/wc3/portraits/p003.png"
        );
        assert_eq!(
            source(Some("not-a-portrait")),
            "images/products/wc3/portraits/p003.png"
        );
        assert_eq!(source(None), "images/products/wc3/portraits/p003.png");
    }

    #[test]
    fn every_known_retail_portrait_has_an_extracted_png() {
        let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("macos/resources");
        for number in (1..=62).chain(68..=207) {
            let id = format!("p{number:03}");
            assert!(
                resources.join(format!("{ROOT}/{id}.png")).is_file(),
                "missing extracted WC3:R portrait {id}"
            );
        }
    }
}
