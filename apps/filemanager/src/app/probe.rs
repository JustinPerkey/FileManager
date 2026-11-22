//! Element tags: a name for an element that the screenshot harness (and tests)
//! can resolve to the element's bounds in a real window.
//!
//! GPUI's `debug_selector` records bounds only where a test context can read
//! them; a real window has no public way to. [`Tag::tag`] records the same
//! bounds into a registry the harness reads. It is a pass-through (no wrapper
//! element, no recording) unless the crate is built for tests or with the
//! `shoot` feature, so the release exe is unaffected. Views that a scenario
//! drives or clips to call `.tag("name")` on the element, next to any
//! `debug_selector`.
//!
//! Rules for a tagged element:
//!
//! - `.tag("name")` is the **last** call in the builder chain (outermost), so
//!   the recorded bounds are the element's final bounds, not an inner box.
//! - A tag resolves only if its element was laid out in the **latest frame**.
//!   A frame starts when [`begin_frame`] runs, which the `Shell` does once per
//!   window draw before any of its children are laid out. An element that left
//!   the tree no longer resolves (`bounds` is `None`), so the harness reports
//!   "not found" instead of using stale coordinates. A view GPUI replays from
//!   its cache without redrawing records nothing for that frame; tagged
//!   elements must not sit inside a `cached` view. [`reset`] forgets
//!   everything (the harness calls it once per scenario).
//! - `debug_selector` is for `VisualTestContext` tests only; the harness cannot
//!   read it in a real window. Tag what a scenario drives.

use gpui::{IntoElement, SharedString};

#[cfg(any(test, feature = "shoot"))]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;

    use gpui::{
        A11ySubtreeBuilder, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
        IntoElement, LayoutId, Pixels, SharedString, Window,
    };

    thread_local! {
        /// Name -> (frame it was recorded in, bounds).
        static BOUNDS: RefCell<HashMap<SharedString, (u64, Bounds<Pixels>)>> = RefCell::default();
        static FRAME: Cell<u64> = const { Cell::new(0) };
    }

    pub fn reset() {
        BOUNDS.with(|b| b.borrow_mut().clear());
    }

    pub fn begin_frame() {
        FRAME.with(|f| f.set(f.get() + 1));
    }

    pub fn bounds(name: &str) -> Option<Bounds<Pixels>> {
        let frame = FRAME.with(Cell::get);
        BOUNDS.with(|b| {
            b.borrow()
                .get(name)
                .filter(|(stamp, _)| *stamp == frame)
                .map(|(_, bounds)| *bounds)
        })
    }

    /// The element `tag` returns: draws as the inner element and records its
    /// bounds.
    pub struct Tagged<E> {
        name: SharedString,
        inner: E,
    }

    impl<E> Tagged<E> {
        pub(super) fn new(name: SharedString, inner: E) -> Self {
            Self { name, inner }
        }
    }

    impl<E: Element> IntoElement for Tagged<E> {
        type Element = Self;

        fn into_element(self) -> Self {
            self
        }
    }

    impl<E: Element> Element for Tagged<E> {
        type RequestLayoutState = E::RequestLayoutState;
        type PrepaintState = E::PrepaintState;

        fn id(&self) -> Option<ElementId> {
            self.inner.id()
        }

        fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
            self.inner.source_location()
        }

        fn request_layout(
            &mut self,
            id: Option<&GlobalElementId>,
            inspector_id: Option<&InspectorElementId>,
            window: &mut Window,
            cx: &mut App,
        ) -> (LayoutId, Self::RequestLayoutState) {
            self.inner.request_layout(id, inspector_id, window, cx)
        }

        fn prepaint(
            &mut self,
            id: Option<&GlobalElementId>,
            inspector_id: Option<&InspectorElementId>,
            bounds: Bounds<Pixels>,
            request_layout: &mut Self::RequestLayoutState,
            window: &mut Window,
            cx: &mut App,
        ) -> Self::PrepaintState {
            let frame = FRAME.with(Cell::get);
            BOUNDS.with(|b| b.borrow_mut().insert(self.name.clone(), (frame, bounds)));
            self.inner
                .prepaint(id, inspector_id, bounds, request_layout, window, cx)
        }

        fn paint(
            &mut self,
            id: Option<&GlobalElementId>,
            inspector_id: Option<&InspectorElementId>,
            bounds: Bounds<Pixels>,
            request_layout: &mut Self::RequestLayoutState,
            prepaint: &mut Self::PrepaintState,
            window: &mut Window,
            cx: &mut App,
        ) {
            self.inner.paint(
                id,
                inspector_id,
                bounds,
                request_layout,
                prepaint,
                window,
                cx,
            );
        }

        fn a11y_role(&self) -> Option<gpui::accesskit::Role> {
            self.inner.a11y_role()
        }

        fn write_a11y_info(&self, node: &mut gpui::accesskit::Node) {
            self.inner.write_a11y_info(node);
        }

        fn a11y_synthetic_children(
            &mut self,
            prepaint: &mut Self::PrepaintState,
            builder: &mut A11ySubtreeBuilder,
        ) {
            self.inner.a11y_synthetic_children(prepaint, builder);
        }
    }
}

