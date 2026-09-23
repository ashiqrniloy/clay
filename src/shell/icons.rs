//! Bounded semantic icon model (Plan 112).
//!
//! One generic, versioned shape for icon-pack geometry and semantic icon
//! references. Packages ship bounded path data; hosts own rendering, color,
//! action routing, and accessibility. No raw SVG/XML, CSS, URLs, scripts, or
//! renderer callbacks exist anywhere in this model — validation rejects them
//! structurally before any allocation-heavy work.

use crate::perf::budgets::{ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES, ICON_PACK_PAYLOAD_BUDGET_BYTES};
use serde::{Deserialize, Serialize};

/// Current icon geometry schema version. Unknown versions are rejected.
pub const ICON_SCHEMA_VERSION: u32 = 1;
/// Maximum paths per glyph (duotone uses a background layer plus foreground).
pub const MAX_ICON_PATHS: usize = 8;
/// Maximum path commands per glyph (bounded before expansion).
pub const MAX_PATH_COMMANDS: usize = 512;
/// Maximum glyphs per icon pack (the required core set is 21).
pub const MAX_ICONS_PER_PACK: usize = 64;
/// ViewBox width/height bounds.
pub const MIN_VIEWBOX_SIDE: f32 = 1.0;
pub const MAX_VIEWBOX_SIDE: f32 = 512.0;
/// Coordinate magnitude bound (viewBox space).
pub const MAX_ICON_COORDINATE: f32 = 4096.0;
/// Maximum semantic reference/key length.
pub const MAX_ICON_REFERENCE_LEN: usize = 128;

/// Core semantic keys reserved to first-party packs (Plan 112 task 2 review).
/// Every key is verified to exist in both Phosphor Regular and Duotone assets
/// at the pinned upstream release.
pub const CORE_ICON_KEYS: [&str; 22] = [
    "action.close",
    "action.new",
    "control-center.open",
    "document.save",
    "document.reload",
    "document.open",
    "message.send",
    "generation.stop",
    "navigation.back",
    "navigation.up",
    "disclosure.right",
    "disclosure.down",
    "file.folder",
    "file.file",
    "file.symlink",
    "session.resume",
    "session.search",
    "git.branch",
    "status.success",
    "status.warning",
    "status.error",
    "preview.toggle",
];

/// Whether `key` is one of the reserved core semantic keys.
pub fn is_core_icon_key(key: &str) -> bool {
    CORE_ICON_KEYS.contains(&key)
}

/// One bounded glyph: viewBox plus flat, absolute path commands.
///
/// Validation (`validate_icon_geometry` / `parse_icon_path`) rejects NaN and
/// infinities, so `PartialEq` is an equivalence relation on any existing
/// value and `Eq` is sound by invariant.
///
/// Wire shape (serde) is the canonical contract shape `{ viewBox, paths:
/// [{ d, opacity? }] }` — the same bounded d-string form packages declare in
/// manifests; parsed commands are an in-memory detail and never serialized.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct IconGeometry {
    /// `[x, y, width, height]`; width/height in [`MIN_VIEWBOX_SIDE`, `MAX_VIEWBOX_SIDE`].
    pub view_box: [f32; 4],
    /// 1..=`MAX_ICON_PATHS` paths; non-empty geometry required.
    pub paths: Vec<IconPath>,
}

/// One path layer with optional constant opacity (duotone shade layers).
#[derive(Debug, Clone, PartialEq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct IconPath {
    /// 1..=`MAX_PATH_COMMANDS` absolute commands.
    pub commands: Vec<IconPathCommand>,
    /// Finite opacity in [0, 1]; `None` renders fully opaque.
    pub opacity: Option<f32>,
}

/// Absolute path commands only. Relative forms and implicit repetition are
/// normalized into absolute coordinates at parse time; runtime never re-parses.
#[derive(Debug, Clone, Copy, PartialEq, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub enum IconPathCommand {
    MoveTo([f32; 2]),
    LineTo([f32; 2]),
    CurveTo([f32; 6]),
    QuadTo([f32; 4]),
    /// rx, ry, x-axis-rotation, large-arc-flag, sweep-flag, x, y (absolute).
    ArcTo([f32; 7]),
    HorizontalTo(f32),
    VerticalTo(f32),
    ClosePath,
}

impl IconPath {
    /// Bounded d-string emission (the wire/manifest form). Absolute commands
    /// only, canonical `cmd x,y` spacing; output re-parses losslessly through
    /// [`parse_icon_path`].
    pub fn to_d(&self) -> String {
        let mut out = String::new();
        for command in &self.commands {
            match *command {
                IconPathCommand::MoveTo(p) => {
                    out.push_str("M ");
                    push_point(&mut out, p);
                }
                IconPathCommand::LineTo(p) => {
                    out.push_str("L ");
                    push_point(&mut out, p);
                }
                IconPathCommand::CurveTo(p) => {
                    out.push_str("C ");
                    push_point(&mut out, [p[0], p[1]]);
                    out.push(' ');
                    push_point(&mut out, [p[2], p[3]]);
                    out.push(' ');
                    push_point(&mut out, [p[4], p[5]]);
                }
                IconPathCommand::QuadTo(p) => {
                    out.push_str("Q ");
                    push_point(&mut out, [p[0], p[1]]);
                    out.push(' ');
                    push_point(&mut out, [p[2], p[3]]);
                }
                IconPathCommand::ArcTo(p) => {
                    out.push_str("A ");
                    push_f32(&mut out, p[0]);
                    out.push(',');
                    push_f32(&mut out, p[1]);
                    out.push(',');
                    push_f32(&mut out, p[2]);
                    out.push(',');
                    // Arc flags are 0/1; format as integers.
                    push_f32(&mut out, p[3]);
                    out.push(',');
                    push_f32(&mut out, p[4]);
                    out.push(',');
                    push_point(&mut out, [p[5], p[6]]);
                }
                IconPathCommand::HorizontalTo(x) => {
                    out.push_str("H ");
                    push_f32(&mut out, x);
                }
                IconPathCommand::VerticalTo(y) => {
                    out.push_str("V ");
                    push_f32(&mut out, y);
                }
                IconPathCommand::ClosePath => out.push('Z'),
            }
            out.push(' ');
        }
        out.trim_end().to_string()
    }

