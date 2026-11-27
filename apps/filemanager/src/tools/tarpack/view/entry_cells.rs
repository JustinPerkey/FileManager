//! The entry table's copy and its cells: one function per column, each over
//! one [`SessionEntry`].
//!
//! The copy lives here because `text.rs` (the model's) carries none of the
//! table's words; it is today's `EntryTable.tsx` verbatim.

use std::fmt::Write as _;

use fm_tarpack::session::SessionEntry;
use fm_tarpack::sources::EntryStatus;
use gpui::prelude::*;
use gpui::{
    div, px, relative, AnyElement, App, Entity, FocusHandle, Font, Pixels, SharedString, Window,
};
use gpui_component::{h_flex, v_flex};

use crate::fonts::mono_family;
use crate::theme::tokens::{FONT_SIZE_MD, LINE_HEIGHT, RADIUS, REM, SPACE_1};
use crate::theme::{palette, Palette};
use crate::tools::tarpack::model::TarpackModel;
use crate::widgets::button::tooltip_view;
use crate::widgets::{button, status, tabular, IconName, MiddlePath, StatusTone};

// ---- the copy ----

/// The column headings, in column order.
pub const HEADINGS: [&str; 7] = [
    "Status",
    "File",
    "Windows location",
    "Linux target",
    "Mode",
    "Owner",
    "Actions",
];
pub const READY: &str = "Ready";
pub const MISSING: &str = "Missing";
pub const UNASSIGNED: &str = "Unassigned";
pub const BROWSE: &str = "Browse\u{2026}";
pub const CLEAR: &str = "Clear";
/// The Clear tooltip, before its shortcut.
pub const CLEAR_TIP: &str = "Forget this file (nothing is deleted)";
pub const EOL_MARKER: &str = "CRLF \u{2192} LF";
pub const EOL_TIP: &str =
    "Windows line endings (CRLF) are converted to Linux (LF) when the archive is written";
pub const EOL_NAME: &str = "line endings converted to LF";
pub const NO_FILE: &str = "none";
pub const NOT_FOUND: &str = "(not found)";
/// The table's accessible name.
pub const TABLE_LABEL: &str = "Files in this package";

/// "1 file", "2 files".
fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 {
        one
    } else {
        many
    }
}

/// The line above the table: how many files are ready, how many were left
/// out for errors, how many convert line endings.
#[must_use]
pub fn summary(total: usize, ready: usize, failed: usize, eol: usize) -> String {
    let files = plural(total, "file", "files");
    let mut out = if ready == total && failed == 0 {
        format!("All {total} {files} ready")
    } else {
        format!("{ready} of {total} {files} ready")
    };
    if failed > 0 {
        let f = plural(failed, "file", "files");
        let _ = write!(out, " \u{b7} {failed} {f} left out (errors)");
    }
    if eol > 0 {
        let c = plural(eol, "file converts", "files convert");
        let _ = write!(out, " \u{b7} {eol} {c} line endings to LF");
    }
    out
}

/// What the table says in place of rows: a lead and a next step.
#[must_use]
pub fn empty_text(withheld: bool, failed: usize) -> (&'static str, &'static str) {
    const ERRORS_NEXT: &str = "The errors are listed above. Fix them in your editor, then Reload.";
    if withheld {
        (
            "No files are listed. A manifest error hides them until it\u{2019}s fixed.",
            ERRORS_NEXT,
        )
    } else if failed > 0 {
        (
            "No files are listed. Every file in this manifest has errors.",
            ERRORS_NEXT,
        )
    } else {
        (
            "This manifest lists no files.",
            "Add a [[file]] entry to the manifest, then Reload.",
        )
    }
}

// ---- the cells ----

/// The width of one mono character at the body size.
fn mono_advance(window: &Window, cx: &App) -> Pixels {
    let style = window.text_style();
    let font = Font {
        family: mono_family(cx),
        ..style.font()
    };
    let font_id = window.text_system().resolve_font(&font);
    window
        .text_system()
        .em_advance(font_id, px(FONT_SIZE_MD * REM))
        .unwrap_or(px(0.0))
}

/// The base of a cell's content: body size, 1.4 line height, full width.
fn cell() -> gpui::Div {
    div()
        .w_full()
        .min_w_0()
        .text_size(px(FONT_SIZE_MD * REM))
        .line_height(relative(LINE_HEIGHT))
}

/// Status: icon and word, never colour alone.
pub fn status_cell(entry: &SessionEntry) -> AnyElement {
    let (tone, icon, word) = status_parts(entry.status);
    cell().child(status(tone, icon, word)).into_any_element()
}

