use gpui::{bounds, point, px, size, TestAppContext};

use super::*;
use crate::app::shell::test_support::open;

fn sample() -> Scenario {
    scenarios()
        .into_iter()
        .find(|s| s.name == "shell-sample")
        .expect("the sample scenario exists")
}

#[test]
fn scenario_names_are_unique_and_valid_file_stems() {
    let all = scenarios();
    assert!(!all.is_empty());
    let mut seen = std::collections::HashSet::new();
    for s in &all {
        assert!(!s.name.is_empty(), "empty scenario name");
        assert!(
            s.name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "{:?} is not a plain file stem",
            s.name
        );
        assert!(seen.insert(s.name), "duplicate scenario name {}", s.name);
        assert!(
            s.size.0 >= 800.0 && s.size.1 >= 560.0,
            "{} is below the minimum size",
            s.name
        );
    }
}

#[gpui::test]
fn steps_play_against_the_shell(cx: &mut TestAppContext) {
    let (shell, vcx) = open(cx, crate::tools::build);
    let handle = vcx.cx.windows()[0];
    for step in sample().steps {
        vcx.cx
            .update(|cx| steps::apply(step, handle, cx))
            .expect("step plays");
        vcx.run_until_parked();
    }
    assert_eq!(shell.read_with(vcx, |s, _| s.current()), ToolId::Schedule);
}

#[gpui::test]
fn click_resolves_a_tag_and_hits_the_element(cx: &mut TestAppContext) {
    let (shell, vcx) = open(cx, crate::tools::build);
    let handle = vcx.cx.windows()[0];
    vcx.cx
        .update(|cx| steps::apply(&Step::Click("tool-nav-schedule"), handle, cx))
        .expect("the nav button is tagged");
    vcx.run_until_parked();
    assert_eq!(shell.read_with(vcx, |s, _| s.current()), ToolId::Schedule);
}

#[gpui::test]
fn missing_selector_fails_the_scenario(cx: &mut TestAppContext) {
    let (_, vcx) = open(cx, crate::tools::build);
    let handle = vcx.cx.windows()[0];
    let err = vcx
        .cx
        .update(|cx| steps::apply(&Step::Click("no-such-element"), handle, cx))
        .expect_err("an unknown selector fails");
    assert!(err.contains("no-such-element"), "{err}");
}

#[test]
fn clip_rect_is_in_device_pixels_and_clamped() {
    let b = bounds(point(px(10.0), px(20.0)), size(px(30.0), px(40.0)));
    assert_eq!(device_rect(b, 1.0, (1000, 700)), (10, 20, 30, 40));
    assert_eq!(device_rect(b, 1.25, (1000, 700)), (13, 25, 37, 50));
    let off = bounds(point(px(990.0), px(690.0)), size(px(30.0), px(40.0)));
    assert_eq!(device_rect(off, 1.0, (1000, 700)), (990, 690, 10, 10));
}

#[test]
fn crop_fails_for_an_unknown_selector() {
    let image = image::RgbaImage::new(10, 10);
    let err = crop(&image, "no-such-element", 1.0).expect_err("unknown selector");
    assert!(err.contains("no-such-element"), "{err}");
}

#[cfg(windows)]
mod capture {
    use super::*;

    fn scenario(
        name: &'static str,
        clip: Option<&'static str>,
        steps: &'static [Step],
    ) -> Scenario {
        Scenario {
            name,
            size: (1000.0, 700.0),
            setup: |_| {},
            steps,
            clip,
        }
    }

    #[test]
    fn capture_writes_png() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = run_scenario(&sample(), dir.path(), "t-").expect("sample renders");
        assert_eq!(path, dir.path().join("t-shell-sample.png"));
        let (w, h) = image::image_dimensions(&path).expect("a PNG");
        assert_eq!((w, h), (1000, 700));
    }

    #[test]
    fn clip_crops_to_element_bounds() {
        let dir = tempfile::tempdir().expect("temp dir");
        let s = scenario("clip", Some("tool-nav-pipeline"), &[Step::WaitIdle]);
        let path = run_scenario(&s, dir.path(), "").expect("renders");
        let b = probe::bounds("tool-nav-pipeline").expect("tagged");
        let (_, _, w, h) = device_rect(b, 1.0, (1000, 700));
        assert_eq!(image::image_dimensions(&path).expect("a PNG"), (w, h));
    }

    #[test]
    fn unknown_clip_fails_the_scenario() {
        let dir = tempfile::tempdir().expect("temp dir");
        let s = scenario("bad-clip", Some("no-such-element"), &[]);
        let err = run_scenario(&s, dir.path(), "").expect_err("fails");
        assert!(err.contains("no-such-element"), "{err}");
        assert!(!dir.path().join("bad-clip.png").exists());
    }

    #[test]
    fn missing_click_selector_fails_the_run() {
        let dir = tempfile::tempdir().expect("temp dir");
        let s = scenario("bad-click", None, &[Step::Click("no-such-element")]);
        let err = run_scenario(&s, dir.path(), "").expect_err("fails");
        assert!(err.contains("no-such-element"), "{err}");
    }
}
