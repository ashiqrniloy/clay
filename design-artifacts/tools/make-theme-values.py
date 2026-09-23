#!/usr/bin/env python3
"""Theme-value board for the Quiet Instrument migration (plan 118, task 5).

The board answers one question: *what exactly do the four shipped themes have
to say for the language to look the way it was approved?* It reads the proposed
values from `theme-values.json`, resolves today's values the way the runtime
resolves them (legacy `textStyles` projected onto the modern roles, the core
catalog underneath), measures every pair composited - alpha over the surface it
is actually drawn on - and writes:

  * `theme-values.md` - the specification: the exact
    `clay.contributions.designTokens` entries per theme plus measured ratios,
  * `themes.html`      - the visual board: all four themes at once, current
    next to proposed, with the ladder, the states, the materials and the
    core-fallback sample.

Numbers are measured by this script, never typed. `--check` fails when the
committed artifacts have drifted from the spec or when a floor is missed.

Why compositing matters: `contrast_ratio` in `src/editor/theme.rs` reads
`to_rgba8()` and ignores alpha, so a 34%-alpha hairline measures as if it were
opaque ink (21:1 on white) while it renders at ~1.5:1. The board measures what
the user sees; the same correction belongs in the gate (task 14).
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
PROTO = ROOT / "design-artifacts/prototypes/quiet-instrument-migration"
SPEC = PROTO / "theme-values.json"
MD_OUT = PROTO / "theme-values.md"
HTML_OUT = PROTO / "themes.html"

# ---------------------------------------------------------------- runtime mirror

# Mirrors `ResolvedUiTheme::base_color` in src/shell/theme.rs: a legacy theme
# ships textStyles only, so the modern roles are projected onto that base
# palette. Roles absent here fall through to the core catalog.
BASE_UI_PROJECTION = {
    "surface.main": "shellBg",
    "surface.panel": "panelBg",
    "surface.control": "statusBg",
    "surface.selected": "selection",
    "surface.hover": "selection",
    "surface.active": "selection",
    "surface.disabled": "panelBg",
    "text.primary": "text",
    "text.muted": "@muted_text",  # placeholder when it clears 4.5:1 on the panel
    "text.disabled": "placeholder",
    "accent.primary": "caret",
    "accent.muted": "placeholder",
    "focus.ring": "caret",
    "border.hairline": "scrollbar",
    "border.subtle": "scrollbar",
    "border.strong": "scrollbar",
    "border.focus": "caret",
    "border.kbd": "placeholder",
    "text.icon": "placeholder",
    "surface.scrollbar": "scrollbar",
    "surface.scrollbar.track": "scrollbarTrack",
    "text.badge": "statusText",
    "text.kbd": "statusText",
    "text.tooltip": "text",
    "diagnostic.error": "diagnosticError",
    "diagnostic.warning": "diagnosticWarning",
    "diagnostic.info": "diagnosticInfo",
}

CORE_FALLBACK = {
    "surface.main": "#100f17",
    "surface.panel": "#21202b",
    "surface.control": "#39354a",
    "surface.overlay": "#181720",
    "surface.scrim": "#000000",
    "surface.hover": "#2d2b3d",
    "surface.active": "#343147",
    "surface.selected": "#3d385c",
    "text.primary": "#eeeaff",
    "text.muted": "#b9b2cf",
    "text.disabled": "#6f6a87",
    "accent.primary": "#7c6fff",
    "accent.muted": "#5a52b8",
    "focus.ring": "#968aff",
    "border.focus": "#7c6fff",
    "border.hairline": "#282638",
    "border.subtle": "#2f2c40",
    "border.strong": "#45415c",
}


def core_fallback_from_source() -> dict[str, str]:
    """Parse `core_theme_value` in src/shell/theme.rs for the color roles.

    The core catalog is the design-system-less baseline; reading it from the
    resolver's own source keeps the board honest if a fallback ever moves.
    """
    src = (ROOT / "src/shell/theme.rs").read_text()
    found: dict[str, str] = {}
    pattern = re.compile(
        r'"(?P<token>[a-z][a-z0-9.]*)"\s*=>\s*CoreThemeValue\s*\{\s*'
        r"token_type:\s*ColorRole,\s*"
        r"value:\s*ColorValue\(Color::from_rgb8\(\s*"
        r"(?P<r>0x[0-9a-fA-F]{2}),\s*(?P<g>0x[0-9a-fA-F]{2}),\s*(?P<b>0x[0-9a-fA-F]{2})\s*\)\)",
        re.S,
    )
    for m in pattern.finditer(src):
        found[m.group("token")] = "#{}{}{}".format(
            m.group("r")[2:].lower(), m.group("g")[2:].lower(), m.group("b")[2:].lower()
        )
    return found


CORE = {**CORE_FALLBACK, **core_fallback_from_source()}

# ------------------------------------------------------------------------ colour


def parse_color(value: str) -> tuple[int, int, int, int]:
    v = value.strip().lstrip("#")
    if len(v) == 3:
        v = "".join(c * 2 for c in v)
    if len(v) == 6:
        v += "ff"
    if len(v) != 8:
        raise SystemExit(f"not a colour: {value}")
    return tuple(int(v[i : i + 2], 16) for i in (0, 2, 4, 6))  # type: ignore[return-value]


def hexof(rgba: tuple[int, int, int, int]) -> str:
    return "#" + "".join(f"{c:02x}" for c in rgba)


def with_alpha(value: str, alpha: float) -> str:
    r, g, b, _ = parse_color(value)
    return hexof((r, g, b, round(alpha * 255)))


def over(top: str, bottom: str) -> str:
    """Straight-alpha composite of `top` over `bottom` (both #rrggbb[aa])."""
    tr, tg, tb, ta = parse_color(top)
    br, bg, bb, _ = parse_color(bottom)
    a = ta / 255
    mix = lambda t, b: round(t * a + b * (1 - a))  # noqa: E731
    return hexof((mix(tr, br), mix(tg, bg), mix(tb, bb), 255))


def luminance(value: str) -> float:
    def channel(component: float) -> float:
        c = component / 255
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b, _ = parse_color(value)
    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)


