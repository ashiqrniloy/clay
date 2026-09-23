//! Client caret-appearance override contract (`setCursorStyle`).

/// Caret glyph shape. `Bar`/`Line` are a thin vertical stroke, `Block` covers
/// the character cell, `Underline` is a horizontal stroke at the line baseline.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum CaretShape {
    Bar,
    Line,
    Block,
    Underline,
}

/// Caret blink behaviour. `Solid` never hides (the reduced-motion-friendly
/// default). `Blink` is discrete on/off with an initial `wait_ms` idle delay.
/// `Phase`/`Smooth` are named for a future alpha-ramp; today they render with
/// discrete on/off timing derived from `period_ms`.
/// `ponytail:` Phase/Smooth use discrete timing until per-frame alpha is wired.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub enum BlinkStyle {
    Solid,
    Blink {
        on_ms: u32,
        off_ms: u32,
        wait_ms: u32,
    },
    Phase {
        period_ms: u32,
    },
    Smooth {
        period_ms: u32,
    },
}

impl BlinkStyle {
    /// True when the caret should animate (anything but `Solid`).
    pub fn animates(&self) -> bool {
        !matches!(self, BlinkStyle::Solid)
    }

    /// The idle delay before the first off-phase, in milliseconds.
    pub fn wait_ms(&self) -> u32 {
        match self {
            BlinkStyle::Blink { wait_ms, .. } => *wait_ms,
            BlinkStyle::Solid | BlinkStyle::Phase { .. } | BlinkStyle::Smooth { .. } => 0,
        }
    }

    /// The visible (on) phase duration, in milliseconds.
    pub fn on_ms(&self) -> u32 {
        match self {
            BlinkStyle::Solid => 0,
            BlinkStyle::Blink { on_ms, .. } => *on_ms,
            BlinkStyle::Phase { period_ms } | BlinkStyle::Smooth { period_ms } => period_ms / 2,
        }
    }

    /// The hidden (off) phase duration, in milliseconds.
    pub fn off_ms(&self) -> u32 {
        match self {
            BlinkStyle::Solid => 0,
            BlinkStyle::Blink { off_ms, .. } => *off_ms,
            BlinkStyle::Phase { period_ms } | BlinkStyle::Smooth { period_ms } => period_ms / 2,
        }
    }
}

/// Caret appearance + blink policy shipped in [`EditorBehaviorRules`] and held
/// as the editor-chrome default in the editor `StyleRegistry`. Colour stays
/// theme-owned (`BaseUiColors::caret`); this struct owns shape + blink only so
/// it never carries raw colour. Language-agnostic; packages override via
/// manifest data, never per-language Rust.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    serde::Serialize,
    serde::Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "ts-bindings", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts-bindings", ts(export_to = "bridge.ts"))]
pub struct CaretStyle {
    pub shape: CaretShape,
    /// Stroke thickness for Bar/Line/Underline, in pixels.
    pub width_px: f32,
    /// Caret height as a fraction of the line height (1.0 = full line).
    pub height_pct: f32,
    /// When true, `Block` renders an outline instead of a solid fill.
    pub hollow: bool,
    pub blink: BlinkStyle,
    /// Reserved smooth-caret travel time; 0 disables (no travel animation).
    pub smooth_animation_ms: u32,
    /// When true, typing resets the blink to visible and restarts the wait.
    pub stop_blink_on_typing: bool,
}

impl CaretStyle {
    /// Clay default: a solid (non-blinking) 1.5px bar at full line height —
    /// reproduces the historical caret and is the reduced-motion-safe default.
    pub const fn default_bar() -> Self {
        Self {
            shape: CaretShape::Bar,
            width_px: 1.5,
            height_pct: 1.0,
            hollow: false,
            blink: BlinkStyle::Solid,
            smooth_animation_ms: 0,
            stop_blink_on_typing: true,
        }
    }

    /// Wire-validation bounds for untrusted transports: geometry must be
    /// finite and within sane caret ranges, and animating blink phases must
    /// not be degenerate (a zero period would flicker every frame).
    pub fn validate(&self) -> Result<(), CaretStyleValidationError> {
        let finite = self.width_px.is_finite()
            && self.height_pct.is_finite()
            && self.smooth_animation_ms <= MAX_CARET_SMOOTH_ANIMATION_MS;
        let geometry = self.width_px > 0.0
            && self.width_px <= MAX_CARET_WIDTH_PX
            && self.height_pct > 0.0
            && self.height_pct <= MAX_CARET_HEIGHT_PCT;
        let blink = match self.blink {
            BlinkStyle::Solid => true,
            BlinkStyle::Blink {
                on_ms,
                off_ms,
                wait_ms,
            } => {
                on_ms <= MAX_CARET_BLINK_PHASE_MS
                    && off_ms <= MAX_CARET_BLINK_PHASE_MS
                    && wait_ms <= MAX_CARET_BLINK_PHASE_MS
            }
            BlinkStyle::Phase { period_ms } | BlinkStyle::Smooth { period_ms } => {
                (2..=MAX_CARET_BLINK_PHASE_MS).contains(&period_ms)
            }
        };
        if finite && geometry && blink {
            Ok(())
        } else {
            Err(CaretStyleValidationError::OutOfBounds)
        }
    }
}

/// Upper bounds enforced by [`CaretStyle::validate`] on wire payloads.
pub const MAX_CARET_WIDTH_PX: f32 = 64.0;

pub const MAX_CARET_HEIGHT_PCT: f32 = 4.0;

pub const MAX_CARET_BLINK_PHASE_MS: u32 = 60_000;

pub const MAX_CARET_SMOOTH_ANIMATION_MS: u32 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretStyleValidationError {
    OutOfBounds,
}

impl Default for CaretStyle {
    fn default() -> Self {
        Self::default_bar()
    }
}
