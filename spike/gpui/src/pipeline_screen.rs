//! B6: today's Pipeline "folder chosen" screen (sidebar + Pipeline view),
//! rebuilt in GPUI with gpui-component controls, with fixture data only.
//!
//! A transcription of `app.css`, `controls.css`, `pipeline.css` and
//! `PipelineView.tsx`: every colour comes from the theme (itself from
//! `tokens.rs`) and every size, gap and weight from those files. Values GPUI
//! cannot express are listed in `REPORT.md`.

use gpui::{
    div, px, relative, AnyElement, AppContext as _, Context, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, SharedString, Styled as _,
    Window,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputState};
use gpui_component::{h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, Sizable as _};

use crate::assets::icon_path;
use crate::measure::Boxes;
use crate::theme::pick_font;
use crate::tokens::{
    FONT_MONO, FONT_SIZE_LG, FONT_SIZE_MD, FONT_SIZE_SM, FONT_SIZE_XL, LINE_HEIGHT, RADIUS, REM,
    SPACE_1, SPACE_2, SPACE_3, SPACE_4, SPACE_5,
};

const NONE: [f32; 4] = [0.0; 4];

pub const FOLDER: &str = r"C:\Deliveries\2026-10";

/// Canned step states for the screenshot scenario.
#[derive(Clone, Copy)]
pub struct StepFixture {
    pub title: &'static str,
    pub description: &'static str,
    pub status: &'static str,
    pub unlocked: bool,
    /// Title of the previous step, shown while this one is locked.
    pub waiting_for: Option<&'static str>,
    pub confirm: bool,
}

pub const STEPS: [StepFixture; 4] = [
    StepFixture {
        title: "Check required files",
        description: "Checks that every subfolder has the files it needs. Nothing is changed.",
        status: "Not run",
        unlocked: true,
        waiting_for: None,
        confirm: false,
    },
    StepFixture {
        title: "Run batch files",
        description: "Runs the batch file in each subfolder and reports the ones that fail.",
        status: "Waiting",
        unlocked: false,
        waiting_for: Some("Check required files"),
        confirm: true,
    },
    StepFixture {
        title: "Schedule",
        description: "Runs the schedule step for each subfolder.",
        status: "Waiting",
        unlocked: false,
        waiting_for: Some("Run batch files"),
        confirm: true,
    },
    StepFixture {
        title: "Pack archives",
        description: "Packs each subfolder into a tar archive.",
        status: "Waiting",
        unlocked: false,
        waiting_for: Some("Schedule"),
        confirm: true,
    },
];

const TOOLS: [&str; 3] = ["Pipeline", "Tar Packager", "Schedule Creator"];

pub struct PipelineScreen {
    input: Entity<InputState>,
    pub boxes: Boxes,
}

