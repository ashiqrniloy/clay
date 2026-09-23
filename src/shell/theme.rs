//! Clay-owned typed theme tokens for package UI and SDUI rendering.
//!
//! Package declarations may name semantic, package-prefixed tokens, but they do
//! not provide raw colors, CSS, renderer callbacks, or native style handles.
//! Clay resolves every package token through a same-typed core fallback token
//! before Masonry paint/layout reads cached native values.
//!
//! Split by concern (plan 133 task 7): `parse` owns the token/level enums,
//! the core catalog, and the package-token resolver; `validate` owns
//! design-token override validation and the WCAG contrast floors; `resolve`
//! owns the cached resolved registry, panel geometry, and the flat snapshot.
//! All three are re-exported here, so `crate::shell::theme::X` stays the
//! call-site path.

mod parse;
mod resolve;
mod validate;

pub use parse::*;
pub use resolve::*;
pub use validate::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod theme_snapshot_tests;
