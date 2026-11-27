//! The config problems panel: a summary that opens onto one row per
//! diagnostic.

use fm_dbmanager::config::DiagnosticSeverity;
use fm_dbmanager::tool::DbmanagerSnapshot;
use gpui::prelude::*;
use gpui::{div, px, App, Entity, FocusHandle, IntoElement};
use gpui_component::{h_flex, v_flex};

use super::{copy, DbmanagerView};
use crate::app::probe::Tag as _;
use crate::theme::palette;
use crate::theme::tokens::{RADIUS, SPACE_1, SPACE_2, SPACE_3};
use crate::widgets::text::{mono_small_muted, small};
use crate::widgets::{button, icon, IconName};

/// The panel; `None` when the config has no problems to show.
pub fn config_problems(
    view: &Entity<DbmanagerView>,
    s: &DbmanagerSnapshot,
    open: bool,
    focus: &FocusHandle,
    cx: &App,
) -> Option<impl IntoElement> {
    if s.diagnostics.is_empty() {
        return None;
    }
    let p = *palette(cx);
    let toggle = {
        let view = view.clone();
        button(
            "db-config-problems-toggle",
            copy::problems_summary(s.error_count, s.warning_count),
        )
        .quiet()
        .icon(IconName::ChevronDown)
        .icon_flip(open)
        .track_focus(focus)
        .debug_selector("db-config-problems-toggle")
        .on_click(move |_, _, cx| {
            view.update(cx, |v, cx| {
                v.problems_open = !v.problems_open;
                cx.notify();
            });
        })
    };
    let rows = open.then(|| {
        v_flex()
            .gap(px(SPACE_1))
            .px(px(SPACE_3))
            .children(s.diagnostics.iter().enumerate().map(|(i, d)| {
                let (name, word, tone) = match d.severity {
                    DiagnosticSeverity::Error => {
                        (IconName::XCircle, copy::SEVERITY_ERROR, p.danger)
                    }
                    DiagnosticSeverity::Warning => {
                        (IconName::AlertTriangle, copy::SEVERITY_WARNING, p.warn)
                    }
                };
                let position = copy::position(d);
                h_flex()
                    .id(("db-config-problem", i))
                    .flex_wrap()
                    .items_baseline()
                    .gap_x(px(SPACE_2))
                    .child(
                        h_flex()
                            .gap(px(SPACE_1))
                            .items_center()
                            .flex_none()
                            .child(icon(name).text_color(tone))
                            .child(small(word)),
                    )
                    .when(!position.is_empty(), |row| {
                        row.child(mono_small_muted(position))
                    })
                    .child(div().min_w_0().child(small(d.message.clone())))
                    .debug_selector(move || format!("db-config-problem-{i}"))
            }))
    });
    Some(
        v_flex()
            .items_start()
            .gap(px(SPACE_1))
            .px(px(SPACE_3))
            .py(px(SPACE_2))
            .bg(p.surface)
            .border_1()
            .border_color(p.border)
            .rounded(px(RADIUS))
            .child(toggle)
            .children(rows)
            .debug_selector(|| "db-config-problems".to_string())
            .tag("db-config-problems"),
    )
}
