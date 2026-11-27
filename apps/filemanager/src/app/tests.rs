//! App-level tests: the `init_app` route that `run` takes.

use std::time::{Duration, Instant};

use gpui::{TestAppContext, VisualTestContext};

use crate::jobs::WorkerGate;
use crate::{init_app, open_main_window, AppSetup, MAIN_SIZE};

/// Asserts that the page's content ends at the header's bottom edge: the page
/// is exactly the header plus its vertical padding, within 1 px. Any extra
/// child (a visible "Loading" line, a body) makes the page taller.
pub(crate) fn assert_header_only(
    cx: &mut VisualTestContext,
    page: &'static str,
    header: &'static str,
) {
    let page_box = cx.debug_bounds(page).unwrap_or_else(|| panic!("no {page}"));
    let header_box = cx
        .debug_bounds(header)
        .unwrap_or_else(|| panic!("no {header}"));
    let pad = gpui::px(crate::theme::tokens::SPACE_4);
    let expected = header_box.size.height + pad + pad;
    let diff = (page_box.size.height - expected).abs();
    assert!(
        diff <= gpui::px(1.0),
        "{page} is {:?} tall, header plus padding is {expected:?}: something besides the header is drawn",
        page_box.size.height
    );
}

#[gpui::test]
fn init_app_with_closed_gate_shows_loading(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    let window = cx.update(|cx| {
        init_app(cx, AppSetup::for_tests().with_closed_worker_gate());
        open_main_window(cx, MAIN_SIZE).expect("window opens")
    });
    let vcx = VisualTestContext::from_window(window.into(), cx).into_mut();
    vcx.run_until_parked();

    // The Pipeline is the first tool, so it is the one shown. Its model
    // reports loading exactly when the view draws the header and no body.
    assert!(vcx.debug_bounds("pipeline-header").is_some(), "header");
    assert!(
        vcx.debug_bounds("pipeline-folder-input").is_none(),
        "loading: no body"
    );
    assert_header_only(vcx, "pipeline-page", "pipeline-header");

    let gate = vcx
        .update(|_, cx| WorkerGate::handle(cx))
        .expect("init_app installed the gate");
    gate.open();
    // Real worker threads answer; poll until a generous deadline.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        vcx.run_until_parked();
        if vcx.debug_bounds("pipeline-folder-input").is_some() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "loaded: the Pipeline body was not drawn within 5 s of opening the gate"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