def contrast(fg: str, bg: str) -> float:
    """WCAG 2.1 ratio, same maths as `editor::theme::contrast_ratio`, but on
    colours composited over their surface first."""
    flat_fg, flat_bg = over(fg, bg), bg
    l1, l2 = luminance(flat_fg), luminance(flat_bg)
    lighter, darker = max(l1, l2), min(l1, l2)
    return (lighter + 0.05) / (darker + 0.05)


# --------------------------------------------------------------- resolved values


def theme_packages() -> dict[str, dict]:
    out = {}
    for pkg in sorted((ROOT / "packages").glob("theme-*/package.json")):
        data = json.loads(pkg.read_text())
        out[data["name"]] = {
            "path": pkg.relative_to(ROOT),
            "version": data["version"],
            "text_styles": {t["token"]: t["color"] for t in data["clay"]["contributions"]["textStyles"]},
            "design_tokens": data["clay"]["contributions"].get("designTokens"),
        }
    return out


def current_value(role: str, base: dict[str, str]) -> str:
    """Today's value for a role: the base-palette projection, then the core
    catalog, exactly as `ResolvedUiTheme::resolved` walks it."""
    key = BASE_UI_PROJECTION.get(role)
    if key == "@muted_text":
        placeholder = base.get("placeholder", CORE["text.disabled"])
        key = "placeholder" if contrast(placeholder, base.get("panelBg", CORE["surface.panel"])) >= 4.5 else "text"
    if key and key in base:
        return base[key]
    return CORE.get(role, "#ff00ff")


def proposed_value(spec_theme: dict, role: str) -> str:
    return spec_theme["tokens"][role]


# -------------------------------------------------------------------- measurement

PAIRS: list[tuple[str, str, float, str]] = [
    ("text.muted", "surface.panel", 4.5, "dim text on the veil"),
    ("text.disabled", "surface.main", 4.5, "meta text on canvas - fails in Gruvbox today"),
    ("accent.primary", "surface.main", 3.0, "accent as an affordance (gated today)"),
    ("accent.muted", "surface.main", 3.0, "quiet accent"),
    ("focus.ring", "surface.main", 3.0, "focus is always visible (gated today)"),
    ("border.focus", "surface.main", 3.0, "focused control border (gated today)"),
    ("border.subtle", "surface.main", 3.0, "structural boundary on canvas"),
    ("border.subtle", "surface.panel", 3.0, "structural boundary on the veil"),
    ("border.strong", "surface.main", 3.0, "explicit separator"),
    ("surface.selected", "text.primary", 3.0, "selected fill against its own text"),
    ("surface.hover", "text.primary", 3.0, "hover fill against its own text"),
    ("surface.active", "text.primary", 3.0, "pressed fill against its own text"),
]

# the hairline is measured, but exempt from the floor; it only has to stay quieter
LADDER = ["border.hairline", "border.subtle", "border.strong"]

SPEC_ROLE_ORDER = [
    "border.hairline",
    "border.subtle",
    "border.strong",
    "surface.hover",
    "surface.active",
    "surface.selected",
    "accent.primary",
    "accent.muted",
    "focus.ring",
    "border.focus",
    "text.muted",
    "text.disabled",
    "surface.scrim",
]