/// The tone, icon and word of a status.
#[must_use]
pub fn status_parts(s: EntryStatus) -> (StatusTone, IconName, &'static str) {
    match s {
        EntryStatus::Ready => (StatusTone::Ok, IconName::CheckCircle, READY),
        EntryStatus::Missing => (StatusTone::Danger, IconName::XCircle, MISSING),
        EntryStatus::Unassigned => (StatusTone::Muted, IconName::Circle, UNASSIGNED),
    }
}

/// The `CRLF -> LF` marker.
fn eol_marker(row: usize, p: &Palette, cx: &App) -> AnyElement {
    div()
        .id(("tarpack-eol", row))
        .flex_none()
        .px(px(SPACE_1))
        .whitespace_nowrap()
        .font_family(mono_family(cx))
        .text_color(p.text_muted)
        .bg(p.surface_sunken)
        .border_1()
        .border_color(p.border)
        .rounded(px(RADIUS))
        .aria_label(EOL_NAME)
        .debug_selector(move || format!("tarpack-eol-{row}"))
        .tooltip(tooltip_view(EOL_TIP.into()))
        .child(EOL_MARKER)
        .into_any_element()
}

/// File: the name, then the marker when the entry converts line endings.
pub fn file_cell(entry: &SessionEntry, row: usize, cx: &App) -> AnyElement {
    let p = palette(cx);
    h_flex()
        .w_full()
        .min_w_0()
        .flex_wrap()
        .items_start()
        .gap_x(px(SPACE_1))
        .text_size(px(FONT_SIZE_MD * REM))
        .line_height(relative(LINE_HEIGHT))
        .child(
            div()
                .min_w_0()
                .line_clamp(2)
                .child(entry.view.source.clone()),
        )
        .when(entry.view.normalize_eol, |r| {
            r.child(eol_marker(row, p, cx))
        })
        .into_any_element()
}

/// Windows location: the assigned path cut in the middle, `none` when
/// unassigned, and the missing note on the second line.
pub fn windows_cell(entry: &SessionEntry, row: usize, cx: &App) -> AnyElement {
    let p = palette(cx);
    let col = v_flex().w_full().min_w_0();
    match &entry.assigned {
        None => col
            .child(cell().text_color(p.text_muted).child(NO_FILE))
            .into_any_element(),
        Some(path) => col
            .child(cell().child(MiddlePath::new(path.clone()).id(("tarpack-windows-path", row))))
            .when(entry.status == EntryStatus::Missing, |c| {
                c.child(cell().text_color(p.danger).child(NOT_FOUND))
            })
            .into_any_element(),
    }
}

/// Linux target: mono, up to two lines, the middle cut out of a longer path
/// (the whole path is its tooltip).
pub fn target_cell(
    entry: &SessionEntry,
    row: usize,
    width: Pixels,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let advance = mono_advance(window, cx);
    let shown = target_shown(&entry.view.target_path, width, advance);
    let truncated = shown != entry.view.target_path;
    let base = cell()
        .id(("tarpack-target", row))
        .font_family(mono_family(cx))
        .whitespace_normal()
        .line_clamp(2);
    if truncated {
        base.tooltip(tooltip_view(entry.view.target_path.clone().into()))
            .child(shown)
            .into_any_element()
    } else {
        base.child(shown).into_any_element()
    }
}

/// The path as drawn in a cell `width` wide: whole while it fits two lines,
/// else cut in the middle.
#[must_use]
pub fn target_shown(path: &str, width: Pixels, advance: Pixels) -> String {
    if advance <= px(0.0) {
        return path.to_string();
    }
    let per_line = (width / advance).floor().max(0.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a width in characters, clamped at zero"
    )]
    let per_line = per_line as usize;
    // Two lines, less one character of slack each for where a wrap falls.
    let max = (per_line * 2).saturating_sub(2);
    crate::widgets::middle_truncate(path, max)
}

/// Mode: symbolic on the first line, octal muted on the second.
pub fn mode_cell(entry: &SessionEntry, cx: &App) -> AnyElement {
    let p = palette(cx);
    let mono = mono_family(cx);
    v_flex()
        .w_full()
        .child(
            cell()
                .font_family(mono.clone())
                .whitespace_nowrap()
                .child(entry.view.mode_text.clone()),
        )
        .child(tabular(
            cell()
                .font_family(mono)
                .whitespace_nowrap()
                .text_color(p.text_muted)
                .child(entry.view.mode.clone()),
        ))
        .into_any_element()
}

/// Owner: wraps within two lines.
pub fn owner_cell(entry: &SessionEntry) -> AnyElement {
    cell()
        .whitespace_normal()
        .line_clamp(2)
        .child(entry.view.owner.clone())
        .into_any_element()
}

/// What the actions need beyond the entry.
pub struct Actions<'a> {
    pub model: &'a Entity<TarpackModel>,
    /// The table's focus handle: Clear leaves focus there.
    pub focus: &'a FocusHandle,
    pub building: bool,
}