/// The element [`Tag::tag`] returns.
#[cfg(any(test, feature = "shoot"))]
pub use imp::Tagged;

/// Forgets every recorded bound.
#[cfg(any(test, feature = "shoot"))]
pub use imp::reset;

/// Starts a frame: tags recorded before this call no longer resolve unless
/// drawn again. A no-op outside test and `shoot` builds.
#[cfg(any(test, feature = "shoot"))]
pub use imp::begin_frame;

/// Starts a frame (no-op: tags are recorded only in test and harness builds).
#[cfg(not(any(test, feature = "shoot")))]
pub fn begin_frame() {}

/// The bounds `name` had in the latest frame (window coordinates, logical px),
/// or `None` if it was not drawn in it.
#[cfg(any(test, feature = "shoot"))]
#[must_use]
pub fn bounds(name: &str) -> Option<gpui::Bounds<gpui::Pixels>> {
    imp::bounds(name)
}

/// Names an element for the harness. See the module documentation.
pub trait Tag: IntoElement {
    /// The element, recorded under `name` when the crate is built for tests or
    /// the harness; unchanged otherwise.
    #[cfg(any(test, feature = "shoot"))]
    fn tag(self, name: impl Into<SharedString>) -> Tagged<Self::Element> {
        Tagged::new(name.into(), self.into_element())
    }

    /// The element, unchanged (tags are recorded only in test and harness
    /// builds).
    #[cfg(not(any(test, feature = "shoot")))]
    fn tag(self, _name: impl Into<SharedString>) -> Self::Element {
        self.into_element()
    }
}

impl<T: IntoElement> Tag for T {}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        canvas, div, point, px, size, Bounds, Context, IntoElement, ParentElement as _, Render,
        Styled as _, TestAppContext, VisualTestContext, Window,
    };

    use super::*;

    /// Draws a tagged box at `x` when `shown`; always marks the frame start.
    struct Frame {
        shown: Rc<Cell<bool>>,
        x: Rc<Cell<f32>>,
    }

    impl Render for Frame {
        fn render(&mut self, _w: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let tracker = canvas(|_, _, _| begin_frame(), |_, (), _, _| ())
                .absolute()
                .size_full();
            let root = div().size_full().child(tracker);
            if self.shown.get() {
                root.child(
                    div()
                        .absolute()
                        .left(px(self.x.get()))
                        .top(px(5.0))
                        .w(px(30.0))
                        .h(px(20.0))
                        .tag("box"),
                )
            } else {
                root
            }
        }
    }

    fn open(cx: &mut TestAppContext) -> (Rc<Cell<bool>>, Rc<Cell<f32>>, &mut VisualTestContext) {
        reset();
        let shown = Rc::new(Cell::new(true));
        let x = Rc::new(Cell::new(10.0));
        let (s, xx) = (shown.clone(), x.clone());
        let window = cx.add_window(move |_, _| Frame { shown: s, x: xx });
        let vcx = VisualTestContext::from_window(window.into(), cx).into_mut();
        vcx.run_until_parked();
        (shown, x, vcx)
    }

    #[gpui::test]
    fn tag_not_drawn_in_latest_frame_does_not_resolve(cx: &mut TestAppContext) {
        let (shown, _, vcx) = open(cx);
        assert_eq!(
            bounds("box"),
            Some(Bounds::new(
                point(px(10.0), px(5.0)),
                size(px(30.0), px(20.0))
            ))
        );
        shown.set(false);
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
        assert_eq!(bounds("box"), None);
    }

    #[gpui::test]
    fn tag_drawn_again_resolves_to_new_bounds(cx: &mut TestAppContext) {
        let (shown, x, vcx) = open(cx);
        shown.set(false);
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
        assert_eq!(bounds("box"), None);
        shown.set(true);
        x.set(50.0);
        vcx.update(|window, _| window.refresh());
        vcx.run_until_parked();
        assert_eq!(
            bounds("box"),
            Some(Bounds::new(
                point(px(50.0), px(5.0)),
                size(px(30.0), px(20.0))
            ))
        );
    }
}