impl PipelineScreen {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            let mut s = InputState::new(window, cx).placeholder(r"C:\path\to\jobs");
            s.set_value(FOLDER, window, cx);
            s
        });
        Self {
            input,
            boxes: Boxes::default(),
        }
    }

    fn nav(&self, cx: &Context<Self>) -> impl IntoElement {
        let th = cx.theme();
        let items = TOOLS.iter().enumerate().map(|(i, label)| {
            let current = i == 0;
            let mut item = h_flex()
                .id(("nav", i))
                .items_center()
                .w_full()
                .px(px(SPACE_3))
                .py(px(SPACE_2))
                .border_1()
                .border_color(gpui::transparent_black())
                .rounded(px(RADIUS));
            if current {
                item = item
                    .bg(th.sidebar_accent)
                    .border_color(th.sidebar_border)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(
                        div()
                            .flex_none()
                            .size(px(6.0))
                            .mr(px(SPACE_2))
                            .rounded_full()
                            .bg(th.sidebar_primary),
                    );
            }
            item.child(SharedString::from(*label)).into_any_element()
        });
        v_flex()
            .id("sidebar")
            .relative()
            .flex_none()
            .w(px(12.0 * REM))
            .h_full()
            .bg(th.sidebar)
            .border_r_1()
            .border_color(th.sidebar_border)
            .px(px(SPACE_2))
            .py(px(SPACE_3))
            .gap(px(SPACE_1))
            .text_color(th.sidebar_foreground)
            .children(items)
            .child(self.boxes.probe("sidebar", [0.0, 0.0, 1.0, 0.0]))
    }

    fn header(&self, cx: &Context<Self>) -> impl IntoElement {
        v_flex()
            .gap(px(SPACE_1))
            .child(
                div()
                    .relative()
                    .text_size(px(FONT_SIZE_XL * REM))
                    .font_weight(FontWeight::BOLD)
                    .child("Pipeline")
                    .child(self.boxes.probe("title", NONE)),
            )
            .child(
                div()
                    .max_w(px(65.0 * 0.6 * FONT_SIZE_MD * REM))
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Choose a folder. Each subfolder is processed by the steps below, in order. \
                         Run one step at a time, or run them all.",
                    ),
            )
    }

    fn folder(&self, mono: &SharedString, cx: &Context<Self>) -> impl IntoElement {
        let th = cx.theme();
        let input = Input::new(&self.input)
            .h(px(32.0))
            .px(px(SPACE_2))
            .py(px(SPACE_1))
            .bg(th.popover)
            .border_color(th.border)
            .font_family(mono.clone())
            .text_color(th.foreground);
        v_flex()
            .gap(px(SPACE_2))
            .child(
                div()
                    .text_size(px(FONT_SIZE_LG * REM))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Folder"),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap(px(SPACE_3))
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_w_0()
                            .child(input)
                            .child(self.boxes.probe("input", NONE)),
                    )
                    .child(
                        div()
                            .relative()
                            .child(button("browse", "Browse…", false, false))
                            .child(self.boxes.probe("browse", NONE)),
                    ),
            )
            .child(div().text_color(th.muted_foreground).child("12 subfolders"))
    }

    fn run_bar(&self) -> impl IntoElement {
        h_flex()
            .items_center()
            .gap(px(SPACE_3))
            .child(
                div()
                    .relative()
                    .child(button(
                        "next",
                        "Run next: Check required files",
                        true,
                        false,
                    ))
                    .child(self.boxes.probe("primary", NONE)),
            )
            .child(
                div()
                    .relative()
                    .child(button("all", "Run all…", false, false))
                    .child(self.boxes.probe("secondary", NONE)),
            )
    }

    fn step(&self, i: usize, s: StepFixture, cx: &Context<Self>) -> AnyElement {
        let th = cx.theme();
        let names = ["step1", "step2", "step3", "step4"];
        let number = div()
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(px(1.5 * REM))
            .rounded_full()
            .bg(th.muted)
            .border_1()
            .border_color(th.border)
            .text_size(px(FONT_SIZE_SM * REM))
            .child(SharedString::from((i + 1).to_string()));
        let mut text = v_flex()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .text_size(px(FONT_SIZE_LG * REM))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(s.title),
            )
            .child(div().text_color(th.muted_foreground).child(s.description));
        if let Some(prev) = s.waiting_for.filter(|_| !s.unlocked) {
            text = text.child(
                div()
                    .mt(px(SPACE_1))
                    .text_size(px(FONT_SIZE_SM * REM))
                    .text_color(th.muted_foreground)
                    .child(SharedString::from(format!("Runs after {prev} passes."))),
            );
        }
        let status = h_flex()
            .flex_none()
            .items_center()
            .gap(px(SPACE_1))
            .min_w(px(9.0 * REM))
            .text_color(th.muted_foreground)
            .child(
                Icon::empty()
                    .path(icon_path("circle"))
                    .with_size(px(FONT_SIZE_MD * REM))
                    .text_color(th.muted_foreground),
            )
            .child(s.status);
        v_flex()
            .id(("step", i))
            .relative()
            .gap(px(SPACE_2))
            .py(px(SPACE_3))
            .border_b_1()
            .border_color(th.border)
            .child(
                h_flex()
                    .items_center()
                    .gap(px(SPACE_3))
                    .child(number)
                    .child(text)
                    .child(status)
                    .child(button(
                        ("run", i),
                        if s.confirm { "Run…" } else { "Run" },
                        false,
                        !s.unlocked,
                    )),
            )
            .child(self.boxes.probe(names[i], [0.0, 0.0, 0.0, 1.0]))
            .into_any_element()
    }
}

