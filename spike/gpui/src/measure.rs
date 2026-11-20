//! Records the painted bounds of named elements, so the geometry check can
//! compare them with the reference image in a release build (GPUI's
//! `debug_selector` bounds exist only in debug builds).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::{canvas, Bounds, IntoElement, Pixels, Styled as _};

/// A box in logical pixels: x, y, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    #[must_use]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// `outset` is the border widths `[left, top, right, bottom]` to add back
    /// to a padding box to get the border box.
    fn from_bounds(b: Bounds<Pixels>, outset: [f32; 4]) -> Self {
        Self {
            x: f32::from(b.origin.x) - outset[0],
            y: f32::from(b.origin.y) - outset[1],
            w: f32::from(b.size.width) + outset[0] + outset[2],
            h: f32::from(b.size.height) + outset[1] + outset[3],
        }
    }
}

/// Shared store of measured boxes, filled during prepaint.
#[derive(Clone, Default)]
pub struct Boxes(Rc<RefCell<BTreeMap<&'static str, Rect>>>);

impl Boxes {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<Rect> {
        self.0.borrow().get(name).copied()
    }

    #[must_use]
    pub fn all(&self) -> BTreeMap<&'static str, Rect> {
        self.0.borrow().clone()
    }

    /// A zero-cost child to add to a `.relative()` element: it records that
    /// element's padding box, grown by the `border` widths
    /// `[left, top, right, bottom]` (px) to give the border box.
    #[must_use]
    pub fn probe(&self, name: &'static str, border: [f32; 4]) -> impl IntoElement {
        let store = self.0.clone();
        canvas(
            move |bounds, _, _| {
                store
                    .borrow_mut()
                    .insert(name, Rect::from_bounds(bounds, border));
            },
            |_, (), _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full()
    }
}