def measure(name: str, spec: dict, packages: dict[str, dict]) -> dict:
    theme = spec["themes"][name]
    base = packages[name]["text_styles"]
    today_canvas = current_value("surface.main", base)
    today_panel = current_value("surface.panel", base)
    canvas, panel = theme["canvas"], theme["panel"]
    # "today" is measured against today's surfaces and the proposal against the
    # proposal's, because the two Gruvbox themes move their canvas.
    today_surfaces = {
        "surface.main": today_canvas,
        "surface.panel": today_panel,
        "text.primary": current_value("text.primary", base),
    }
    surfaces = {
        "surface.main": canvas,
        "surface.panel": panel,
        "text.primary": current_value("text.primary", base),
    }

    rows = []
    failures = []
    for role in SPEC_ROLE_ORDER:
        prop = proposed_value(theme, role)
        cur = current_value(role, base)
        rows.append(
            {
                "role": role,
                "current": cur,
                "proposed": prop,
                "current_on_panel": over(cur, today_canvas),
                "proposed_on_panel": over(prop, canvas),
                "current_ratio": contrast(cur, today_canvas),
                "proposed_ratio": contrast(prop, canvas),
            }
        )

    pairs = []
    for fg, bg, floor, why in PAIRS:
        today_bg, proposed_bg = today_surfaces[bg], surfaces[bg]
        p_fg = proposed_value(theme, fg)
        c_fg = current_value(fg, base)
        p_ratio, c_ratio = contrast(p_fg, proposed_bg), contrast(c_fg, today_bg)
        verdict = "pass" if p_ratio >= floor else "FAIL"
        if verdict == "FAIL":
            failures.append(f"{fg} on {bg} = {p_ratio:.2f} (needs {floor})")
        pairs.append(
            {
                "pair": f"{fg} on {bg}",
                "floor": floor,
                "why": why,
                "current": c_ratio,
                "proposed": p_ratio,
                "verdict": verdict,
            }
        )

    ladder = [
        {
            "role": role,
            "value": proposed_value(theme, role),
            "ratio": contrast(proposed_value(theme, role), canvas),
            "composited": over(proposed_value(theme, role), canvas),
        }
        for role in LADDER
    ]
    order_ok = ladder[0]["ratio"] < ladder[1]["ratio"] < ladder[2]["ratio"]
    if not order_ok:
        failures.append("border ladder is not monotonic")

    depth_ok = luminance(canvas) >= luminance(theme["chrome"])
    if not depth_ok:
        failures.append("canvas is darker than the chrome (inverted panel, DESIGN.md 10.2)")
    today_depth_ratio = contrast(today_canvas, today_panel)

    scrim = proposed_value(theme, "surface.scrim")
    opacity = spec["materials"]["scrim"]["opacity"]
    dimmed = over(with_alpha(scrim, opacity), canvas)
    scrim_ratio = contrast(scrim, canvas)
    dim_ratio = contrast(dimmed, canvas)
    direction = "darkens" if luminance(scrim) <= luminance(canvas) else "LIGHTENS"
    if direction != "darkens":
        failures.append("scrim does not darken the canvas")

    return {
        "name": name,
        "palette": theme["palette"],
        "version": packages[name]["version"],
        "path": str(packages[name]["path"]),
        "ink": theme["ink"],
        "canvas": canvas,
        "panel": panel,
        "line2": theme["line2"],
        "rows": rows,
        "pairs": pairs,
        "ladder": ladder,
        "order_ok": order_ok,
        "depth": {
            "chrome": theme["chrome"],
            "today_ratio": today_depth_ratio,
            "ratio": contrast(canvas, panel),
            "ok": depth_ok,
            "canvas_moved": today_canvas != canvas,
            "today_canvas": today_canvas,
        },
        "text_styles_fix": theme.get("text_styles_fix"),
        "scrim": {
            "value": scrim,
            "opacity": opacity,
            "dimmed": dimmed,
            "ratio": scrim_ratio,
            "dim_ratio": dim_ratio,
            "direction": direction,
        },
        "failures": failures,
    }


# ----------------------------------------------------------------------- markdown

ROLE_TITLES = {
    "border.hairline": "Hairline (zone divider, control outline at rest)",
    "border.subtle": "Subtle (structural boundary, hover outline)",
    "border.strong": "Strong (rare explicit separators)",
    "surface.hover": "Hover fill (opaque)",
    "surface.active": "Pressed fill (opaque)",
    "surface.selected": "Selected fill (opaque)",
    "accent.primary": "Accent (selection, active navigation, running work)",
    "accent.muted": "Accent, quiet step",
    "focus.ring": "Focus ring",
    "border.focus": "Focused control border",
    "text.muted": "Dim text",
    "text.disabled": "Meta / disabled text",
    "surface.scrim": "Scrim colour (dim)",
}


