//! Embedded assets: the SVG icons of `apps/desktop/src/app/icons.tsx`
//! (`assets/icons/*.svg`), then gpui-kit's bundled set as the fallback.
//!
//! A UI task adds an icon by dropping its SVG into `assets/icons/` and adding
//! its name to the `icons!` list below: one name per line.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

macro_rules! icons {
    ($($name:literal),+ $(,)?) => {
        const ICONS: &[(&str, &[u8])] = &[
            $((concat!("icons/", $name, ".svg"),
               include_bytes!(concat!("../assets/icons/", $name, ".svg")).as_slice())),+
        ];
        /// Icon names, as in `icons.tsx`.
        pub const ICON_NAMES: &[&str] = &[$($name),+];
    };
}

icons!(
    "check-circle",
    "alert-triangle",
    "circle",
    "x-circle",
    "info",
    "chevron-down",
    "folder",
    "copy",
    "x",
    "keyboard",
);

/// Path of an icon asset by its `icons.tsx` name.
#[must_use]
pub fn icon_path(name: &str) -> SharedString {
    SharedString::from(format!("icons/{name}.svg"))
}

/// Our transcribed icons first, then gpui-kit's bundled set: its checkbox
/// check mark, menu check and dialog close icons are loaded from there.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = ICONS.iter().find(|(p, _)| *p == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        gpui_kit_assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut all: Vec<SharedString> = ICONS
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from((*p).to_string()))
            .collect();
        all.extend(gpui_kit_assets::Assets.list(path)?);
        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpui_kit_assets_are_the_fallback() {
        // gpui-kit's bundle (the checkbox mark) is reached through ours.
        assert!(Assets.load("icons/check.svg").unwrap().is_some());
    }

    #[test]
    fn every_icon_loads_and_is_svg() {
        for name in ICON_NAMES {
            let bytes = Assets
                .load(&format!("icons/{name}.svg"))
                .unwrap()
                .expect(name);
            assert!(bytes.starts_with(b"<svg"), "{name}");
        }
        assert!(
            Assets.load("icons/nope.svg").map_or(true, |b| b.is_none()),
            "unknown asset is an error or absent"
        );
    }
}
