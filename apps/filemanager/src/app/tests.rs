//! App-level tests: the `init_app` route that `run` takes.

use gpui::{TestAppContext, VisualTestContext};

use crate::jobs::WorkerGate;
use crate::{init_app, open_main_window, AppSetup, MAIN_SIZE};

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

    let gate = vcx
        .update(|_, cx| WorkerGate::handle(cx))
        .expect("init_app installed the gate");
    gate.open();
    // Real worker threads answer; give them time, as the view tests do.
    for _ in 0..50 {
        vcx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(2));
        if vcx.debug_bounds("pipeline-folder-input").is_some() {
            break;
        }
    }
    vcx.run_until_parked();
    assert!(
        vcx.debug_bounds("pipeline-folder-input").is_some(),
        "loaded: body drawn"
    );
}
