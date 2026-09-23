//! The `string_enum_impl!` macro: one declaration for closed string enums.
//!
//! Ten-plus enums in this crate are "string enums": a closed variant set that
//! round-trips to fixed strings on the wire, in JSON config, or in the package
//! manifest surface. Every one of them used to hand-write the same two
//! functions — a `const fn as_str(self) -> &'static str` match and a
//! `fn parse(value: &str) -> Option<Self>` match — with the literals duplicated
//! in both directions.
//!
//! [`string_enum_impl!`] generates both from a single `Variant => "text"` list,
//! so a string can only be wrong once. The enum declaration itself (derives,
//! `serde`/`rkyv` attributes, variant docs and attributes) is deliberately left
//! alone: the macro takes the *impl*, not the type, which keeps the wire/archive
//! attributes of archivable enums untouched.
//!
//! Contract, checked by `src/str_enum/tests.rs`:
//! - `as_str` returns exactly the declared literal;
//! - `parse(as_str(v)) == Some(v)` for every variant;
//! - `parse` rejects unknown strings with `None`;
//! - the declared strings are unique.

/// Generates `as_str` + `parse` for a closed string enum.
///
/// ```text
/// string_enum_impl! {
///     #[allow(dead_code)]
///     pub(crate) BorderStyle {
///         // Comments between variants are allowed.
///         None => "none",
///         Solid => "solid",
///         Dashed => "dashed",
///     }
/// }
/// ```
///
/// expands to
///
/// ```text
/// impl BorderStyle {
///     pub(crate) const fn as_str(self) -> &'static str { /* match self */ }
///     pub(crate) fn parse(value: &str) -> Option<Self> { /* match value */ }
/// }
/// ```
///
/// `as_str` is `const fn` (callers may use it in constants); `parse` cannot be,
/// because matching on `&str` is not allowed in `const fn` (rustc 1.98).
///
/// Attributes and visibility in front of the type name are copied onto the
/// generated `impl` (`#[allow(dead_code)]` for enums only used by tests or
/// conformance surfaces). When `as_str` is exported but `parse` stays
/// crate-private (the package wire enums), write `parse_private` after the type
/// name. Add a trailing `all_as_str` to also generate
/// `const fn all_as_str() -> &'static [&'static str]` for diagnostics that list
/// the accepted values.
///
/// Every invocation also generates a `#[cfg(test)] const ALL: &'static [Self]`
/// so `src/str_enum/tests.rs` can prove the golden string list covers every
/// variant; it does not exist in non-test builds.
macro_rules! string_enum_impl {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident {
            $( $variant:ident => $text:literal, )+
        }
        all_as_str
    ) => {
        string_enum_impl!(@pair $(#[$meta])* $vis $name, $vis, { $( $variant => $text, )+ });
        string_enum_impl!(@all $(#[$meta])* $vis $name { $( $text, )+ });
    };
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident, parse_private {
            $( $variant:ident => $text:literal, )+
        }
    ) => {
        string_enum_impl!(@pair $(#[$meta])* $vis $name, , { $( $variant => $text, )+ });
    };
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident {
            $( $variant:ident => $text:literal, )+
        }
    ) => {
        string_enum_impl!(@pair $(#[$meta])* $vis $name, $vis, { $( $variant => $text, )+ });
    };
    (@pair $(#[$meta:meta])* $vis:vis $name:ident, $parse_vis:vis, {
        $( $variant:ident => $text:literal, )+
    }) => {
        $(#[$meta])*
        impl $name {
            /// The string form of this variant (wire/config/manifest surface).
            $vis const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $text, )+
                }
            }

            /// Parses the string form produced by `as_str`; unknown strings
            /// are rejected with `None`.
            $parse_vis fn parse(value: &str) -> Option<Self> {
                match value {
                    $( $text => Some(Self::$variant), )+
                    _ => None,
                }
            }

            /// Every variant, in declaration order (test surface).
            #[cfg(test)]
            $vis const ALL: &'static [Self] = &[ $( Self::$variant, )+ ];
        }
    };
    (@all $(#[$meta:meta])* $vis:vis $name:ident { $( $text:literal, )+ }) => {
        $(#[$meta])*
        impl $name {
            /// Every accepted string, in declaration order, for diagnostics.
            $vis const fn all_as_str() -> &'static [&'static str] {
                &[ $( $text, )+ ]
            }
        }
    };
}

pub(crate) use string_enum_impl;

#[cfg(test)]
mod tests;
