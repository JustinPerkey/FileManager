//! The shared controls every tool view builds from, transcribed from the React
//! components and `controls.css` onto gpui-component.
//!
//! - [`text`]: `h1`, `h2`, `body`, `muted`, `small`, `small_muted`, `mono`,
//!   `mono_small_muted`, `intro`, `tabular`
//! - [`icon`]: [`IconName`], `icon`
//! - [`button`]: `button` -> [`FmButton`] (secondary, primary, quiet, danger;
//!   icon, icon-only, disabled, busy, shortcut)
//! - [`input`]: `text_input`, `text_area` -> [`FmInput`]
//! - [`checkbox`]: [`CheckState`], `checkbox` (with the "Not set" state)
//! - [`select`]: `select` -> [`FmSelect`]
//! - [`tag`](mod@tag): `tag`
//! - [`banner`]: [`Banner`], [`Tone`]
//! - [`disclosure`](mod@disclosure): [`Disclosure`]
//! - [`status`]: [`StatusTone`], `status`
//! - [`step_number`](mod@step_number): `step_number`
//! - [`skeleton`](mod@skeleton): `skeleton_bar`
//! - [`middle_path`]: `middle_truncate`, [`MiddlePath`]
//! - [`kbd`]: `kbd`, `chord`, `alternatives`
//!
//! The rule: no colour literal anywhere here (colours come from
//! [`crate::theme::palette`]), and sizes come from [`crate::theme::tokens`],
//! except the fixed control sizes `controls.css` writes as pixels (24, 30 and
//! 32 px).

pub mod banner;
pub mod button;
pub mod checkbox;
pub mod disclosure;
pub mod icon;
pub mod input;
pub mod kbd;
pub mod middle_path;
pub mod select;
pub mod skeleton;
pub mod status;
pub mod step_number;
pub mod tag;
pub mod text;

#[cfg(test)]
mod tests;

pub use banner::{Banner, Tone};
pub use button::{button, FmButton};
pub use checkbox::{checkbox, CheckState, FmCheckbox};
pub use disclosure::Disclosure;
pub use icon::{icon, IconName};
pub use input::{text_area, text_input, FmInput};
pub use kbd::{alternatives, chord, kbd};
pub use middle_path::{middle_truncate, MiddlePath};
pub use select::{select, FmSelect};
pub use skeleton::skeleton_bar;
pub use status::{status, StatusTone};
pub use step_number::step_number;
pub use tag::tag;
pub use text::{body, h1, h2, intro, mono, mono_small_muted, muted, small, small_muted, tabular};