    /// Approximate wire payload of this path's d-string in bytes.
    fn d_len(&self) -> usize {
        self.to_d().len()
    }
}

fn push_point(out: &mut String, point: [f32; 2]) {
    push_f32(out, point[0]);
    out.push(',');
    push_f32(out, point[1]);
}

fn push_f32(out: &mut String, value: f32) {
    // Display prints the shortest representation that round-trips.
    out.push_str(&value.to_string());
}

impl serde::Serialize for IconPath {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let with_opacity = self.opacity.is_some();
        let mut state =
            serializer.serialize_struct("IconPath", if with_opacity { 2 } else { 1 })?;
        state.serialize_field("d", &self.to_d())?;
        if let Some(opacity) = self.opacity {
            state.serialize_field("opacity", &opacity)?;
        }
        state.end()
    }
}

impl<'de> serde::Deserialize<'de> for IconPath {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct RawIconPath {
            d: String,
            opacity: Option<f32>,
        }
        let raw = RawIconPath::deserialize(deserializer)?;
        let commands = parse_icon_path(&raw.d).map_err(serde::de::Error::custom)?;
        Ok(Self {
            commands,
            opacity: raw.opacity,
        })
    }
}

impl IconPathCommand {
    /// Terminal point after this command, given the current point.
    fn endpoint(&self, current: (f32, f32)) -> (f32, f32) {
        match *self {
            Self::MoveTo(p) | Self::LineTo(p) => (p[0], p[1]),
            Self::CurveTo(p) => (p[4], p[5]),
            Self::QuadTo(p) => (p[2], p[3]),
            Self::ArcTo(p) => (p[5], p[6]),
            Self::HorizontalTo(x) => (x, current.1),
            Self::VerticalTo(y) => (current.0, y),
            Self::ClosePath => current,
        }
    }
}

/// Structural rejection with a bounded, deterministic message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconValidationError {
    pub field: String,
    pub message: String,
}