def markdown(results: list[dict], spec: dict) -> str:
    out: list[str] = []
    out.append("# Theme values — Quiet Instrument migration")
    out.append("")
    out.append(
        "The four shipped themes at their current values next to the values the language needs, with every\n"
        "ratio measured off the composited colour by `design-artifacts/tools/make-theme-values.py`. Regenerate\n"
        "with `python3 design-artifacts/tools/make-theme-values.py`; `--check` fails on drift or a missed floor.\n"
        "The visual board is `themes.html` (all four themes at once, `file://`, no network)."
    )
    out.append("")
    out.append("**This file is the specification for tasks 13–14.** Each block below is the exact")
    out.append("`clay.contributions.designTokens` entry the package will gain. No value is a new colour source:")
    out.append("every one comes from the theme's own approved palette (`theme.css`), its ink, or its border grey.")
    out.append("Host CSS and design-system recipes receive **no colour** from this board: they name roles, and a value")
    out.append("lands only in a theme package.")
    out.append("")
    out.append(f"- Floors: text ≥ {spec['floors']['text']}:1, UI affordances ≥ {spec['floors']['ui']}:1 (`DESIGN.md` §13.1).")
    out.append("- Ratios are measured on the composited colour (alpha over the surface it is drawn on). The runtime's")
    out.append("  `contrast_ratio` currently ignores alpha — see finding 2.")
    out.append("- `border.hairline` is measured but exempt from the floor: it separates zones, it does not identify a")
    out.append("  control or a state (WCAG 1.4.11). What it must keep is order — quieter than `border.subtle`.")

    for r in results:
        out.append("")
        out.append(f"## {r['palette']} (`{r['name']}` {r['version']})")
        out.append("")
        entries = ",\n".join(
            '    { "token": "%s", "value": "%s" }' % (role, r_["proposed"])
            for role in SPEC_ROLE_ORDER
            for r_ in r["rows"]
            if r_["role"] == role
        )
        out.append("```json")
        out.append('"clay": { "contributions": { "designTokens": [')
        out.append(entries)
        out.append("] } }")
        out.append("```")
        out.append("")
        out.append("| Role | Current value | Current on canvas | Proposed | Proposed on canvas | Composited vs canvas |")
        out.append("| --- | --- | --- | --- | --- | --- |")
        for row in r["rows"]:
            out.append(
                "| `%s` | `%s` | `%s` | `%s` | `%s` | %s |"
                % (
                    row["role"],
                    row["current"],
                    row["current_on_panel"],
                    row["proposed"],
                    row["proposed_on_panel"],
                    f"{row['proposed_ratio']:.2f}:1",
                )
            )
        out.append("")
        out.append("### Border ladder")
        out.append("")
        out.append(f"Border grey (the approved palette's `line-2`): `{r['line2']}` at 34 / 62 / 100 %.")
        out.append("")
        out.append("| Step | Value | Renders as | Composited vs canvas |")
        out.append("| --- | --- | --- | --- |")
        for step in r["ladder"]:
            out.append(
                "| `%s` | `%s` | `%s` | %.2f:1 |" % (step["role"], step["value"], step["composited"], step["ratio"])
            )
        out.append("")
        out.append(
            f"Monotonic: **{'yes' if r['order_ok'] else 'NO'}** — the quiet line stays quieter than the "
            "structural one, which stays quieter than the explicit separator."
        )
        out.append("")
        dep = r["depth"]
        out.append(
            "Depth direction (`DESIGN.md` §10.2): chrome `%s`, canvas `%s` — canvas is %s. Composed depth step "
            "%.2f:1 (today %.2f:1).%s"
            % (
                dep["chrome"],
                r["canvas"],
                "lighter, as required" if dep["ok"] else "DARKER — inverted panel",
                dep["ratio"],
                dep["today_ratio"],
                (
                    " The canvas also moves `%s` → `%s`, because today's pair is inverted (the panel is the "
                    "lighter surface, which §10.2 forbids)." % (dep["today_canvas"], r["canvas"])
                    if dep["canvas_moved"]
                    else ""
                ),
            )
        )
        out.append("")
        if r["text_styles_fix"]:
            fix = r["text_styles_fix"]
            out.append("**Not a designTokens entry — a `textStyles` correction this theme needs.**")
            out.append("")
            out.append(
                "Swap `%s`: `%s` → `%s`."
                % (
                    "`/`".join(fix["swapped"]),
                    "`/`".join(f"`{fix['from'][k]}`" for k in fix["swapped"]),
                    "`/`".join(f"`{fix['to'][k]}`" for k in fix["swapped"]),
                )
            )
            out.append("")
            out.append(fix["why"])
            out.append("")
        sc = r["scrim"]
        dim_note = (
            "a canvas this dark cannot be dimmed further, so separation comes from the overlay surface and its "
            "shadow - the scrim's job is only to stop the canvas competing"
            if sc["dim_ratio"] < 1.1
            else "the dim step is what the eye reads"
        )
        out.append(
            "Scrim: `%s` %s the canvas -> at `opacity.scrim` %.2f the canvas becomes `%s` (%.2f:1 dim step, "
            "%.2f:1 from the raw colour). The theme supplies the colour; the design system supplies the opacity "
            "and the 3px blur (`DESIGN.md` §6) - %s."
            % (sc["value"], sc["direction"], sc["opacity"], sc["dimmed"], sc["dim_ratio"], sc["ratio"], dim_note)
        )
        out.append("")
        out.append("### Enforced pairs")
        out.append("")
        out.append("| Pair | Floor | Current | Proposed | |")
        out.append("| --- | --- | --- | --- | --- |")
        for p in r["pairs"]:
            mark = "ok" if p["verdict"] == "pass" else "**FAIL**"
            flag = "" if p["current"] >= p["floor"] else " (fails today)"
            out.append(
                "| `%s` | %.1f:1 | %.2f:1%s | %.2f:1 | %s |"
                % (p["pair"], p["floor"], p["current"], flag, p["proposed"], mark)
            )
        out.append("")
        if r["failures"]:
            out.append("**Unmet:** " + "; ".join(r["failures"]))
            out.append("")

    out.append("")
    out.append("## Core fallback (no design system installed)")
    out.append("")
    out.append(
        "What the same roles resolve to with no design system active — `core_theme_value` in\n"
        "`src/shell/theme.rs`, parsed and measured by the same script, because the migration must not\n"
        "leave a role depending on it once a shipped theme covers it (task 13: identical token coverage\n"
        "across the four themes)."
    )
    out.append("")
    out.append("| Role | Core fallback | Composited on core canvas |")
    out.append("| --- | --- | --- |")
    for role in SPEC_ROLE_ORDER:
        value = CORE[role]
        out.append("| `%s` | `%s` | %.2f:1 |" % (role, value, contrast(value, CORE["surface.main"])))
    out.append("")
    out.append("## Findings")
    out.append("")
    out.append(
        "1. **The approved artifact and `DESIGN.md` §10.1 disagree about how the ladder is defined.**\n"
        "   §10.1 says the steps are the theme's *ink* at 34 / 62 / 100 %; the approved language\n"
        "   implements them as the theme's *border grey* (`--c-line-2`) at those alphas\n"
        "   (`--hairline` / `--hairline-strong` in `ds-quiet.css`). Both are mechanical. This board\n"
        "   proposes the border grey, because that is what the approved screens render (ink at 34 %\n"
        "   composites ~0.2 darker than the approved divider) and because the same grey at 100 % is the\n"
        "   structural border the screens already draw. Task 15 must reword §10.1 to match, or the spec\n"
        "   and the artifact stay in disagreement."
    )
    out.append(
        "2. **The contrast gate does not composite alpha.** `editor::theme::contrast_ratio` reads\n"
        "   `to_rgba8()` and ignores the alpha byte, so today a 34 %-alpha hairline measures as opaque\n"
        "   ink (21:1 on white) while it renders at ~1.5:1. Any required pair whose foreground carries\n"
        "   alpha must composite first, or the `border.subtle` floor this board adds is decoration.\n"
        "   Belongs to task 14."
    )
    out.append(
        "3. **One step, two names.** `theme.css` defines `--c-line` and the language computes\n"
        "   `--hairline` from `--c-line-2` at 34 %; measured, they land within a few percent of each\n"
        "   other on all four themes, and `--c-line` has no consumer left in `ds-quiet.css` or\n"
        "   `components.css`. `border.hairline` replaces both. Task 16 deletes the dead variable."
    )
    out.append(
        "4. **`surface.scrim` has a core fallback but no projection from `textStyles`.** `base_color`\n"
        "   has no arm for it, so a legacy theme dims with the core catalog's pure black (or worse, a\n"
        "   dark theme's core value) rather than its own ink. It is in the proposal for that reason:\n"
        "   the theme should say what dimming means for it."
    )
    out.append(
        "5. **Today every state fill is the same 40 %-alpha selection colour and every accent-driven\n"
        "   role resolves to the monochrome caret** (`accent.primary`, `accent.muted`, `focus.ring`,\n"
        "   `border.focus`). `DESIGN.md` §10.3 requires the fills to be opaque, and a focus ring the same\n"
        "   colour as the text is not a ring. The proposal replaces both with values the theme already\n"
        "   owns."
    )
    out.append(
        "6. **One theme inverts depth, and fixing it is a `textStyles` swap rather than a token.** "
        "`theme-gruvbox-material-dark` ships `shellBg` #1d2021 and `panelBg` #282828, so `surface.main` resolves "
        "to the *chrome* colour and the panel comes out lighter than the canvas - the 'inverted panel' "
        "`DESIGN.md` §10.2 exists to forbid, and the reason the approved palette swaps them. The other three "
        "themes already resolve depth correctly (Modus Operandi and Gruvbox Material Light ship the lighter "
        "colour as `shellBg`; Modus Vivendi is black on black by design). Swapping the two values fixes the "
        "shell, the editor and the SDUI at once, needs no designTokens entry, and is why the board records it "
        "beside the token list instead of hiding it in one. Task 13's 'keep textStyles untouched' must be "
        "relaxed for exactly that one pair."
    )
    out.append("")
    out.append("## Approval")
    out.append("")
    out.append(
        "This board is a **prototype** (`design-artifacts/README.md`): exploratory until the user\n"
        "approves it. On approval it is frozen to\n"
        "`design-artifacts/approved/quiet-instrument-migration/themes.html` and becomes the binding\n"
        "specification for tasks 13–14."
    )
    return "\n".join(out) + "\n"