/// The Browse and Clear names of row `row`, for the tests: `(selector, name)`.
#[must_use]
pub fn browse_name(entry: &SessionEntry) -> SharedString {
    format!("{BROWSE} for {}", entry.view.target_path).into()
}

/// The accessible name of Clear.
#[must_use]
pub fn clear_name(entry: &SessionEntry) -> SharedString {
    format!("{CLEAR} assigned file for {}", entry.view.target_path).into()
}

/// Actions: quiet Browse, and quiet Clear when a file is assigned. Neither is
/// a Tab stop; the table's Enter and Delete do the same.
pub fn actions_cell(entry: &SessionEntry, row: usize, a: &Actions) -> AnyElement {
    use crate::app::probe::Tag as _;

    let id = entry.view.id.clone();
    let browse = {
        let model = a.model.clone();
        let id = id.clone();
        button(("tarpack-browse", row), BROWSE)
            .quiet()
            .tab_stop(false)
            .accessible_name(browse_name(entry))
            .tooltip(format!("{BROWSE} for {}", entry.view.source))
            .shortcut("Enter")
            .disabled(a.building)
            .debug_selector(format!("tarpack-browse-{row}"))
            .on_click(move |_, _, cx| model.update(cx, |m, cx| m.browse(&id, cx)))
            .tag(format!("tarpack-browse-{row}"))
    };
    let clear = (entry.assigned.is_some()).then(|| {
        let model = a.model.clone();
        let focus = a.focus.clone();
        button(("tarpack-clear", row), CLEAR)
            .quiet()
            .tab_stop(false)
            .accessible_name(clear_name(entry))
            .tooltip(CLEAR_TIP)
            .shortcut("Delete")
            .disabled(a.building)
            .debug_selector(format!("tarpack-clear-{row}"))
            .on_click(move |_, window, cx| {
                // The button is about to leave the row: focus goes to the table
                // first, so it is not lost.
                focus.focus(window, cx);
                model.update(cx, |m, cx| m.clear(&id, cx));
            })
            .tag(format!("tarpack-clear-{row}"))
    });
    h_flex()
        .gap(px(SPACE_1))
        .items_start()
        .child(browse)
        .children(clear)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_cases() {
        assert_eq!(summary(5, 5, 0, 0), "All 5 files ready");
        assert_eq!(summary(1, 1, 0, 0), "All 1 file ready");
        assert_eq!(
            summary(5, 5, 0, 1),
            "All 5 files ready \u{b7} 1 file converts line endings to LF"
        );
        assert_eq!(
            summary(5, 5, 0, 2),
            "All 5 files ready \u{b7} 2 files convert line endings to LF"
        );
        assert_eq!(summary(5, 3, 0, 0), "3 of 5 files ready");
        // Failed entries: never "All", even when every listed file is ready.
        assert_eq!(
            summary(2, 2, 1, 0),
            "2 of 2 files ready \u{b7} 1 file left out (errors)"
        );
        assert_eq!(
            summary(4, 1, 2, 3),
            "1 of 4 files ready \u{b7} 2 files left out (errors) \u{b7} 3 files convert line endings to LF"
        );
    }

    #[test]
    fn status_words_icons_and_tones() {
        use StatusTone as T;
        assert_eq!(
            status_parts(EntryStatus::Ready),
            (T::Ok, IconName::CheckCircle, "Ready")
        );
        assert_eq!(
            status_parts(EntryStatus::Missing),
            (T::Danger, IconName::XCircle, "Missing")
        );
        assert_eq!(
            status_parts(EntryStatus::Unassigned),
            (T::Muted, IconName::Circle, "Unassigned")
        );
    }

    #[test]
    fn empty_text_cases() {
        assert!(empty_text(true, 0).0.contains("hides them"));
        assert!(empty_text(false, 2).0.contains("Every file"));
        let (lead, next) = empty_text(false, 0);
        assert_eq!(lead, "This manifest lists no files.");
        assert!(next.contains("[[file]]"));
    }

    #[test]
    fn target_path_is_cut_only_when_two_lines_cannot_hold_it() {
        let path = "/opt/gateway/bin/gateway";
        // 24 characters: fits two lines of 20.
        assert_eq!(target_shown(path, px(160.0), px(8.0)), path);
        let long = format!("/srv/{}/gateway.conf", "d".repeat(100));
        let shown = target_shown(&long, px(160.0), px(8.0));
        assert_eq!(shown.chars().count(), 38);
        assert!(shown.starts_with("/srv/"), "{shown}");
        assert!(shown.ends_with("/gateway.conf"), "{shown}");
        // No usable advance: the whole path (the cell clips it).
        assert_eq!(target_shown(&long, px(160.0), px(0.0)), long);
    }
}
