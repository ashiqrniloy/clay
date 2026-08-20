// Auto-extracted from surface.rs (Plan 090 task 5). Private submodule: caret.
use super::*;

/// Blink phase for the caret state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum BlinkPhase {
    /// Idle delay before the first off-phase (caret stays visible).
    #[default]
    Wait,
    On,
    Off,
}

/// Pure caret-blink state machine. The widget drives [`CaretBlink::advance`]
/// from masonry animation frames and [`CaretBlink::reset`] on user input;
/// `paint_caret` reads [`CaretBlink::is_visible`]. Kept on `EditorSurface` so
/// the timing logic is unit-testable without a widget/event loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CaretBlink {
    pub(super) phase: BlinkPhase,
    pub(super) elapsed_ms: u64,
    pub(super) visible: bool,
}

impl Default for CaretBlink {
    fn default() -> Self {
        // The caret starts visible; the first animation frame (or a Solid
        // style) keeps it so.
        Self {
            phase: BlinkPhase::Wait,
            elapsed_ms: 0,
            visible: true,
        }
    }
}

impl CaretBlink {
    /// Advance the blink clock by `delta_ms` under `style`. `Solid` always
    /// shows; discrete/phase styles cycle Wait -> On -> Off -> On. Zero-length
    /// phases are skipped (bounded so a degenerate all-zero period cannot spin).
    pub(super) fn advance(&mut self, style: &BlinkStyle, delta_ms: u64) {
        if !style.animates() {
            self.phase = BlinkPhase::On;
            self.elapsed_ms = 0;
            self.visible = true;
            return;
        }
        self.elapsed_ms += delta_ms;
        for _ in 0..4 {
            let limit = match self.phase {
                BlinkPhase::Wait => style.wait_ms(),
                BlinkPhase::On => style.on_ms(),
                BlinkPhase::Off => style.off_ms(),
            } as u64;
            if limit != 0 && self.elapsed_ms < limit {
                break;
            }
            if limit != 0 {
                self.elapsed_ms -= limit;
            }
            self.phase = match self.phase {
                BlinkPhase::Wait => BlinkPhase::On,
                BlinkPhase::On => BlinkPhase::Off,
                BlinkPhase::Off => BlinkPhase::On,
            };
        }
        self.visible = !matches!(self.phase, BlinkPhase::Off);
    }

    /// Reset to visible and restart the idle wait (called on user input).
    pub(super) fn reset(&mut self) {
        self.phase = BlinkPhase::Wait;
        self.elapsed_ms = 0;
        self.visible = true;
    }

    pub(super) fn is_visible(&self) -> bool {
        self.visible
    }
}