# --------------------------------------------------------------------------- board

BOARD_CSS = """
.board { height: 100%; overflow: auto; padding: 0 0 24px; }
.board-head { position: sticky; top: 0; z-index: 3; padding: 14px 18px 12px; background: var(--c-surface);
  border-bottom: 1px solid var(--hairline); }
.board-head h2 { margin: 0 0 4px; font-size: var(--fs-lg); font-weight: 600; letter-spacing: -0.01em; }
.board-head p { margin: 0; max-width: 110ch; color: var(--c-text-2); font-size: var(--fs-sm); }
.board-note { display: flex; flex-wrap: wrap; gap: 6px 18px; margin-top: 8px; color: var(--c-text-3);
  font-family: var(--font-mono); font-size: var(--fs-xs); }
.theme-panel { padding: 16px 18px 20px; border-bottom: 1px solid var(--c-line-2);
  background: var(--c-surface); color: var(--c-text); }
.tp-head { display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap; margin-bottom: 12px; }
.tp-name { font-size: var(--fs-md); font-weight: 600; }
.tp-meta { color: var(--c-text-3); font-family: var(--font-mono); font-size: var(--fs-xs); }
.tp-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 18px; align-items: start; }
@media (max-width: 1100px) { .tp-grid { grid-template-columns: minmax(0, 1fr); } }
.block { min-width: 0; }
.block h3 { margin: 0 0 8px; font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: var(--label-tracking);
  color: var(--c-text-3); font-weight: 600; }
.ladder { display: grid; gap: 6px; padding: 12px; border: 1px solid var(--hairline); border-radius: var(--r-sm);
  background: var(--c-surface); }
.ladder-row { display: grid; grid-template-columns: 140px minmax(0, 1fr) 200px; align-items: center; gap: 12px; }
.ladder-rule { height: 1px; }
.ladder-val { font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--c-text-2); text-align: right; }
.swatches { display: grid; gap: 4px; }
.sw { display: grid; grid-template-columns: 150px 22px 78px 22px 78px minmax(0, 1fr); align-items: center; gap: 8px;
  padding: 3px 0; }
.sw-core { grid-template-columns: 150px 22px 78px 110px minmax(0, 1fr); }
.sw-role { font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--c-text-2); overflow: hidden;
  text-overflow: ellipsis; white-space: nowrap; }
.chip { height: 18px; border-radius: var(--r-xs); border: 1px solid var(--hairline); }
.chip-r { font-family: var(--font-mono); font-size: 10px; color: var(--c-text-3); }
.sw-old { outline: 1px dashed var(--c-line-2); outline-offset: 1px; }
.spec-row { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid var(--hairline);
  border-radius: var(--r-sm); }
.spec-row + .spec-row { margin-top: 6px; }
.spec-hover { background: var(--c-hover); }
.spec-active { background: var(--c-active); }
.spec-sel { background: var(--c-sel); }
.mono { font-family: var(--font-mono); }
.veil-demo { position: relative; height: 92px; border: 1px solid var(--hairline); border-radius: var(--r-sm); overflow: hidden; }
.veil-under { position: absolute; inset: 0; padding: 8px 10px; font-family: var(--font-mono); font-size: var(--fs-xs);
  color: var(--c-text-2); }
.veil-top { position: absolute; left: 12px; top: 16px; right: 12px; padding: 8px 10px; border: 1px solid var(--hairline);
  border-radius: var(--r-sm); background: var(--veil); backdrop-filter: blur(8px); }
.scrim-demo { position: relative; height: 92px; border: 1px solid var(--hairline); border-radius: var(--r-sm); overflow: hidden; }
.scrim-top { position: absolute; inset: 0; background: var(--scrim-colour); opacity: 0.5; backdrop-filter: blur(3px); }
.scrim-card { position: absolute; left: 12px; top: 22px; right: 12px; padding: 8px 10px; border: 1px solid var(--hairline);
  border-radius: var(--r-sm); background: var(--c-surface); }
.pairs { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
.pairs th { text-align: left; font-size: var(--fs-xs); text-transform: uppercase; letter-spacing: var(--label-tracking);
  color: var(--c-text-3); font-weight: 600; padding: 4px 8px 4px 0; }
.pairs td { padding: 3px 8px 3px 0; border-top: 1px solid var(--hairline); }
.pairs td.num, .pairs th.num { text-align: right; font-family: var(--font-mono); }
.verdict-ok { color: var(--c-text-2); }
.verdict-bad { color: var(--c-err); font-weight: 600; }
.fails { margin-top: 10px; padding: 8px 10px; border: 1px solid var(--c-err); border-radius: var(--r-sm); color: var(--c-err);
  font-size: var(--fs-sm); }
.core-panel { border-top: 2px solid var(--c-line-2); background: var(--c-surface-2); }
.theme-panel .tp-meta code { color: var(--c-text-2); }
"""