/// A button as `controls.css` draws it: min 24x24, padding 4px 12px, 1px
/// border, `--radius`; disabled = muted text on the sunken fill at 55%.
fn button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    primary: bool,
    disabled: bool,
) -> impl IntoElement {
    let b = Button::new(id)
        .label(label)
        .h(px(FONT_SIZE_MD * REM * LINE_HEIGHT + 2.0 * SPACE_1 + 2.0))
        .px(px(SPACE_3))
        .disabled(disabled);
    if primary {
        b.primary()
    } else {
        b
    }
}

impl Render for PipelineScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mono = pick_font(cx, FONT_MONO, "monospace");
        let th = cx.theme();
        let steps = STEPS
            .iter()
            .enumerate()
            .map(|(i, s)| self.step(i, *s, cx))
            .collect::<Vec<_>>();
        let first_rule = th.border;
        h_flex()
            .size_full()
            .bg(th.background)
            .text_color(th.foreground)
            .text_size(px(FONT_SIZE_MD * REM))
            .line_height(relative(LINE_HEIGHT))
            .overflow_hidden()
            .child(self.nav(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .px(px(SPACE_5))
                    .py(px(SPACE_4))
                    .gap(px(SPACE_4))
                    .child(self.header(cx))
                    .child(self.folder(&mono, cx))
                    .child(self.run_bar())
                    .child(
                        v_flex()
                            .border_t_1()
                            .border_color(first_rule)
                            .children(steps),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;

    /// Reference boxes measured by pixel inspection of
    /// `docs/manual/images/pipeline-chosen.png` (x, y, w, h).
    #[gpui::test]
    fn text_independent_boxes_match_the_reference(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        cx.update(|cx| crate::theme::apply(false, cx));
        let (view, cx) = cx.add_window_view(PipelineScreen::new);
        cx.simulate_resize(gpui::size(px(1000.0), px(700.0)));
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        let boxes = view.read_with(cx, |v, _| v.boxes.clone());
        let want = [
            ("sidebar", (0.0, 0.0, 192.0, 700.0)),
            ("title", (216.0, 16.0, 760.0, 28.0)),
            ("input", (216.0, 134.0, 661.0, 32.0)),
            ("primary", (216.0, 209.0, f32::NAN, 30.0)),
            ("secondary", (f32::NAN, 209.0, f32::NAN, 30.0)),
            // Row heights and later rows depend on whether the description wraps,
            // which depends on the font; the CI check on real renders covers them.
            ("step1", (216.0, 256.0, 760.0, f32::NAN)),
            ("step2", (216.0, f32::NAN, 760.0, f32::NAN)),
            ("step3", (216.0, f32::NAN, 760.0, f32::NAN)),
            ("step4", (216.0, f32::NAN, 760.0, f32::NAN)),
        ];
        for (name, (x, y, w, h)) in want {
            let got = boxes.get(name).unwrap_or_else(|| panic!("no box {name}"));
            for (what, g, r) in [
                ("x", got.x, x),
                ("y", got.y, y),
                ("w", got.w, w),
                ("h", got.h, h),
            ] {
                assert!(
                    r.is_nan() || (g - r).abs() <= 2.0,
                    "{name}.{what}: got {g}, reference {r}"
                );
            }
        }
    }
}
