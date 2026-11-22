//! Fonts: system fonts only, nothing embedded. The UI font is Segoe UI; the
//! mono font is Cascadia Mono, falling back to Consolas. Windows 11 ships
//! both.

use gpui::{App, Global, SharedString};

/// The UI font family.
pub const UI_FAMILY: &str = "Segoe UI";

/// Mono families in order of preference; the first installed one is used.
pub const MONO_FAMILIES: [&str; 2] = ["Cascadia Mono", "Consolas"];

/// The first of [`MONO_FAMILIES`] present in `installed`, else the last entry
/// (GPUI's own fallback then applies).
#[must_use]
pub fn pick_mono<S: AsRef<str>>(installed: &[S]) -> &'static str {
    MONO_FAMILIES
        .iter()
        .copied()
        .find(|family| installed.iter().any(|name| name.as_ref() == *family))
        .unwrap_or(MONO_FAMILIES[MONO_FAMILIES.len() - 1])
}

struct MonoFamily(SharedString);

impl Global for MonoFamily {}

/// Resolves the mono family against the installed fonts and keeps it in a
/// global. Call once at startup.
pub fn init(cx: &mut App) {
    let family = pick_mono(&cx.text_system().all_font_names());
    cx.set_global(MonoFamily(SharedString::from(family)));
}

/// The mono family the theme and every mono text element use. Resolved by
/// [`init`]; resolved on the spot if that has not run.
#[must_use]
pub fn mono_family(cx: &App) -> SharedString {
    cx.try_global::<MonoFamily>().map_or_else(
        || SharedString::from(pick_mono(&cx.text_system().all_font_names())),
        |family| family.0.clone(),
    )
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    #[test]
    fn prefers_cascadia_mono() {
        assert_eq!(
            pick_mono(&["Arial", "Consolas", "Cascadia Mono"]),
            "Cascadia Mono"
        );
    }

    #[test]
    fn falls_back_to_consolas() {
        assert_eq!(pick_mono(&["Consolas", "Arial"]), "Consolas");
    }

    #[test]
    fn falls_back_when_neither_is_installed() {
        assert_eq!(pick_mono::<&str>(&[]), "Consolas");
    }

    #[gpui::test]
    fn mono_family_uses_the_text_system(cx: &mut TestAppContext) {
        cx.update(init);
        let family = cx.update(|cx| mono_family(cx));
        assert!(MONO_FAMILIES.contains(&family.as_ref()));
    }
}
