use gpui::ImageSource;

/// the two physical locations of one semantic UI asset.
///
/// Native resources retain the directory layout used by the desktop bundle;
/// the browser build serves its curated files below one public root. Product
/// packs name the role and provide these paths, while the host resolver decides
/// which location is active.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetPaths {
    pub native: &'static str,
    pub web: &'static str,
}

impl AssetPaths {
    #[must_use]
    pub const fn new(native: &'static str, web: &'static str) -> Self {
        Self { native, web }
    }
}

pub trait AssetResolver {
    fn image(&self, paths: AssetPaths) -> ImageSource;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NativeAssetResolver;

impl AssetResolver for NativeAssetResolver {
    fn image(&self, paths: AssetPaths) -> ImageSource {
        paths.native.into()
    }
}

#[derive(Clone, Debug)]
pub struct WebAssetResolver {
    root: String,
}

impl WebAssetResolver {
    #[must_use]
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            root: root.into().trim_end_matches('/').to_owned(),
        }
    }
}

impl AssetResolver for WebAssetResolver {
    fn image(&self, paths: AssetPaths) -> ImageSource {
        format!("{}/{}", self.root, paths.web.trim_start_matches('/')).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATHS: AssetPaths = AssetPaths::new("native/image.png", "web/image.png");

    #[test]
    fn web_resolver_normalizes_the_root_separator() {
        let source = WebAssetResolver::new("/assets/").image(PATHS);
        let ImageSource::Resource(resource) = source else {
            panic!("the web resolver must return a resource image");
        };
        assert_eq!(
            resource,
            gpui::Resource::Embedded("/assets/web/image.png".into())
        );
    }
}