impl IconValidationError {
    fn new(field: &str, message: impl Into<String>) -> Self {
        Self {
            field: field.to_string(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for IconValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl Eq for IconGeometry {}
impl Eq for IconPath {}

fn check_finite(value: f32, field: &str) -> Result<(), IconValidationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(IconValidationError::new(
            field,
            "path coordinates must be finite numbers",
        ))
    }
}

fn check_coordinate(value: f32, field: &str) -> Result<(), IconValidationError> {
    check_finite(value, field)?;
    if value.abs() <= MAX_ICON_COORDINATE {
        Ok(())
    } else {
        Err(IconValidationError::new(
            field,
            format!("path coordinates must be within ±{MAX_ICON_COORDINATE} (got {value})"),
        ))
    }
}

/// Exact test for an SVG arc flag (large-arc / sweep), which must be the
/// literal `0` or `1`. The parser accepts only the exactly representable `f32`
/// values `0.0`/`1.0`, so identity — not an epsilon — is correct; `abs()` keeps
/// `-0.0` accepted and NaN rejected (clippy `float_cmp` bans the `==` form).
fn is_arc_flag(value: f32) -> bool {
    let bits = value.abs().to_bits();
    bits == 0.0_f32.to_bits() || bits == 1.0_f32.to_bits()
}

/// Validate a fully constructed geometry (used after deserialization and
/// before transport; parse paths apply the same bounds inline).
pub fn validate_icon_geometry(geometry: &IconGeometry) -> Result<(), IconValidationError> {
    let [x, y, width, height] = geometry.view_box;
    for (index, value) in geometry.view_box.iter().enumerate() {
        check_finite(*value, &format!("viewBox[{index}]"))?;
        check_coordinate(*value, &format!("viewBox[{index}]"))?;
    }
    if !(MIN_VIEWBOX_SIDE..=MAX_VIEWBOX_SIDE).contains(&width) {
        return Err(IconValidationError::new(
            "viewBox.width",
            format!(
                "viewBox width must be within [{MIN_VIEWBOX_SIDE}, {MAX_VIEWBOX_SIDE}] (got {width})"
            ),
        ));
    }
    if !(MIN_VIEWBOX_SIDE..=MAX_VIEWBOX_SIDE).contains(&height) {
        return Err(IconValidationError::new(
            "viewBox.height",
            format!(
                "viewBox height must be within [{MIN_VIEWBOX_SIDE}, {MAX_VIEWBOX_SIDE}] (got {height})"
            ),
        ));
    }
    let _ = x;
    let _ = y;
    if geometry.paths.is_empty() || geometry.paths.len() > MAX_ICON_PATHS {
        return Err(IconValidationError::new(
            "paths",
            format!(
                "icon geometry requires 1..={MAX_ICON_PATHS} paths (got {})",
                geometry.paths.len()
            ),
        ));
    }
    for path in &geometry.paths {
        if path.commands.is_empty() || path.commands.len() > MAX_PATH_COMMANDS {
            return Err(IconValidationError::new(
                "paths.commands",
                format!(
                    "icon paths require 1..={MAX_PATH_COMMANDS} commands (got {})",
                    path.commands.len()
                ),
            ));
        }
        // SVG structural invariant also enforced at parse time (moveto-first
        // rule); re-checked here so directly constructed geometries and
        // wire-bound snapshots cannot bypass it.
        if !matches!(path.commands.first(), Some(IconPathCommand::MoveTo(_))) {
            return Err(IconValidationError::new(
                "paths.commands",
                "path data must begin with a moveto command",
            ));
        }
        if let Some(opacity) = path.opacity {
            check_finite(opacity, "paths.opacity")?;
            if !(0.0..=1.0).contains(&opacity) {
                return Err(IconValidationError::new(
                    "paths.opacity",
                    format!("path opacity must be within [0, 1] (got {opacity})"),
                ));
            }
        }
        // Re-check every coordinate: geometry may be constructed directly
        // (deserialization/transport boundary), bypassing the d-string parser.
        for command in &path.commands {
            let coordinates: &[f32] = match command {
                IconPathCommand::MoveTo(p) | IconPathCommand::LineTo(p) => p,
                IconPathCommand::CurveTo(p) => p,
                IconPathCommand::QuadTo(p) => p,
                IconPathCommand::ArcTo(p) => p,
                IconPathCommand::HorizontalTo(x) => std::slice::from_ref(x),
                IconPathCommand::VerticalTo(y) => std::slice::from_ref(y),
                IconPathCommand::ClosePath => &[],
            };
            for (index, value) in coordinates.iter().enumerate() {
                check_finite(*value, &format!("paths.commands[{index}]"))?;
                check_coordinate(*value, &format!("paths.commands[{index}]"))?;
            }
            if let IconPathCommand::ArcTo(p) = command {
                if p[0] < 0.0 || p[1] < 0.0 {
                    return Err(IconValidationError::new(
                        "paths.commands",
                        "arc radii must be non-negative",
                    ));
                }
                if !is_arc_flag(p[3]) {
                    return Err(IconValidationError::new(
                        "paths.commands",
                        "arc large-arc flag must be 0 or 1",
                    ));
                }
                if !is_arc_flag(p[4]) {
                    return Err(IconValidationError::new(
                        "paths.commands",
                        "arc sweep flag must be 0 or 1",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Validate a semantic icon reference on a UI node or component declaration.
///
/// Core keys (for example `action.close`) are always allowed. Package-owned
/// keys must use the declaring package's apiPrefix namespace
/// (`<apiPrefix>.<key>`); any other namespace, the reserved `clay.` prefix,
/// or prototype-sensitive shapes are rejected.
pub fn validate_icon_reference(
    reference: &str,
    api_prefix: &str,
) -> Result<(), IconValidationError> {
    check_reference_shape(reference)?;
    if is_core_icon_key(reference) {
        return Ok(());
    }
    let prefix = format!("{api_prefix}.");
    if reference.starts_with(&prefix) && reference.len() > prefix.len() {
        return Ok(());
    }
    Err(IconValidationError::new(
        "icon",
        format!(
            "semantic icon reference `{reference}` must be a core key or use the `{prefix}` namespace"
        ),
    ))
}

/// Validate a runtime-SDUI icon reference: core keys only. Runtime tree
/// builders carry no declaring-package identity, so package-owned keys are
/// unavailable there (package component declarations validate their own
/// namespaces at record time instead).
pub fn validate_core_icon_reference(reference: &str) -> Result<(), IconValidationError> {
    check_reference_shape(reference)?;
    if is_core_icon_key(reference) {
        Ok(())
    } else {
        Err(IconValidationError::new(
            "icon",
            format!("runtime SDUI icon references must use core semantic keys (got `{reference}`)"),
        ))
    }
}

/// Validate an icon-pack declaration key. Bare core keys are reserved to
/// first-party packages (`@clay/` names; activation additionally requires
/// compiled-inventory provenance). Other packages must use their own
/// apiPrefix namespace.
pub fn validate_icon_pack_key(
    key: &str,
    api_prefix: &str,
    first_party: bool,
) -> Result<(), IconValidationError> {
    check_reference_shape(key)?;
    if is_core_icon_key(key) {
        if first_party {
            Ok(())
        } else {
            Err(IconValidationError::new(
                "key",
                format!(
                    "core semantic key `{key}` is reserved to first-party icon packs; declare `{api_prefix}.` keys instead"
                ),
            ))
        }
    } else {
        validate_icon_reference(key, api_prefix)
    }
}

fn check_reference_shape(reference: &str) -> Result<(), IconValidationError> {
    if reference.is_empty() || reference.len() > MAX_ICON_REFERENCE_LEN {
        return Err(IconValidationError::new(
            "icon",
            format!("semantic icon references must be 1..={MAX_ICON_REFERENCE_LEN} characters"),
        ));
    }
    if reference.starts_with("clay.") {
        return Err(IconValidationError::new(
            "icon",
            "the clay.* namespace is reserved",
        ));
    }
    if reference.contains("__") || reference.contains("..") {
        return Err(IconValidationError::new(
            "icon",
            "semantic icon references must not contain `..` or `__` sequences",
        ));
    }
    if !reference
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
    {
        return Err(IconValidationError::new(
            "icon",
            "semantic icon references must use lowercase kebab/dot syntax",
        ));
    }
    Ok(())
}

// ── Bounded SVG path-data parser ─────────────────────────────────────────────
//
// Parses bounded `d` strings into absolute [`IconPathCommand`] sequences at
// load time. Supports the full SVG 1.1 command set (MmLlHhVvCcSsQqTtAaZz) with
// relative forms, implicit repetition, and comma/whitespace/sign separators.
// Input length is bounded by the caller (ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES);
// command count is capped here before push.

struct PathScanner<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> PathScanner<'a> {
    fn skip_separators(&mut self) {
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_whitespace() || self.bytes[self.pos] == b',')
        {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_separators();
        self.bytes.get(self.pos).copied()
    }

    /// Parse one SVG number: sign, digits, decimal point, exponent.
    fn number(&mut self) -> Result<f32, IconValidationError> {
        self.skip_separators();
        let start = self.pos;
        if matches!(self.bytes.get(self.pos), Some(b'+') | Some(b'-')) {
            self.pos += 1;
        }
        let mut digits = 0;
        while matches!(self.bytes.get(self.pos), Some(c) if c.is_ascii_digit()) {
            self.pos += 1;
            digits += 1;
        }
        if matches!(self.bytes.get(self.pos), Some(b'.')) {
            self.pos += 1;
            while matches!(self.bytes.get(self.pos), Some(c) if c.is_ascii_digit()) {
                self.pos += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return Err(IconValidationError::new(
                "paths.d",
                format!(
                    "malformed path data: expected a number at byte {}",
                    self.pos
                ),
            ));
        }
        if matches!(self.bytes.get(self.pos), Some(b'e') | Some(b'E')) {
            let mut lookahead = self.pos + 1;
            if matches!(self.bytes.get(lookahead), Some(b'+') | Some(b'-')) {
                lookahead += 1;
            }
            if matches!(self.bytes.get(lookahead), Some(c) if c.is_ascii_digit()) {
                self.pos = lookahead;
                while matches!(self.bytes.get(self.pos), Some(c) if c.is_ascii_digit()) {
                    self.pos += 1;
                }
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| IconValidationError::new("paths.d", "path data must be valid UTF-8"))?;
        let value: f32 = text.parse().map_err(|_| {
            IconValidationError::new("paths.d", format!("invalid path number `{text}`"))
        })?;
        check_finite(value, "paths.d")?;
        Ok(value)
    }

    fn flag(&mut self) -> Result<f32, IconValidationError> {
        let value = self.number()?;
        if is_arc_flag(value) {
            Ok(value)
        } else {
            Err(IconValidationError::new(
                "paths.d",
                format!("arc flags must be 0 or 1 (got {value})"),
            ))
        }
    }
}

/// Parse bounded SVG path data into absolute commands.
///
/// The command budget is enforced during expansion so hostile input cannot
/// allocate proportional memory before rejection.
pub fn parse_icon_path(d: &str) -> Result<Vec<IconPathCommand>, IconValidationError> {
    let mut scanner = PathScanner {
        bytes: d.as_bytes(),
        pos: 0,
    };
    let mut commands: Vec<IconPathCommand> = Vec::new();
    // Current point and path start for relative/absolute normalization.
    let mut current = (0.0_f32, 0.0_f32);
    let mut path_start = (0.0_f32, 0.0_f32);
    // Last control point for S/T reflection (absolute coordinates).
    let mut last_cubic_control: Option<(f32, f32)> = None;
    let mut last_quad_control: Option<(f32, f32)> = None;
    let mut previous_command: Option<u8> = None;

    while scanner.peek().is_some() {
        let command_char = match scanner.peek() {
            Some(c) if c.is_ascii_alphabetic() => {
                scanner.pos += 1;
                c
            }
            Some(_) => match previous_command {
                // Implicit repetition; a repeated relative moveto becomes a lineto.
                Some(c @ (b'M' | b'm')) => {
                    if c == b'M' {
                        b'L'
                    } else {
                        b'l'
                    }
                }
                Some(c) => c,
                None => {
                    return Err(IconValidationError::new(
                        "paths.d",
                        "path data must begin with a command",
                    ));
                }
            },
            None => break,
        };
        let command = command_char;
        let upper = command.to_ascii_uppercase();
        let relative = command.is_ascii_lowercase();
        if commands.is_empty() && upper != b'M' {
            return Err(IconValidationError::new(
                "paths.d",
                "path data must begin with a moveto command",
            ));
        }
        macro_rules! point {
            () => {{
                let x = scanner.number()?;
                let y = scanner.number()?;
                check_coordinate(x, "paths.d")?;
                check_coordinate(y, "paths.d")?;
                if relative {
                    (current.0 + x, current.1 + y)
                } else {
                    (x, y)
                }
            }};
        }
        let command = match command.to_ascii_uppercase() {
            b'M' | b'L' => {
                let p = point!();
                if upper == b'M' {
                    path_start = p;
                    last_cubic_control = None;
                    last_quad_control = None;
                    IconPathCommand::MoveTo([p.0, p.1])
                } else {
                    IconPathCommand::LineTo([p.0, p.1])
                }
            }
            b'H' => {
                let x = scanner.number()?;
                check_coordinate(x, "paths.d")?;
                let x = if relative { current.0 + x } else { x };
                IconPathCommand::HorizontalTo(x)
            }
            b'V' => {
                let y = scanner.number()?;
                check_coordinate(y, "paths.d")?;
                let y = if relative { current.1 + y } else { y };
                IconPathCommand::VerticalTo(y)
            }
            b'C' => {
                let x1 = scanner.number()?;
                let y1 = scanner.number()?;
                let x2 = scanner.number()?;
                let y2 = scanner.number()?;
                let x = scanner.number()?;
                let y = scanner.number()?;
                check_coordinate(x1, "paths.d")?;
                check_coordinate(y1, "paths.d")?;
                check_coordinate(x2, "paths.d")?;
                check_coordinate(y2, "paths.d")?;
                check_coordinate(x, "paths.d")?;
                check_coordinate(y, "paths.d")?;
                let (x1, y1, x2, y2, ex, ey) = if relative {
                    (
                        current.0 + x1,
                        current.1 + y1,
                        current.0 + x2,
                        current.1 + y2,
                        current.0 + x,
                        current.1 + y,
                    )
                } else {
                    (x1, y1, x2, y2, x, y)
                };
                last_cubic_control = Some((x2, y2));
                IconPathCommand::CurveTo([x1, y1, x2, y2, ex, ey])
            }
            b'S' => {
                let x2 = scanner.number()?;
                let y2 = scanner.number()?;
                let x = scanner.number()?;
                let y = scanner.number()?;
                check_coordinate(x2, "paths.d")?;
                check_coordinate(y2, "paths.d")?;
                check_coordinate(x, "paths.d")?;
                check_coordinate(y, "paths.d")?;
                let (x2, y2, ex, ey) = if relative {
                    (current.0 + x2, current.1 + y2, current.0 + x, current.1 + y)
                } else {
                    (x2, y2, x, y)
                };
                let (x1, y1) = match last_cubic_control {
                    Some((cx, cy)) => ((2.0 * current.0) - cx, (2.0 * current.1) - cy),
                    None => current,
                };
                check_coordinate(x1, "paths.d")?;
                check_coordinate(y1, "paths.d")?;
                last_cubic_control = Some((x2, y2));
                IconPathCommand::CurveTo([x1, y1, x2, y2, ex, ey])
            }
            b'Q' => {
                let x1 = scanner.number()?;
                let y1 = scanner.number()?;
                let x = scanner.number()?;
                let y = scanner.number()?;
                check_coordinate(x1, "paths.d")?;
                check_coordinate(y1, "paths.d")?;
                check_coordinate(x, "paths.d")?;
                check_coordinate(y, "paths.d")?;
                let (x1, y1, ex, ey) = if relative {
                    (current.0 + x1, current.1 + y1, current.0 + x, current.1 + y)
                } else {
                    (x1, y1, x, y)
                };
                last_quad_control = Some((x1, y1));
                IconPathCommand::QuadTo([x1, y1, ex, ey])
            }
            b'T' => {
                let x = scanner.number()?;
                let y = scanner.number()?;
                check_coordinate(x, "paths.d")?;
                check_coordinate(y, "paths.d")?;
                let (ex, ey) = if relative {
                    (current.0 + x, current.1 + y)
                } else {
                    (x, y)
                };
                // Fold the reflected control point into a plain quadratic.
                let (x1, y1) = match last_quad_control {
                    Some((cx, cy)) => ((2.0 * current.0) - cx, (2.0 * current.1) - cy),
                    None => current,
                };
                check_coordinate(x1, "paths.d")?;
                check_coordinate(y1, "paths.d")?;
                last_quad_control = Some((x1, y1));
                IconPathCommand::QuadTo([x1, y1, ex, ey])
            }
            b'A' => {
                let rx = scanner.number()?;
                let ry = scanner.number()?;
                let rotation = scanner.number()?;
                let large_arc = scanner.flag()?;
                let sweep = scanner.flag()?;
                let x = scanner.number()?;
                let y = scanner.number()?;
                check_coordinate(rx, "paths.d")?;
                check_coordinate(ry, "paths.d")?;
                if rx < 0.0 || ry < 0.0 {
                    return Err(IconValidationError::new(
                        "paths.d",
                        "arc radii must be non-negative",
                    ));
                }
                check_coordinate(rotation, "paths.d")?;
                check_coordinate(x, "paths.d")?;
                check_coordinate(y, "paths.d")?;
                let (ex, ey) = if relative {
                    (current.0 + x, current.1 + y)
                } else {
                    (x, y)
                };
                IconPathCommand::ArcTo([rx, ry, rotation, large_arc, sweep, ex, ey])
            }
            b'Z' => IconPathCommand::ClosePath,
            other => {
                return Err(IconValidationError::new(
                    "paths.d",
                    format!(
                        "unsupported path command `{}`",
                        char::from_u32(u32::from(other)).unwrap_or('?')
                    ),
                ));
            }
        };
        // S/T reflections were folded into plain CurveTo/QuadTo above.
        if commands.len() >= MAX_PATH_COMMANDS {
            return Err(IconValidationError::new(
                "paths.commands",
                format!("path data exceeds the {MAX_PATH_COMMANDS} command limit"),
            ));
        }
        current = command.endpoint(current);
        if matches!(command, IconPathCommand::ClosePath) {
            current = path_start;
        }
        if matches!(
            command,
            IconPathCommand::MoveTo(_) | IconPathCommand::LineTo(_)
        ) {
            last_cubic_control = None;
            last_quad_control = None;
        }
        commands.push(command);
        // Store the original scanned char (case matters: implicit repetition
        // after `M`/`m` must produce `L`/`l` respectively per SVG 1.1).
        previous_command = Some(command_char);
        scanner.skip_separators();
    }
    if commands.is_empty() {
        return Err(IconValidationError::new(
            "paths.d",
            "path data must contain at least one command",
        ));
    }
    Ok(commands)
}

/// Resolved active icon-pack snapshot (Plan 112 task 5), set by the
/// `setIconPack` Clay JS op. `None` in the server slot means the bundled
/// Regular safety subset is active (host fallback, no package execution).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub struct ActiveIconPack {
    /// User-selected pack specifier (selected identity; distinct from the
    /// resolved geometry map so revocation can withdraw data while a reload
    /// decision is pending).
    pub specifier: String,
    pub schema_version: u32,
    /// Stamped with the runtime generation at commit; stale snapshots are
    /// rejected by the transport layer (task 6).
    pub generation: u64,
    pub provenance: crate::shell::design_system::DesignSystemProvenance,
    /// Resolved active geometry: semantic key -> bounded geometry. Only the
    /// selected pack's data lives here; every other loaded pack stays inert
    /// registration data.
    pub icons: std::collections::BTreeMap<String, IconGeometry>,
}

/// Why a wire-bound icon-pack snapshot was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconPackValidationError {
    EmptySpecifier,
    SpecifierTooLong,
    UnsupportedSchemaVersion { version: u32 },
    NoIcons,
    TooManyIcons { count: usize },
    InvalidGeometry { key: String },
    GeometryTooLarge { key: String, bytes: usize },
    PackTooLarge { bytes: usize },
}

impl std::fmt::Display for IconPackValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySpecifier => write!(f, "icon pack specifier cannot be empty"),
            Self::SpecifierTooLong => {
                write!(f, "icon pack specifier exceeds 256 bytes")
            }
            Self::UnsupportedSchemaVersion { version } => write!(
                f,
                "unsupported icon pack schema version {version} (supported: {ICON_SCHEMA_VERSION})"
            ),
            Self::NoIcons => write!(f, "icon pack must resolve at least one icon"),
            Self::TooManyIcons { count } => {
                write!(
                    f,
                    "icon pack resolves {count} icons (max {MAX_ICONS_PER_PACK})"
                )
            }
            Self::InvalidGeometry { key } => {
                write!(f, "icon `{key}` carries invalid geometry")
            }
            Self::GeometryTooLarge { key, bytes } => write!(
                f,
                "icon `{key}` geometry is {bytes} bytes (max {ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES})"
            ),
            Self::PackTooLarge { bytes } => write!(
                f,
                "icon pack geometry totals {bytes} bytes (max {ICON_PACK_PAYLOAD_BUDGET_BYTES})"
            ),
        }
    }
}

impl std::error::Error for IconPackValidationError {}

impl ActiveIconPack {
    /// Validate before wire publication (task 6): identity bounds, supported
    /// schema version, bounded icon count, and per-icon/pack geometry
    /// budgets. Defensive re-check at the DTO/render boundary uses this too.
    pub fn validate(&self) -> Result<(), IconPackValidationError> {
        if self.specifier.trim().is_empty() {
            return Err(IconPackValidationError::EmptySpecifier);
        }
        if self.specifier.len() > 256 {
            return Err(IconPackValidationError::SpecifierTooLong);
        }
        if self.schema_version != ICON_SCHEMA_VERSION {
            return Err(IconPackValidationError::UnsupportedSchemaVersion {
                version: self.schema_version,
            });
        }
        if self.icons.is_empty() {
            return Err(IconPackValidationError::NoIcons);
        }
        if self.icons.len() > MAX_ICONS_PER_PACK {
            return Err(IconPackValidationError::TooManyIcons {
                count: self.icons.len(),
            });
        }
        let mut total_bytes = 0usize;
        for (key, geometry) in &self.icons {
            validate_icon_geometry(geometry)
                .map_err(|_| IconPackValidationError::InvalidGeometry { key: key.clone() })?;
            let bytes: usize = geometry.paths.iter().map(IconPath::d_len).sum();
            if bytes > ICON_GEOMETRY_PAYLOAD_BUDGET_BYTES {
                return Err(IconPackValidationError::GeometryTooLarge {
                    key: key.clone(),
                    bytes,
                });
            }
            total_bytes = total_bytes.saturating_add(bytes);
        }
        if total_bytes > ICON_PACK_PAYLOAD_BUDGET_BYTES {
            return Err(IconPackValidationError::PackTooLarge { bytes: total_bytes });
        }
        Ok(())
    }

    pub fn from_record(
        specifier: &str,
        generation: u64,
        record: &crate::packages::record::PackageRecord,
    ) -> Result<Self, String> {
        let descriptor = record.contributions.icon_pack.as_ref().ok_or_else(|| {
            format!(
                "theme.invalid_icon_pack: package `{}` does not contribute an iconPack",
                record.manifest.name
            )
        })?;
        // Core semantic keys are host contract: only compiled-inventory
        // first-party records may resolve them at activation time. Record-time
        // parsing gates `@clay/` manifests; this closes the loop against any
        // future non-inventory source of an `@clay/` record.
        let inventory_backed = crate::packages::bundled::bundled_entry(&record.manifest.name)
            .is_some_and(|entry| entry.version == record.manifest.version);
        for icon in &descriptor.icons {
            if crate::shell::icons::is_core_icon_key(&icon.key) && !inventory_backed {
                return Err(format!(
                    "theme.invalid_icon_pack: core semantic key `{}` requires compiled first-party provenance",
                    icon.key
                ));
            }
        }
        Ok(Self {
            specifier: specifier.to_string(),
            schema_version: descriptor.schema_version,
            generation,
            provenance: crate::shell::design_system::DesignSystemProvenance::from_record(record),
            icons: descriptor
                .icons
                .iter()
                .map(|icon| (icon.key.clone(), icon.geometry.clone()))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real Phosphor `x` glyph path data (regular weight, pinned v2.0.8).
    const PHOSPHOR_X_D: &str = "M205.66,194.34a8,8,0,0,1-11.32,11.32L128,139.31,61.66,205.66a8,8,0,0,1-11.32-11.32L116.69,128,50.34,61.66A8,8,0,0,1,61.66,50.34L128,116.69l66.34-66.35a8,8,0,0,1,11.32,11.32L139.31,128Z";

    #[test]
    fn parses_real_phosphor_glyph_with_arcs() {
        let commands = parse_icon_path(PHOSPHOR_X_D).expect("parses real phosphor path data");
        assert!(commands.len() > 10);
        assert_eq!(commands[0], IconPathCommand::MoveTo([205.66, 194.34]));
        // All arcs normalized to absolute ArcTo with 0/1 flags.
        assert!(commands
            .iter()
            .all(|command| matches!(command, IconPathCommand::ArcTo(p) if is_arc_flag(p[3]) && is_arc_flag(p[4]))
                || !matches!(command, IconPathCommand::ArcTo(_))));
        assert_eq!(
            *commands.last().expect("non-empty"),
            IconPathCommand::ClosePath
        );
    }

    /// The arc-flag check is exact identity, not a tolerance: only the
    /// representable flag values (`-0.0`, `0.0`, `1.0`) pass, so a near-flag
    /// number cannot be smuggled in as a flag (D4 regression net).
    #[test]
    fn arc_flag_identity_accepts_only_exact_zero_and_one() {
        for accepted in [0.0_f32, -0.0, 1.0] {
            assert!(is_arc_flag(accepted), "must accept {accepted}");
        }
        for rejected in [0.5, 0.999_999_9, 1.0 + f32::EPSILON, 2.0, f32::NAN] {
            assert!(!is_arc_flag(rejected), "must reject {rejected}");
        }
    }

    #[test]
    fn relative_commands_normalize_to_absolute() {
        let commands = parse_icon_path("M10,10l5,5L20,20h5v-5z").expect("parses");
        assert_eq!(
            commands,
            vec![
                IconPathCommand::MoveTo([10.0, 10.0]),
                IconPathCommand::LineTo([15.0, 15.0]),
                IconPathCommand::LineTo([20.0, 20.0]),
                IconPathCommand::HorizontalTo(25.0),
                IconPathCommand::VerticalTo(15.0),
                IconPathCommand::ClosePath,
            ]
        );
    }

    #[test]
    fn implicit_repetition_after_relative_moveto_becomes_relative_lineto() {
        let commands = parse_icon_path("m10,10 5,0 5,0").expect("parses");
        assert_eq!(
            commands,
            vec![
                IconPathCommand::MoveTo([10.0, 10.0]),
                IconPathCommand::LineTo([15.0, 10.0]),
                IconPathCommand::LineTo([20.0, 10.0]),
            ]
        );
    }

    #[test]
    fn smooth_curves_fold_into_plain_curves() {
        let commands =
            parse_icon_path("M10,10C20,20,30,30,40,40S50,60,60,70").expect("parses cubic+S");
        let IconPathCommand::CurveTo(first) = commands[1] else {
            panic!("expected plain cubic");
        };
        assert_eq!((first[0], first[1]), (20.0, 20.0));
        let IconPathCommand::CurveTo(second) = commands[2] else {
            panic!("expected folded cubic");
        };
        // Reflection of (30,30) about (40,40).
        assert_eq!((second[0], second[1]), (50.0, 50.0));

        let commands = parse_icon_path("M10,10Q20,20,30,30T50,50").expect("parses quad+T");
        let IconPathCommand::QuadTo(second) = commands[2] else {
            panic!("expected folded quadratic");
        };
        // Reflection of (20,20) about (30,30).
        assert_eq!((second[0], second[1]), (40.0, 40.0));
    }

    #[test]
    fn command_budget_enforced_before_expansion() {
        let mut d = String::from("M0,0");
        for _ in 0..=MAX_PATH_COMMANDS {
            d.push_str("L1,1");
        }
        let error = parse_icon_path(&d).expect_err("over-budget path data rejected");
        assert!(error.message.contains("command limit"));
    }

    #[test]
    fn malformed_and_hostile_inputs_rejected() {
        for hostile in [
            "",
            "M",
            "M0,0X",
            "M0,0url(evil)",
            "M0,0<script>",
            "M1e999,2",
            "M0,0A8,8,0,2,1,1,1",  // large-arc flag must be 0/1
            "M0,0A-8,8,0,0,1,1,1", // negative radius
            "L5,5",                // must begin with a command context
            "M0,0L",               // truncated command
        ] {
            assert!(
                parse_icon_path(hostile).is_err(),
                "expected rejection for {hostile:?}"
            );
        }
    }

    fn geometry(paths: Vec<IconPath>) -> IconGeometry {
        IconGeometry {
            view_box: [0.0, 0.0, 256.0, 256.0],
            paths,
        }
    }

    fn line_path() -> IconPath {
        IconPath {
            commands: vec![
                IconPathCommand::MoveTo([0.0, 0.0]),
                IconPathCommand::LineTo([256.0, 256.0]),
            ],
            opacity: None,
        }
    }

    #[test]
    fn geometry_bounds_zero_min_max_and_max_plus_one() {
        // Zero paths rejected.
        assert!(validate_icon_geometry(&geometry(Vec::new())).is_err());
        // Min (one path) accepted.
        assert!(validate_icon_geometry(&geometry(vec![line_path()])).is_ok());
        // Max+1 paths rejected.
        let paths = vec![line_path(); MAX_ICON_PATHS + 1];
        assert!(validate_icon_geometry(&geometry(paths)).is_err());
        // viewBox width out of range in both directions.
        let mut wide = geometry(vec![line_path()]);
        wide.view_box[2] = MAX_VIEWBOX_SIDE + 1.0;
        assert!(validate_icon_geometry(&wide).is_err());
        wide.view_box[2] = MIN_VIEWBOX_SIDE - 1.0;
        assert!(validate_icon_geometry(&wide).is_err());
        // Opacity out of range.
        let mut opaque = geometry(vec![line_path()]);
        opaque.paths[0].opacity = Some(1.5);
        assert!(validate_icon_geometry(&opaque).is_err());
        // NaN coordinates rejected (validate covers non-parse construction).
        let mut nan = geometry(vec![line_path()]);
        nan.paths[0].commands[0] = IconPathCommand::MoveTo([f32::NAN, 0.0]);
        assert!(validate_icon_geometry(&nan).is_err());
    }

    #[test]
    fn reference_validation_accepts_core_and_own_prefix_rejects_others() {
        assert!(validate_icon_reference("action.close", "markdown").is_ok());
        assert!(validate_icon_reference("markdown.eye", "markdown").is_ok());
        // Another package's namespace rejected (impersonation).
        assert!(validate_icon_reference("git.branch.custom", "markdown").is_err());
        // Reserved namespace rejected.
        assert!(validate_icon_reference("clay.icon", "markdown").is_err());
        // Prototype-sensitive shapes rejected.
        assert!(validate_icon_reference("a__proto__b", "markdown").is_err());
        // Syntax bounds.
        assert!(validate_icon_reference("", "markdown").is_err());
        assert!(validate_icon_reference("UPPER", "markdown").is_err());
        assert!(
            validate_icon_reference(&"a".repeat(MAX_ICON_REFERENCE_LEN + 1), "markdown").is_err()
        );
        // Runtime trees accept core keys only.
        assert!(validate_core_icon_reference("action.close").is_ok());
        assert!(validate_core_icon_reference("markdown.eye").is_err());
    }

    #[test]
    fn pack_key_gate_reserves_core_keys_to_first_party() {
        assert!(validate_icon_pack_key("action.close", "icons", true).is_ok());
        assert!(validate_icon_pack_key("icons.sparkle", "icons", false).is_ok());
        assert!(validate_icon_pack_key("action.close", "icons", false).is_err());
        assert!(validate_icon_pack_key("git.branch", "icons", false).is_err());
    }

    #[test]
    fn active_icon_pack_requires_compiled_inventory_for_core_keys() {
        // An `@clay/`-named record is not enough: activation-time provenance
        // requires compiled bundled-inventory backing (exact name + version).
        let manifest = serde_json::json!({
            "name": "@clay/icons-test",
            "version": "9.9.9",
            "type": "module",
            "clay": {
                "apiPrefix": "icons-test",
                "entry": "./dist/index.js",
                "loadEntry": "./dist/load.js",
                "permissions": [],
                "modes": [],
                "docs": "./docs/index.md",
                "contributions": {
                    "iconPack": {
                        "schemaVersion": 1,
                        "displayName": "Not In Inventory",
                        "icons": [{
                            "key": "action.close",
                            "viewBox": [0.0, 0.0, 24.0, 24.0],
                            "paths": [{ "d": "M0,0L1,1Z" }]
                        }]
                    }
                }
            }
        });
        let record = crate::packages::record::assemble_package_record(&manifest)
            .expect("record-time @clay/ gate accepts the manifest");
        let error = ActiveIconPack::from_record("@clay/icons-test", 7, &record)
            .expect_err("non-inventory record cannot resolve core keys");
        assert!(error.contains("compiled first-party provenance"));
    }
}
