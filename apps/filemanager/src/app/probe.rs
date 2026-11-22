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
//! A tag keeps the bounds of its latest paint. An element that stops being
//! drawn keeps its last bounds until [`reset`] (the harness calls it once per
//! scenario).

use gpui::{IntoElement, SharedString};

#[cfg(any(test, feature = "shoot"))]
mod imp {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use gpui::{
        A11ySubtreeBuilder, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
        IntoElement, LayoutId, Pixels, SharedString, Window,
    };

    thread_local! {
        static BOUNDS: RefCell<HashMap<SharedString, Bounds<Pixels>>> = RefCell::default();
    }

    pub fn reset() {
        BOUNDS.with(|b| b.borrow_mut().clear());
    }

    pub fn bounds(name: &str) -> Option<Bounds<Pixels>> {
        BOUNDS.with(|b| b.borrow().get(name).copied())
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
            BOUNDS.with(|b| b.borrow_mut().insert(self.name.clone(), bounds));
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

/// The bounds `name` had when last painted (window coordinates, logical px).
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