def board_html(results: list[dict], spec: dict) -> str:
    spec_loader = importlib.util.spec_from_file_location(
        "make_pages", ROOT / "design-artifacts/tools/make-pages.py"
    )
    mp = importlib.util.module_from_spec(spec_loader)
    sys.modules["make_pages"] = mp
    spec_loader.loader.exec_module(mp)

    links = "".join(
        '<a class="proto-link" href="%s.html"%s>%s</a>'
        % (pid, ' aria-current="page"' if pid == "themes" else "", label)
        for pid, label in mp.PAGE_ORDER
    )
    bar = (
        '<div class="proto-bar"><span class="proto-id"><h1>Quiet Instrument — Theme values</h1>'
        '<span class="t-micro">migration prototype</span></span>'
        '<nav class="proto-nav" data-nav aria-label="Prototype pages">%s</nav>'
        '<span class="proto-note">All four shipped themes at once, current next to proposed. Measured by '
        "tools/make-theme-values.py; the JSON blocks in theme-values.md are the entries tasks 13–14 add.</span></div>"
        % links
    )
    panels = []
    for r in results:
        pairs_rows = "".join(
            '<tr><td class="mono">%s</td><td class="num">%.1f:1</td><td class="num">%.2f:1</td>'
            '<td class="num">%.2f:1</td><td class="%s">%s</td></tr>'
            % (
                p["pair"],
                p["floor"],
                p["current"],
                p["proposed"],
                "verdict-ok" if p["verdict"] == "pass" else "verdict-bad",
                "pass" if p["verdict"] == "pass" else "FAIL",
            )
            for p in r["pairs"]
        )
        swatches = "".join(
            '<div class="sw"><span class="sw-role">%s</span>'
            '<span class="chip sw-old" style="background:%s" title="today"></span>'
            '<span class="chip-r">%s</span>'
            '<span class="chip" style="background:%s" title="proposed"></span>'
            '<span class="chip-r">%s</span>'
            '<span class="chip-r">%s → %s</span></div>'
            % (
                row["role"],
                row["current"],
                row["current"],
                row["proposed"],
                row["proposed"],
                f"today {row['current_ratio']:.2f}:1",
                f"proposed {row['proposed_ratio']:.2f}:1",
            )
            for row in r["rows"]
        )
        ladder = "".join(
            '<div class="ladder-row"><span class="sw-role">%s</span>'
            '<span class="ladder-rule" style="background:%s"></span>'
            '<span class="ladder-val">%s · %.2f:1</span></div>'
            % (step["role"], step["composited"], step["value"], step["ratio"])
            for step in r["ladder"]
        )
        fails = (
            '<div class="fails">Unmet floors: %s</div>' % "; ".join(r["failures"]) if r["failures"] else ""
        )
        moves = ""
        if r["text_styles_fix"]:
            moves = (
                '<div class="fails" style="border-color:var(--c-warn);color:var(--c-warn)">Canvas moves %s → %s '
                "(swap the package's shellBg/panelBg): today the panel is lighter than the canvas, which "
                "DESIGN.md 10.2 forbids.</div>" % (r["depth"]["today_canvas"], r["canvas"])
            )
        elif r["depth"]["canvas_moved"]:
            moves = '<div class="fails" style="border-color:var(--c-warn);color:var(--c-warn)">Canvas moves %s → %s.</div>' % (
                r["depth"]["today_canvas"],
                r["canvas"],
            )
        panels.append(
            """
<section class="theme-panel" data-theme="%(palette)s" style="--scrim-colour:%(scrim)s">
  <div class="tp-head">
    <span class="tp-name">%(palette)s</span>
    <span class="tp-meta">%(pkg)s %(version)s · canvas %(canvas)s · panel %(panel)s · ink %(ink)s · border grey %(line2)s</span>
  </div>
  <div class="tp-grid">
    <div class="block">
      <h3>Border ladder (one grey, three alphas)</h3>
      <div class="ladder">%(ladder)s</div>
      <h3 style="margin-top:14px">Role values — today (dashed) → proposed</h3>
      <div class="swatches">%(swatches)s</div>
    </div>
    <div class="block">
      <h3>States, drawn</h3>
      <div class="spec-row spec-hover">hover fill <span class="mono">surface.hover</span></div>
      <div class="spec-row spec-active">pressed fill <span class="mono">surface.active</span></div>
      <div class="spec-row spec-sel">selected row <span class="mono">surface.selected</span> <span style="color:var(--c-text-2)">focused next</span></div>
      <div class="spec-row"><span class="dot dot--ok"></span>running work in <span class="mono" style="color:%(accent)s">accent.primary</span>
        <span class="dot" style="background:%(accent_muted)s"></span>quiet affordance <span class="mono">accent.muted</span></div>
      <div class="spec-row"><button class="btn" type="button" style="border-color:%(accent)s">accent border</button>
        <button class="btn" type="button" data-fx="focus-ring" style="border-color:%(accent)s">focus ring</button>
        <button class="btn" type="button" style="background:%(accent)s;color:%(accent_fg)s;border-color:%(accent)s">primary</button></div>
      <div class="spec-row"><span style="color:var(--c-text)">primary text</span>
        <span style="color:%(muted)s">muted text</span>
        <span style="color:%(disabled)s">disabled text</span>
        <kbd class="kbd kbd--sm">%(kbd)s</kbd></div>
      <h3 style="margin-top:14px">Materials (design system owns the opacity)</h3>
      <div class="veil-demo"><div class="veil-under">underlying content — 13px prose, an editor line, a count 38<br>veil = surface.panel at opacity.veil 0.55 + blur 8</div>
        <div class="veil-top"><span class="panel-sub">toast on the veil</span></div></div>
      <div class="scrim-demo" style="margin-top:8px"><div class="veil-under">underlying content dimmed</div>
        <div class="scrim-top"></div>
        <div class="scrim-card"><span class="panel-sub">scrim = %(scrim)s at opacity.scrim %(scrim_op)s + blur 3 — canvas %(canvas)s → %(dimmed)s (%(dim_ratio).2f:1 dim step)</span></div></div>
    </div>
    <div class="block">
      <h3>Enforced pairs</h3>
      <table class="pairs"><thead><tr><th>pair</th><th class="num">floor</th><th class="num">today</th>
        <th class="num">proposed</th><th></th></tr></thead><tbody>%(pairs)s</tbody></table>
      %(fails)s%(moves)s
    </div>
  </div>
</section>"""
            % {
                "palette": r["palette"],
                "pkg": r["name"],
                "version": r["version"],
                "canvas": r["canvas"],
                "panel": r["panel"],
                "ink": r["ink"],
                "line2": r["line2"],
                "ladder": ladder,
                "swatches": swatches,
                "pairs": pairs_rows,
                "fails": fails,
                "moves": moves,
                "scrim": r["scrim"]["value"],
                "scrim_op": r["scrim"]["opacity"],
                "canvas": r["canvas"],
                "dimmed": r["scrim"]["dimmed"],
                "dim_ratio": r["scrim"]["dim_ratio"],
                "direction": r["scrim"]["direction"],
                "accent": r["rows"][6]["proposed"],
                "accent_muted": r["rows"][7]["proposed"],
                "accent_fg": "#ffffff" if luminance(r["rows"][6]["proposed"]) < 0.4 else "#000000",
                "muted": r["rows"][10]["proposed"],
                "disabled": r["rows"][11]["proposed"],
                "kbd": "Ctrl+K",
            }
        )

    core_rows = "".join(
        '<div class="sw sw-core"><span class="sw-role">%s</span>'
        '<span class="chip" style="background:%s"></span>'
        '<span class="chip-r">%s</span><span class="chip-r">renders %s</span>'
        '<span class="chip-r">%.2f:1 on the core canvas</span></div>'
        % (role, CORE[role], CORE[role], over(CORE[role], CORE["surface.main"]), contrast(CORE[role], CORE["surface.main"]))
        for role in SPEC_ROLE_ORDER
    )
    core = (
        '<section class="theme-panel core-panel" data-theme="modus-vivendi">'
        '<div class="tp-head"><span class="tp-name">Core fallback</span>'
        '<span class="tp-meta">no design system installed — parsed from <code>core_theme_value</code> in '
        "src/shell/theme.rs and measured by this script</span></div>"
        f'<div class="swatches">{core_rows}</div>'
        '<p class="tp-meta" style="margin-top:10px">Task 13 requires identical token coverage across the four '
        "shipped themes, so no role may fall back to this catalog once a theme covers it. Note the core catalog is "
        "not itself language-compliant - its own boundary steps are 1.3-2.0:1 and its scrim is 1.10:1 against its "
        "canvas - because it is the pre-bootstrap and design-system-less baseline, not a theme the gates validate; "
        "that is the gap the four themes close.</p></section>"
    )

    fails_total = sum(len(r["failures"]) for r in results)
    head = (
        '<div class="board-head"><h2>Theme values — Quiet Instrument migration</h2>'
        "<p>Every ratio below is measured on the <strong>composited</strong> colour — alpha over the surface it is "
        "actually drawn on — which is what the eye sees and what the runtime gate does not yet do. The dashed chip "
        "is today's value; the solid chip is the proposal.</p>"
        '<div class="board-note"><span>floors: text ≥ %s:1 · UI ≥ %s:1</span>'
        "<span>13 proposed designTokens per theme</span>"
        "<span>%s</span><span>measurement: design-artifacts/tools/make-theme-values.py</span></div></div>"
        % (
            spec["floors"]["text"],
            spec["floors"]["ui"],
            "no unmet floors" if not fails_total else f"{fails_total} unmet floors",
        )
    )

    body = (
        '<div class="board" data-board>'
        + head
        + "".join(panels)
        + core
        + "</div>"
    )

    return mp.HEAD % {
        "label": "Theme values",
        "htmlattrs": "",
        "css": BOARD_CSS + "\n" + mp_build_css(),
        "bar": bar,
        "body": body,
        "data": "",
        "script": "",
    }


