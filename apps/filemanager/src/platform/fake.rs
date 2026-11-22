//! A scripted [`Platform`] for tests and screenshots. It never touches the
//! filesystem or the OS.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{App, Task};

use super::{OpenError, OpenSpec, PickResult, Platform, SaveSpec};

/// One call a model made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformCall {
    Pick(OpenSpec),
    PickSave(SaveSpec),
    OpenWithSystem(PathBuf),
    Reveal(PathBuf),
    CopyText(String),
}

#[derive(Default)]
struct Script {
    picks: VecDeque<PickResult>,
    saves: VecDeque<PickResult>,
    opens: VecDeque<Result<(), OpenError>>,
    calls: Vec<PlatformCall>,
}

/// Answers dialogs from a script, in order, and logs every call. A dialog
/// with no scripted answer resolves to `Ok(None)` (cancelled); an open or
/// reveal with none resolves to `Ok(())`. Clones share
/// the same script and log.
#[derive(Clone, Default)]
pub struct FakePlatform {
    inner: Rc<RefCell<Script>>,
}

impl FakePlatform {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues the answer to the next open dialog.
    pub fn answer_pick(&self, answer: PickResult) {
        self.inner.borrow_mut().picks.push_back(answer);
    }

    /// Queues the answer to the next save dialog.
    pub fn answer_save(&self, answer: PickResult) {
        self.inner.borrow_mut().saves.push_back(answer);
    }

    /// Queues the answer to the next open or reveal (they share one queue, in
    /// call order). The default is `Ok(())`.
    pub fn answer_open(&self, answer: Result<(), OpenError>) {
        self.inner.borrow_mut().opens.push_back(answer);
    }

    /// Every call so far, in order.
    #[must_use]
    pub fn calls(&self) -> Vec<PlatformCall> {
        self.inner.borrow().calls.clone()
    }
}

impl Platform for FakePlatform {
    fn pick(&self, spec: OpenSpec, _cx: &mut App) -> Task<PickResult> {
        let mut s = self.inner.borrow_mut();
        s.calls.push(PlatformCall::Pick(spec));
        Task::ready(s.picks.pop_front().unwrap_or(Ok(None)))
    }

    fn pick_save(&self, spec: SaveSpec, _cx: &mut App) -> Task<PickResult> {
        let mut s = self.inner.borrow_mut();
        s.calls.push(PlatformCall::PickSave(spec));
        Task::ready(s.saves.pop_front().unwrap_or(Ok(None)))
    }

    fn open_with_system(&self, path: &Path, _cx: &mut App) -> Task<Result<(), OpenError>> {
        let mut s = self.inner.borrow_mut();
        s.calls
            .push(PlatformCall::OpenWithSystem(path.to_path_buf()));
        Task::ready(s.opens.pop_front().unwrap_or(Ok(())))
    }

    fn reveal(&self, path: &Path, _cx: &mut App) -> Task<Result<(), OpenError>> {
        let mut s = self.inner.borrow_mut();
        s.calls.push(PlatformCall::Reveal(path.to_path_buf()));
        Task::ready(s.opens.pop_front().unwrap_or(Ok(())))
    }

    fn copy_text(&self, text: &str, _cx: &mut App) {
        let call = PlatformCall::CopyText(text.to_string());
        self.inner.borrow_mut().calls.push(call);
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use gpui::TestAppContext;

    use super::*;
    use crate::platform::{FileFilter, PickError, PickKind};

    fn open_spec() -> OpenSpec {
        OpenSpec {
            kind: PickKind::File,
            filters: vec![FileFilter {
                name: "TOML".into(),
                patterns: vec!["*.toml".into()],
            }],
            start_dir: Some(PathBuf::from("start")),
            title: Some("Open".into()),
        }
    }

    fn save_spec() -> SaveSpec {
        SaveSpec {
            filters: vec![FileFilter {
                name: "Archive".into(),
                patterns: vec!["*.tar.gz".into()],
            }],
            default_extension: Some("tar.gz".into()),
            start_dir: Some(PathBuf::from("out")),
            suggested_name: Some(OsString::from("a.tar.gz")),
            title: None,
        }
    }

    #[gpui::test]
    async fn answers_in_order_and_logs_calls(cx: &mut TestAppContext) {
        let fake = FakePlatform::new();
        fake.answer_pick(Ok(Some(PathBuf::from("one"))));
        fake.answer_pick(Err(PickError::Failed { detail: "x".into() }));
        fake.answer_save(Ok(Some(PathBuf::from("saved"))));
        let first = cx.update(|cx| fake.pick(open_spec(), cx)).await;
        let second = cx.update(|cx| fake.pick(open_spec(), cx)).await;
        let saved = cx.update(|cx| fake.pick_save(save_spec(), cx)).await;
        assert_eq!(first, Ok(Some(PathBuf::from("one"))));
        assert_eq!(second, Err(PickError::Failed { detail: "x".into() }));
        assert_eq!(saved, Ok(Some(PathBuf::from("saved"))));
        let opened = cx
            .update(|cx| fake.open_with_system(Path::new("o"), cx))
            .await;
        let revealed = cx.update(|cx| fake.reveal(Path::new("r"), cx)).await;
        assert_eq!((opened, revealed), (Ok(()), Ok(())), "the default");
        cx.update(|cx| fake.copy_text("t", cx));
        assert_eq!(
            fake.calls(),
            [
                PlatformCall::Pick(open_spec()),
                PlatformCall::Pick(open_spec()),
                PlatformCall::PickSave(save_spec()),
                PlatformCall::OpenWithSystem("o".into()),
                PlatformCall::Reveal("r".into()),
                PlatformCall::CopyText("t".into()),
            ]
        );
    }

    #[gpui::test]
    async fn scripted_open_and_reveal_answers_are_returned_in_order(cx: &mut TestAppContext) {
        let fake = FakePlatform::new();
        let err = OpenError {
            detail: "no handler".into(),
        };
        fake.answer_open(Err(err.clone()));
        let first = cx
            .update(|cx| fake.open_with_system(Path::new("o"), cx))
            .await;
        let second = cx.update(|cx| fake.reveal(Path::new("r"), cx)).await;
        assert_eq!(first, Err(err));
        assert_eq!(second, Ok(()));
        assert_eq!(fake.calls().len(), 2, "failures are still logged");
    }

    #[gpui::test]
    async fn unscripted_pick_is_cancelled(cx: &mut TestAppContext) {
        let fake = FakePlatform::new();
        assert_eq!(cx.update(|cx| fake.pick(open_spec(), cx)).await, Ok(None));
        assert_eq!(
            cx.update(|cx| fake.pick_save(save_spec(), cx)).await,
            Ok(None)
        );
    }

    #[gpui::test]
    fn records_specs(cx: &mut TestAppContext) {
        let fake = FakePlatform::new();
        drop(cx.update(|cx| fake.pick(open_spec(), cx)));
        drop(cx.update(|cx| fake.pick_save(save_spec(), cx)));
        let calls = fake.calls();
        let PlatformCall::Pick(open) = &calls[0] else {
            panic!("first call is a pick");
        };
        assert_eq!(open.filters[0].patterns, ["*.toml"]);
        assert_eq!(open.start_dir.as_deref(), Some(Path::new("start")));
        let PlatformCall::PickSave(save) = &calls[1] else {
            panic!("second call is a save");
        };
        assert_eq!(save.default_extension.as_deref(), Some("tar.gz"));
        assert_eq!(save.suggested_name, Some(OsString::from("a.tar.gz")));
    }
}