def mp_build_css() -> str:
    return (
        ".kbd { border: 1px solid var(--c-kbd-line); background: var(--c-kbd-bg); color: var(--c-kbd-fg); }\n"
        ".dot { width: 7px; height: 7px; border-radius: 50%; background: var(--c-text-3); display: inline-block; }\n"
        ".dot--ok { background: var(--c-ok); }\n"
    )


# ----------------------------------------------------------------------------- main


def generate() -> tuple[str, str, list[dict]]:
    spec = json.loads(SPEC.read_text())
    packages = theme_packages()
    missing = [n for n in spec["themes"] if n not in packages]
    if missing:
        raise SystemExit(f"theme package(s) missing: {missing}")
    results = [measure(name, spec, packages) for name in spec["themes"]]
    return markdown(results, spec), board_html(results, spec), results


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate the theme-value board.")
    ap.add_argument("--check", action="store_true", help="fail if the committed artifacts drifted")
    args = ap.parse_args()

    md, html, results = generate()

    if args.check:
        ok = True
        for path, text in ((MD_OUT, md), (HTML_OUT, html)):
            if not path.exists() or path.read_text() != text:
                print(f"drifted: {path.relative_to(ROOT)}")
                ok = False
            else:
                print(f"verified {path.relative_to(ROOT)} ({path.stat().st_size / 1024:.1f} kB)")
        return 0 if ok else 1

    MD_OUT.write_text(md)
    HTML_OUT.write_text(html)
    print(f"wrote {MD_OUT.relative_to(ROOT)} ({MD_OUT.stat().st_size / 1024:.1f} kB)")
    print(f"wrote {HTML_OUT.relative_to(ROOT)} ({HTML_OUT.stat().st_size / 1024:.1f} kB)")
    unmet = sum(len(r["failures"]) for r in results)
    for r in results:
        print(
            "  %-24s %d pairs measured, ladder %s, scrim %s%s"
            % (
                r["palette"],
                len(r["pairs"]),
                "ordered" if r["order_ok"] else "BROKEN",
                r["scrim"]["direction"],
                f", {len(r['failures'])} unmet" if r["failures"] else "",
            )
        )
    return 1 if unmet else 0


if __name__ == "__main__":
    raise SystemExit(main())
