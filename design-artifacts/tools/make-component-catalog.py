#!/usr/bin/env python3
"""Generate the Quiet Instrument component specimen.

The state matrix is not hand-written: it is built from the real recipe contract
(`packages/design-instrument/package.json`; before plan 118 task 9 it read the
removed Neobrutal package, whose 142-key set the shipped 165-key set extends).
Note: regenerating now emits specimens for the ten new families, so the committed
approved page will report drift until the migration tasks re-approve it. Every declared key gets exactly one
specimen, labelled with that key, so approval maps 1:1 onto catalog entries and
"every kind/variant/slot/state is shown" is a property of construction.

Usage:
  python3 design-artifacts/tools/make-component-catalog.py          # write
  python3 design-artifacts/tools/make-component-catalog.py --check   # verify unchanged
"""

import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "design-artifacts/prototypes/quiet-instrument-migration/component-catalog.html"
PACKAGE = ROOT / "packages/design-instrument/package.json"

# The migration deletes the chat surface (tasks 23-24), so its 12 recipe keys are
# not part of the target set and get no specimen. They are listed, not hidden.
EXCLUDED_FAMILIES = {"chat.default"}

# Canonical state columns. Every family shows all seven so that a state the
# contract does not declare reads as an explicit gap ("—"), not as an omission.
STATES = ["rest", "hover", "active", "focus", "selected", "disabled", "invalid"]

# Which language state rule each cell mirrors. `data-fx` names the effect so the
# mirrors stay one rule per effect instead of one per family, and so a reviewer
# can see exactly which state the specimen claims to show.
DEFAULT_FX = {
    "rest": "plain",
    "hover": "hover-surface",
    "active": "active-press",
    "focus": "focus-ring",
    "selected": "selected-accent",
    "disabled": "disabled-muted",
    "invalid": "invalid-error",
}

FX_OVERRIDES = {
    ("textInput.default", "focus"): "focus-accent",
    ("textInput.default", "input", "focus"): "focus-accent",
    ("textInput.default", "input", "invalid"): "invalid-error",
    ("dropdown.default", "focus"): "focus-accent",
    ("list.default", "selected"): "selected-accent",
    ("fileBrowser.default", "selected"): "selected-accent",
    ("tab.default", "selected"): "selected-accent",
    ("menu.default", "active"): "active-press",
    ("menu.default", "selected"): "selected-accent",
    ("scroll.default", "hover"): "thumb-hover",
    ("scroll.default", "active"): "thumb-active",
    ("scroll.default", "scrollbarThumb", "hover"): "thumb-hover",
    ("scroll.default", "scrollbarThumb", "active"): "thumb-active",
    ("collapse.default", "focus"): "focus-ring",
    ("card.default", "hover"): "hover-surface",
}

# Sections in review order: the plan's grouping (buttons, inputs, controls, tabs,
# chrome, lists/rows, overlays, status/data, editor-adjacent, structural, legacy).
SECTIONS = [
    ("values", "Values & state language", []),
    ("keyboard", "Keyboard affordances", ["kbd.", "tooltip."]),
    ("type", "Type roles", ["label."]),
    ("buttons", "Buttons", ["button."]),
    ("inputs", "Inputs", ["textInput."]),
    ("controls", "Controls & surfaces", ["dropdown.", "collapse.", "divider.", "badge.", "card.", "panel."]),
    ("tabs", "Tabs & chrome", ["tab.", "tabBar.", "shell.", "statusBar.", "statusItem.", "paneSplitTree.", "settingsPanel."]),
    ("lists", "Lists, rows & trees", ["list.", "fileBrowser.", "menu.", "popover."]),
    ("overlays", "Overlays", ["modal.", "overlay.", "portal.", "scroll.", "commandCentre."]),
    ("editor", "Editor-adjacent", ["editor."]),
    ("structural", "Structural layout kinds", ["flex.", "stack."]),
    ("legacy", "Legacy, reserved & removed", []),
]

KIND_OF = {
    "label.": "package-facing",
    "kbd.": "package-facing",
    "tooltip.": "package-facing",
    "button.": "package-facing",
    "textInput.": "package-facing",
    "dropdown.": "package-facing",
    "collapse.": "package-facing",
    "card.": "legacy — not in the kind list",
    "divider.": "package-facing",
    "badge.": "package-facing",
    "tab.": "package-facing",
    "tabBar.": "clay-native",
    "shell.": "clay-native",
    "statusBar.": "clay-native",
    "statusItem.": "package-facing",
    "paneSplitTree.": "clay-native",
    "settingsPanel.": "clay-native",
    "list.": "package-facing",
    "fileBrowser.": "clay-native",
    "menu.": "clay-native",
    "popover.": "package-facing",
    "modal.": "package-facing",
    "overlay.": "package-facing",
    "portal.": "package-facing",
    "scroll.": "package-facing",
    "commandCentre.": "clay-native",
    "editor.": "package-facing (`editor` family)",
    "flex.": "package-facing",
    "stack.": "package-facing",
    "panel.": "package-facing",
}

# Notes: the DESIGN.md §11 target a reviewer checks the specimen against.
NOTES = {
    "button.default": "transparent rest, 1px `border.hairline`, r8; hover = `surface.hover`, active = `press-shift-down`",
    "button.primary": "`accent.primary` fill with `surface.main` text; hover deepens the fill, never adds a shadow",
    "button.muted": "text action: no border, no fill at rest, `text.secondary` label",
    "button.danger": "`diagnostic.error` text; hover = error @0.16 fill, no border",
    "textInput.default": "`surface.control` well + 1px hairline, r12; focus = accent border + 3px accent halo; invalid = `diagnostic.error` border",
    "dropdown.default": "trigger is a default button; popover = `surface.overlay`, r12, hairline, pop shadow",
    "list.default": "transparent root, no border; row r8; hover = `surface.hover`; selected = accent @0.15 (+ 2px inset accent bar on navigation lists)",
    "collapse.default": "transparent header, hairline top on the body, chevron rotates in 150ms",
    "modal.default": "scrim = `surface.scrim` @0.5 + blur 3; dialog = `surface.overlay`, r16, hairline, overlay shadow",
    "panel.default": "`surface.panel` @0.55 veil, 1px hairline, r12, no shadow at rest",
    "panel.fixed": "pinned region: veil, no shadow — a static region never floats",
    "panel.transient": "floating region: overlay shadow, the only elevation above canvas",
    "label.default": "the base text role: 13px UI, `text.primary`",
    "label.body": "prose: 13px, `text.secondary`, 1.6 leading",
    "label.title": "page/section title: 15px/600, negative tracking",
    "label.status": "status text: 12px mono, tabular figures",
    "label.display": "display: 20px/500, reserved for empty-state and landing headers",
    "label.section": "section heading: 13px/600",
    "label.detail": "detail/meta: 12px `text.secondary`",
    "label.caption": "caption: 11px `text.muted`",
    "statusItem.default": "`text.muted` at rest, `text.disabled` when inactive; part of a live region",
    "flex.default": "layout only: no paint, transparent to assistive tech",
    "flex.row": "horizontal layout: gap from spacing tokens, no paint",
    "flex.column": "vertical layout: gap from spacing tokens, no paint",
    "stack.default": "layout only: single-axis stack with token gaps, no paint",
    "overlay.default": "`surface.overlay`, r12, 1px hairline, pop shadow; never nests more than one level",
    "portal.default": "fixed z-layer host: keeps transient surfaces out of the document flow, paints nothing itself",
    "scroll.default": "track transparent; thumb `surface.scrollbar`, pill radius, 9px; hover/active reach full opacity",
    "tab.default": "transparent rest, `text.muted`; selected = accent @0.15 + `accent.primary` text, pill radius",
    "tabBar.default": "the strip: hairline bottom, no fill",
    "card.default": "`surface.control` well, 1px hairline, r12; hover lifts the fill one step — never a shadow",
    "badge.default": "pill, 1px hairline, mono caption; variants use the role text colour with a 34% role border",
    "badge.accent": "accent @0.15 fill, `accent.primary` text, no border",
    "badge.muted": "`text.secondary` + hairline",
    "badge.error": "`diagnostic.error` text + 34% error border",
    "badge.warning": "`diagnostic.warning` text + 34% warning border",
    "badge.success": "`diagnostic.success` text + 34% success border",
    "kbd.default": "veil, r5, 18px, mono; on a primary button the chip is `surface.main` @0.18 with no border",
    "tooltip.default": "`surface.overlay`, r8, hairline, pop shadow, 100ms; never contains controls",
    "popover.default": "`surface.overlay`, r12, hairline, pop shadow; enters with `spring-snappy`, transform only",
    "menu.default": "popover body; items r8, hover `surface.hover`, selected accent @0.15, shortcut chip right-aligned",
    "commandCentre.default": "the only global launcher: r16 dialog, inset input well, keyboard-selected rows, group micro-labels",
    "editor.default": "editor on canvas at a 92ch measure; gutter mono `text.muted`; active line = subtle wash + 2px accent bar",
    "shell.default": "the window frame: title bar 40px, tab strip, working area (canvas), status bar 28px",
    "statusBar.default": "inherits canvas, hairline top, 11px mono, `text.muted`",
    "fileBrowser.default": "tree rows: no boxes, hairline-free; selected = accent @0.15 + 2px inset bar; counts right-aligned mono",
    "paneSplitTree.default": "handle is a 1px hairline that becomes accent on hover/focus/active; panes paint nothing",
    "settingsPanel.default": "panel veil, section micro-labels, actions right-aligned; fields are catalog components",
    "divider.default": "1px `border.hairline`, no margin",
}


def esc(text):
    return (text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;"))


def kbd(spec, extra=""):
    return '<kbd class="kbd%s" data-kbd="%s"></kbd>' % (extra, spec)


CHEV = ('<svg viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor" '
        'stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'
        '<path d="M4.5 2.5 8 6l-3.5 3.5"/></svg>')
CHEV_DOWN = ('<svg viewBox="0 0 12 12" width="12" height="12" fill="none" stroke="currentColor" '
             'stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'
             '<path d="M2.5 4.5 6 8l3.5-3.5"/></svg>')
DOC_ICON = ('<svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" '
            'stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'
            '<path d="M4 1.75h5.5L12.5 5v9.25h-8.5z"/><path d="M9.25 1.75V5.5h3.25"/></svg>')

# ---------------------------------------------------------------- specimen parts
# Generic slot renderers. Families override only what is specific to them.

SLOT_DEFAULTS = {
    "label": '<span class="t-detail">Save</span>',
    "field": ('<span class="input-shell"><span class="cat-icon">%s</span>'
              '<span class="t-body" style="color:inherit">src/shell/design_system.rs</span></span>' % DOC_ICON),
    "input": '<span class="t-body" style="color:inherit">Update the model list</span>',
    "description": '<span class="field-desc">Sent with the current session context.</span>',
    "error": '<span class="field-err">Path not found: src/nope.rs</span>',
    "list": '<span class="pop-list">%s</span>' % (
        '<span class="pop-item">find-docs<span class="pal-meta">skill</span></span>'
        '<span class="pop-item">clay-execution<span class="pal-meta">skill</span></span>'),
    "item": '<span class="pop-item">find-docs<span class="pal-meta">skill</span></span>',
    "row": ('<span class="row"><span class="row-main"><span class="row-head">'
            '<span class="row-name">find-docs</span></span>'
            '<span class="row-desc clamp-2">Retrieves up-to-date documentation, API references, '
            'and code examples for any developer technology.</span></span></span>'),
    "header": '<span class="collapse-head">%s<span class="t-body" style="color:inherit">Workspace</span></span>' % CHEV,
    "body": '<span class="collapse-body">79 files indexed · 8 sources snapshotted</span>',
    "scrim": '<span class="scrim"></span>',
    "dialog": ('<span class="sheet cat-static"><span class="sheet-head"><span class="t-section">'
               'Remove skill?</span></span><span class="sheet-body t-body">'
               'design-taste-frontend will not load in this session.</span></span>'),
    "popover": ('<span class="popover cat-static"><span class="pop-head"><span class="t-micro">'
                'Content theme</span></span><span class="pop-list">'
                '<span class="pop-item" aria-checked="true">Modus Operandi</span>'
                '<span class="pop-item">Modus Vivendi</span></span></span>'),
    "scrollbarTrack": '<span class="scroll-track cat-vscroll"></span>',
    "scrollbarThumb": '<span class="scroll-thumb cat-vscroll"></span>',
    "container": '<span class="ed-frame"><span class="ed-gutter">1|2|3</span><span class="ed-doc"># Title</span></span>',
    "gutter": '<span class="ed-gutter">1<br>2<br>3</span>',
    "activeLine": '<span class="ed-line" data-current="true">## Quiet Instrument</span>',
    "selection": '<span class="ed-sel">quiet instrument</span>',
    "matchingBracket": '<span class="ed-bracket">[</span>',
    "findMatch": '<span class="ed-find">Instrument</span>',
    "chrome": '<span class="ed-chrome">design_system.rs · 1,284 lines · rust</span>',
    "path": '<span class="ed-path">design-artifacts/approved/quiet-instrument-language/</span>',
    "tooltip": '<span class="tipwrap"><span class="t-detail">design_system.rs</span>'
               '<span class="tip" data-open="true">Promoted to the recipe contract</span></span>',
    "group": '<span class="split"><span class="split-pane t-body">Editor</span></span>',
    "pane": '<span class="split-pane t-body">Editor</span>',
    "handle": '<span class="split"><span class="split-pane t-body">Editor</span>'
              '<span class="split-handle" role="separator" aria-orientation="vertical"></span>'
              '<span class="split-pane t-body">Inspector</span></span>',
    "brand": '<span class="cat-brand">Clay</span>',
    "workingArea": '<span class="cat-canvas" data-wash="canvas"><span class="t-body" style="color:inherit">canvas</span></span>',
    "footer": '<span class="cat-foot"><span class="status-seg">VENT.md v1 · clean · editable</span></span>',
    "panel": '<span class="panel cat-static"><span class="panel-head"><span class="panel-title">'
             'Settings</span></span><span class="panel-body t-detail">Row density · Motion · Content theme</span></span>',
    "heading": '<span class="section-head"><span class="t-micro">Appearance</span></span>',
    "actions": ('<span class="cat-row">%s%s</span>'
                % ('<button class="btn" type="button">Cancel</button>',
                   '<button class="btn btn--primary" type="button">Apply</button>')),
    "empty": '<div class="pal-empty">No match. Try “theme” or “file”.</div>',
    "status": '<span class="pal-foot"><span class="status-seg status-seg--muted">4 of 79 files</span>'
              '<span class="status-hints">%s<span class="hint">close</span></span></span>' % kbd("esc"),
}

FAMILY_PARTS = {
    "button.default": {"root": '<button class="btn" type="button"><span>Save</span></button>'},
    "button.primary": {"root": '<button class="btn btn--primary" type="button"><span>Send</span>%s</button>' % kbd("mod+enter")},
    "button.muted": {"root": '<button class="btn btn--ghost" type="button"><span>Cancel</span></button>'},
    "button.danger": {"root": '<button class="btn btn--danger" type="button"><span>Delete</span></button>'},
    "textInput.default": {
        "field": ('<span class="input-shell"><span class="cat-icon">%s</span>'
                  '<span class="t-body" style="color:inherit">Update the model list</span></span>' % DOC_ICON),
        "input": ('<span class="input-shell"><input class="field-input" type="text" '
                  'value="Update the model list" aria-label="Message" readonly></span>'),
        "label": '<span class="t-micro">Message</span>',
    },
    "dropdown.default": {
        "root": '<button class="btn" type="button"><span>Modus Operandi</span>%s</button>' % CHEV_DOWN,
        "trigger": '<button class="btn" type="button"><span>Modus Operandi</span>%s</button>' % CHEV_DOWN,
        "popover": ('<span class="popover cat-static"><span class="pop-list">'
                    '<span class="pop-item" aria-checked="true">Modus Operandi%s</span>'
                    '<span class="pop-item">Modus Vivendi%s</span>'
                    '<span class="pop-item">Gruvbox Material Dark%s</span></span></span>'
                    % (kbd("alt+1"), kbd("alt+2"), kbd("alt+3"))),
        "list": '<span class="pop-list"><span class="pop-item" aria-checked="true">Modus Operandi</span></span>',
        "item": '<span class="pop-item">Gruvbox Material Dark</span>',
    },
    "list.default": {"row": SLOT_DEFAULTS["row"]},
    "collapse.default": {"root": '<span class="collapse" data-open="true">%s%s</span>' % (SLOT_DEFAULTS["header"], SLOT_DEFAULTS["body"])},
    "modal.default": {
        "scrim": '<span class="scrim-demo"><span class="t-body">underlying canvas</span>%s<span class="scrim-note">scrim: surface.scrim @0.5 + blur 3, over the canvas only</span></span>' % SLOT_DEFAULTS["scrim"],
        "dialog": SLOT_DEFAULTS["dialog"],
    },
    "panel.default": {"root": '<span class="cat-row">%s</span>' % SLOT_DEFAULTS["panel"], "header": '<span class="panel-head"><span class="panel-title">Context</span></span>'},
    "panel.fixed": {"root": '<span class="panel panel--fixed cat-static"><span class="panel-head"><span class="panel-title">Files</span></span><span class="panel-body t-detail">pinned region</span></span>'},
    "panel.transient": {"root": '<span class="panel panel--transient cat-static"><span class="panel-head"><span class="panel-title">MCP servers</span></span><span class="panel-body t-detail">floating region</span></span>'},
    "label.default": {"root": '<span class="t-body" style="color:inherit">81 files changed</span>'},
    "label.body": {"root": '<span class="t-body">The workspace document is a real editor column: gutter, 92ch measure, centred.</span>'},
    "label.title": {"root": '<span class="t-title">Quiet Instrument</span>'},
    "label.status": {"root": '<span class="t-status">hyper/glm-5.3-flash · 0/1m</span>'},
    "label.display": {"root": '<span class="t-display">No conversation yet.</span>'},
    "label.section": {"root": '<span class="t-section">Context counts</span>'},
    "label.detail": {"root": '<span class="t-detail">graft · MCP server with 6 tools</span>'},
    "label.caption": {"root": '<span class="t-caption">last write 16:44 · 1,284 lines</span>'},
    "statusItem.default": {"root": '<span class="status-seg">VENT.md v1 · clean · editable</span>'},
    "flex.default": {"root": '<span class="hstack"><span class="cat-chip">A</span><span class="cat-chip">B</span><span class="cat-chip">C</span></span>'},
    "flex.row": {"root": '<span class="hstack"><span class="cat-chip">files</span><span class="cat-chip">memory</span><span class="cat-chip">context</span></span>'},
    "flex.column": {"root": '<span class="vstack"><span class="cat-chip">files</span><span class="cat-chip">memory</span><span class="cat-chip">context</span></span>'},
    "stack.default": {"root": '<span class="vstack"><span class="cat-chip">Session</span><span class="cat-chip">Appearance</span></span>'},
    "overlay.default": {"root": '<span class="cat-note-box">positioned transient layer · one level only</span>'},
    "portal.default": {"root": '<span class="cat-note-box">fixed z-layer host · paints nothing</span>'},
    "scroll.default": {
        "root": '<span class="scrollbox"><span class="t-detail">%s</span></span>' % "<br>".join("line %d" % i for i in range(1, 13)),
        "scrollbarTrack": SLOT_DEFAULTS["scrollbarTrack"],
        "scrollbarThumb": SLOT_DEFAULTS["scrollbarThumb"],
    },
    "tab.default": {"item": '<span class="tab" aria-selected="true">Files</span>'},
    "tabBar.default": {"root": '<span class="tabbar"><span class="tab" aria-selected="true">Files</span><span class="tab">Memory</span><span class="tab">Context</span><span class="tab">Session Info</span><span class="tab">Settings</span></span>'},
    "card.default": {"root": '<span class="card"><span class="t-section">graft</span><span class="t-detail">MCP server · 6 tools</span></span>'},
    "badge.default": {"root": '<span class="badge">v1</span>'},
    "badge.accent": {"root": '<span class="badge badge--accent">13 skills</span>'},
    "badge.muted": {"root": '<span class="badge badge--muted">draft</span>'},
    "badge.error": {"root": '<span class="badge badge--error">failed</span>'},
    "badge.warning": {"root": '<span class="badge badge--warning">modified</span>'},
    "badge.success": {"root": '<span class="badge badge--success">clean</span>'},
    "kbd.default": {"root": '<span class="cat-row">%s%s%s<span class="t-detail">Enter sends, Esc cancels</span></span>' % (kbd("mod+k"), kbd("mod+enter"), kbd("shift+tab"))},
    "tooltip.default": {"root": SLOT_DEFAULTS["tooltip"]},
    "popover.default": {"root": SLOT_DEFAULTS["popover"]},
    "menu.default": {
        "root": '<span class="popover cat-static"><span class="pop-list">'
                '<span class="pop-item">Clear conversation</span>'
                '<span class="pop-item" aria-checked="true">Open keyboard map%s</span>'
                '<span class="pop-item">Copy transcript as markdown</span></span></span>' % kbd("mod+/"),
        "item": '<span class="pop-item">Clear conversation</span>',
    },
    "commandCentre.default": {
        "root": '<div class="palette cat-static"><div class="pal-scopes"><span class="seg">'
                '<span class="seg-item" aria-selected="true">All</span><span class="seg-item">Files</span>'
                '</span></div><div class="pal-list"><div class="pal-group">Commands</div>'
                '<div class="pal-item" data-active="true"><span class="pal-title">Open Workspace</span>'
                '<span class="pal-meta">view</span>%s</div>'
                '<div class="pal-item"><span class="pal-title">Quiet Instrument</span>'
                '<span class="pal-meta">theme</span>%s</div></div>%s</div>'
                % (kbd("mod+1"), kbd("alt+1"), SLOT_DEFAULTS["status"]),
    },
    "editor.default": {
        "root": '<span class="ed-frame"><span class="ed-gutter">1<br>2<br>3</span>'
                '<span class="ed-doc"><span class="ln md-h2">## Quiet Instrument</span>'
                '<span class="ln" data-current="true">One surface, hairline zones.</span>'
                '<span class="ln">· <span class="ed-bracket">[</span>state<span class="ed-bracket">]</span></span>'
                '</span></span>',
    },
    "shell.default": {
        "root": '<span class="cat-win"><span class="cat-titlebar"><span class="cat-brand">Clay</span>'
                '<span class="tabbar"><span class="tab" aria-selected="true">Workspace</span>'
                '<span class="tab">Coding Agent</span></span></span>'
                '<span class="cat-canvas" data-wash="canvas"><span class="t-detail">working area</span></span>'
                '<span class="cat-foot"><span class="status-seg">VENT.md v1 · clean · editable</span></span></span>',
        "header": '<span class="cat-titlebar"><span class="cat-brand">Clay</span></span>',
        "brand": '<span class="cat-brand">Clay</span>',
        "workingArea": SLOT_DEFAULTS["workingArea"],
        "footer": SLOT_DEFAULTS["footer"],
    },
    "statusBar.default": {"root": '<span class="cat-foot"><span class="status-seg">VENT.md v1 · clean · editable</span><span class="status-seg status-seg--muted">79 files</span><span class="status-hints"><span class="hint">%s</span><span class="t-detail">palette</span></span></span>' % kbd("mod+k")},
    "fileBrowser.default": {
        "root": '<span class="tree"><span class="tree-row" data-type="dir"><span class="tree-caret" data-open="true">%s</span>'
                '<span class="tree-icon">%s</span><span class="tree-name">design-artifacts</span></span>'
                '<span class="tree-row" aria-selected="true"><span class="tree-caret"></span>'
                '<span class="tree-icon">%s</span><span class="tree-name">component-catalog.html</span>'
                '<span class="tree-count">1</span></span></span>' % (CHEV, DOC_ICON, DOC_ICON),
    },
    "paneSplitTree.default": {
        "group": '<span class="split"><span class="split-pane t-body">Editor</span><span class="split-pane t-body">Inspector</span></span>',
        "pane": '<span class="split-pane t-body">Editor</span>',
        "handle": SLOT_DEFAULTS["handle"],
    },
    "settingsPanel.default": {
        "panel": SLOT_DEFAULTS["panel"],
        "heading": SLOT_DEFAULTS["heading"],
        "actions": SLOT_DEFAULTS["actions"],
    },
    "divider.default": {"root": '<span class="hr"></span>'},
}


def specimen_html(family, slot, state):
    """One specimen.

    The state attributes go on the component element itself, not on a wrapper, so
    `data-fx` forces the state on exactly the element the language styles: the
    sheet can never show a state the component does not have.
    """
    parts = dict(SLOT_DEFAULTS)
    parts.update(FAMILY_PARTS.get(family, {}))
    markup = parts.get(slot)
    if not markup:
        raise SystemExit("%s.%s has no renderer" % (family, slot))
    fx = FX_OVERRIDES.get((family, slot, state)) or FX_OVERRIDES.get((family, state)) or DEFAULT_FX[state]
    key = "%s.%s.%s" % (family, slot, state)
    tag = re.match(r"<([a-zA-Z][\w-]*)((?:\s[^>]*?)?)>", markup)
    if not tag:
        raise SystemExit("specimen for %s does not start with an element: %s" % (key, markup[:40]))
    attrs = ' data-recipe="%s" data-state="%s" data-fx="%s"' % (key, state, fx)
    return '<div class="cat-spec">%s</div>' % (markup[:tag.end() - 1] + attrs + ">" + markup[tag.end():])


# ---------------------------------------------------------------- page chrome

PAGE_CSS = r"""
/* ===========================================================================
   SECTION A — SPECIMEN SCAFFOLDING. Never binding: it lays out the matrix,
   forces the states that cannot be forced with a pseudo-class, and prints the
   recipe keys. Nothing here is a design decision to approve.
   ========================================================================= */

html, body.catalog { overflow: auto; }
body.catalog { background: var(--c-bg); }

.cat-head {
  position: sticky; top: 0; z-index: 40;
  display: flex; align-items: center; gap: 12px; flex-wrap: wrap;
  padding: 10px 20px; min-height: var(--titlebar-h);
  background: color-mix(in oklab, var(--c-bg) 92%, transparent);
  backdrop-filter: blur(6px);
  border-bottom: 1px solid var(--hairline);
}
.cat-head h1 { font: 600 var(--fs-lg) / 1 var(--font-ui); letter-spacing: -0.01em; }
.cat-head .cat-note { font: 400 var(--fs-xs) / 1.5 var(--font-ui); color: var(--c-text-3); max-width: 60ch; }
.cat-head-actions { display: flex; align-items: center; gap: 6px; margin-left: auto; }

.cat-wrap { padding: 20px; display: flex; flex-direction: column; gap: 30px; max-width: 1560px; margin: 0 auto; }
.cat-section { display: flex; flex-direction: column; gap: 14px; }
.cat-section-head { display: flex; align-items: baseline; gap: 10px; padding-bottom: 8px; border-bottom: 1px solid var(--hairline); }
.cat-section-head h2 { font: 600 var(--fs-lg) / 1.2 var(--font-ui); letter-spacing: -0.01em; }
.cat-section-head .cat-count { font: 400 var(--fs-xs) / 1 var(--font-mono); color: var(--c-text-3); }

.cat-family { display: flex; flex-direction: column; gap: 8px; padding: 14px 0 4px; }
.cat-family-head { display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap; }
.cat-family-head h3 { font: 500 var(--fs-md) / 1.2 var(--font-mono); }
.cat-family-head .cat-kind { font: 500 10px / 1 var(--font-ui); letter-spacing: var(--label-tracking); text-transform: uppercase; color: var(--c-text-3); border: 1px solid var(--hairline); border-radius: var(--r-pill); padding: 3px 7px; }
.cat-family-head .cat-keys { margin-left: auto; font: 400 var(--fs-xs) / 1 var(--font-mono); color: var(--c-text-3); }
.cat-family .cat-target { font: 400 var(--fs-xs) / 1.55 var(--font-ui); color: var(--c-text-2); max-width: 110ch; }
.cat-family .cat-target code { font-family: var(--font-mono); color: var(--c-text); }

.cat-matrix { border-collapse: collapse; width: 100%; table-layout: fixed; }
.cat-matrix th, .cat-matrix td { vertical-align: top; text-align: left; padding: 0; }
.cat-matrix thead th { font: 500 10px / 1 var(--font-ui); letter-spacing: var(--label-tracking); text-transform: uppercase; color: var(--c-text-3); padding: 0 10px 6px; }
.cat-matrix thead th:first-child { width: 118px; padding-left: 0; }
.cat-matrix tbody th { font: 400 var(--fs-xs) / 1.5 var(--font-mono); color: var(--c-text-2); padding: 12px 12px 12px 0; width: 118px; }
.cat-matrix tbody td { padding: 10px 10px 12px; border-top: 1px solid var(--hairline); border-left: 1px solid var(--hairline); }
.cat-matrix tbody tr:first-child td { border-top: 1px solid var(--hairline); }
.cat-matrix td[data-absent] { color: var(--c-text-3); font: 400 var(--fs-xs) / 1 var(--font-mono); text-align: center; }
.cat-matrix td[data-absent] span { opacity: 0.55; }

.cat-spec { display: flex; align-items: center; gap: 8px; min-height: 34px; min-width: 0; }
/* a specimen never spills into its neighbour: boxes fill the cell instead */
.cat-spec { max-width: 420px; }
.cat-spec > * { min-width: 0; max-width: 100%; }
/* a floating surface keeps its natural width: cramping it would misrepresent it */
.cat-spec > .popover { min-width: 250px; width: auto; }
.cat-matrix td .sheet, .cat-matrix td .popover, .cat-matrix td .palette,
.cat-matrix td .cat-win, .cat-matrix td .ed-frame, .cat-matrix td .pop-list { width: 100%; }
.cat-key { display: block; margin-top: 8px; font: 400 10px / 1.4 var(--font-mono); color: var(--c-text-3); word-break: break-all; }

.cat-icon { display: inline-flex; color: currentColor; }
.cat-row { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
.hstack { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.vstack { display: flex; flex-direction: column; gap: 6px; align-items: flex-start; }
.cat-chip { border: 1px solid var(--hairline); border-radius: var(--r-xs); padding: 3px 7px; font: 400 var(--fs-xs) / 1.2 var(--font-mono); color: var(--c-text-2); }
.cat-note-box { border: 1px dashed var(--hairline-strong); border-radius: var(--r-sm); padding: 10px 12px; font: 400 var(--fs-xs) / 1.4 var(--font-mono); color: var(--c-text-3); }
.cat-static { position: static !important; opacity: 1 !important; transform: none !important; pointer-events: auto; }
/* inline by default -> the containers the language expects are block-level */
.cat-static.popover, .cat-static.palette, .cat-static .pal-list, .cat-static.pal-empty { display: block; }
.cat-vscroll { display: block; height: 96px; }

/* State forcing and the language extensions live in `components.css`, which the
   page prototypes link too: one rule per state, one owner for both artefacts.
   What follows is scaffolding only: the specimen sheet's own furniture. */

/* the specimen window: the real shell chrome lives in ds-quiet.css */
.cat-win { display: flex; flex-direction: column; width: 100%; border: 1px solid var(--hairline); border-radius: var(--r-md); overflow: hidden; }
.cat-titlebar { display: flex; align-items: center; gap: 10px; min-height: var(--titlebar-h); padding: 0 10px; background: var(--c-bg); }
.cat-canvas { display: flex; align-items: center; justify-content: center; min-height: 58px; background: var(--c-surface); }
.cat-foot { display: flex; align-items: center; gap: 10px; min-height: var(--status-h); padding: 0 10px; background: var(--c-bg); border-top: 1px solid var(--hairline); font: 400 var(--fs-xs) / 1 var(--font-mono); color: var(--c-text-3); }
.cat-brand { display: inline-flex; align-items: center; gap: 7px; font: 500 var(--fs-md) / 1 var(--font-ui); }

/* the specimen editor: the workspace screen's real chrome lives in pages.css */
.ed-frame { display: grid; grid-template-columns: 42px minmax(0, 1fr); width: 100%; border: 1px solid var(--hairline); border-radius: var(--r-sm); overflow: hidden; }
.ed-gutter { padding: 10px 10px 10px 0; text-align: right; font: 400 var(--fs-xs) / 1.7 var(--font-mono); color: var(--c-text-3); font-variant-numeric: tabular-nums; user-select: none; }
.ed-doc { padding: 10px 12px; min-width: 0; }
.ed-doc .ln { display: block; white-space: pre-wrap; font: 400 var(--fs-sm) / 1.7 var(--font-mono); border-left: 2px solid transparent; margin-left: -8px; padding-left: 6px; border-radius: 0 var(--r-xs) var(--r-xs) 0; }
.ed-doc .ln[data-current='true'] { background: var(--c-accent-soft); border-left-color: var(--c-accent); }
.ed-doc .md-h2 { color: var(--c-doc-h2); font-weight: 600; }
.ed-line { display: inline-block; font: 400 var(--fs-sm) / 1.7 var(--font-mono); background: var(--c-accent-soft); border-left: 2px solid var(--c-accent); padding-left: 6px; }
.ed-sel { background: var(--c-sel); color: var(--c-text); border-radius: var(--r-xs); }
.ed-bracket { background: color-mix(in oklab, var(--c-text) 8%, transparent); outline: 1px solid var(--hairline-strong); border-radius: var(--r-xs); }
.ed-find { background: color-mix(in oklab, var(--c-warn) 28%, transparent); border-radius: var(--r-xs); }
.ed-chrome, .ed-path { font: 400 var(--fs-xs) / 1.5 var(--font-mono); color: var(--c-text-3); }

/* the specimen scroll box and scrim box */
.scrollbox { display: block; width: 190px; height: 96px; overflow: auto; padding: 8px; border: 1px solid var(--hairline); border-radius: var(--r-sm); }
.scroll-track { width: 9px; border-radius: var(--r-pill); background: transparent; }
.scroll-thumb { width: 9px; border-radius: var(--r-pill); background: var(--c-scroll); }
.scrim-demo { position: relative; display: block; min-height: 132px; padding: 14px; border: 1px solid var(--hairline); border-radius: var(--r-md); overflow: hidden; }
.scrim-demo > .t-body, .scrim-demo > .scrim-note { position: relative; z-index: 1; display: block; }
.scrim-note { margin-top: 8px; font: 400 var(--fs-xs) / 1.5 var(--font-mono); color: var(--c-text-3); }

/* structural / legacy notes */
.cat-reserved { display: flex; flex-direction: column; gap: 6px; padding: 14px 16px; border: 1px dashed var(--hairline-strong); border-radius: var(--r-md); }
.cat-reserved h4 { font: 500 var(--fs-md) / 1.3 var(--font-ui); }
.cat-reserved p { font: 400 var(--fs-xs) / 1.6 var(--font-ui); color: var(--c-text-2); max-width: 100ch; }

/* the values / state-language boards */
.cat-board { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 10px; align-items: start; }
.cat-tile { display: flex; flex-direction: column; gap: 8px; padding: 12px 14px; border: 1px solid var(--hairline); border-radius: var(--r-md); }
.cat-tile h4 { font: 500 var(--fs-sm) / 1.3 var(--font-mono); }
.cat-tile .cat-tile-note { font: 400 var(--fs-xs) / 1.5 var(--font-ui); color: var(--c-text-2); }
.cat-demo { display: flex; align-items: center; gap: 8px; min-height: 34px; }
.cat-ruler { height: 14px; border-radius: var(--r-xs); background: var(--c-accent-soft); border: 1px solid color-mix(in oklab, var(--c-accent) 34%, transparent); }
.cat-keys-list { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 4px 18px; }
.cat-key-row { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 5px 0; border-bottom: 1px solid var(--hairline); }
.cat-fx { font: 400 10px / 1 var(--font-mono); color: var(--c-text-3); }
"""



def theme_switcher():
    items = []
    for i, (tid, label, mode) in enumerate(THEME_ROWS, start=1):
        items.append(
            '<button class="pop-item theme-item" type="button" role="menuitemradio" aria-checked="false" '
            'data-set-theme="%s"><span class="swatch" data-theme="%s"><i></i><i></i><i></i></span>'
            '<span>%s</span><span class="cat-fx">%s</span>%s</button>'
            % (tid, tid, esc(label), mode, kbd("alt+%d" % i)))
    return ('<span class="popwrap"><button class="btn" type="button" data-popover-toggle="theme" aria-expanded="false" '
            'aria-haspopup="true">%s<span data-theme-label>Modus Operandi</span>%s</button>'
            '<span class="popover" data-popover="theme" data-open="false" role="menu" aria-label="Content theme">'
            '<span class="pop-head"><span class="t-micro">Content theme</span></span>'
            '<span class="pop-list">%s</span>'
            '<span class="pop-note">four shipped themes · no new themes in this migration</span></span></span>'
            % (CHEV_DOWN, "", "".join(items)))


VALUES_BOARD = [
    ("radius.lg", "16px", "modal dialogs, the palette shell", "16px"),
    ("radius.md", "12px", "panels, popovers, inputs, cards", "12px"),
    ("radius.sm", "8px", "buttons, rows, tooltips, menu items", "8px"),
    ("radius.xs", "5px", "chips, badges, syntax highlights", "5px"),
    ("radius.pill", "999px", "tabs, kbd chips, badges, progress", "28px"),
    ("border.hairline", "1px", "the only structural border weight", "1px"),
    ("border.state", "2px", "focus ring, selected-row bar (never a container)", "2px"),
    ("motion.fast", "150ms ease-out", "hover, press, disclosure, chevrons", None),
    ("motion.enter", "240ms cubic-bezier(.32,.72,0,1)", "overlays and panels entering", None),
    ("motion.flash", "620ms cubic-bezier(.32,.72,0,1)", "keyboard focus that moves somewhere new", None),
    ("opacity.veil", "0.55", "panel/rail veils over the canvas", None),
    ("opacity.accent-soft", "0.15", "selection and accent tints", None),
    ("opacity.disabled", "0.5", "disabled glyphs and thumbs at rest", None),
    ("blur.scrim", "3px", "the scrim only — never canvas, panels or chrome", None),
]

STATE_BOARD = [
    ("rest", "plain", "transparent; a static region never looks pressable"),
    ("hover", "hover-surface", "surface.hover one step, 150ms"),
    ("active", "active-press", "surface.active + 1px press, no scale"),
    ("focus", "focus-ring", "2px focus.ring, 2px offset; text fields also get a 3px accent halo"),
    ("selected", "selected-accent", "accent @0.15 fill + accent text; the fill is the whole signal — no leading bar (§14.13)"),
    ("disabled", "disabled-muted", "surface.control fill, text.disabled, no hover"),
    ("invalid", "invalid-error", "diagnostic.error border only — never a red fill"),
]


def keyboard_section():
    rows = [
        ("mod+k", "Command palette", "Global"),
        ("mod+o", "Open a path", "Workspace"),
        ("mod+i", "Toggle the inspector / outline rail", "Panels"),
        ("mod+s", "Save the document", "Workspace"),
        ("mod+enter", "Send the turn", "Agent"),
        ("mod+.", "Cancel the running turn", "Agent"),
        ("alt+1..4", "Select a content theme", "Appearance"),
        ("alt+up", "Previous entry", "Workspace"),
        ("alt+down", "Next entry", "Workspace"),
        ("shift+tab", "Cycle row density", "Appearance"),
        ("?", "Keyboard map", "Global"),
        ("esc", "Close the transient surface / cancel", "Global"),
    ]
    body = "".join(
        '<span class="cat-key-row"><span class="t-detail">%s</span>'
        '<span class="cat-row">%s<span class="cat-fx">%s</span></span></span>'
        % (esc(desc), kbd(spec), esc(scope))
        for spec, desc, scope in rows)
    return ('<div class="cat-board"><div class="cat-tile"><h4>Shortcut vocabulary</h4>'
            '<div class="cat-keys-list" style="grid-template-columns:1fr">%s</div></div>'
            '<div class="cat-tile"><h4>Hint row (statusBar)</h4>'
            '<div class="cat-demo"><span class="hint">%s<span>palette</span></span>'
            '<span class="hint">%s<span>open path</span></span>'
            '<span class="hint">%s<span>keys</span></span></div>'
            '<p class="cat-tile-note">Hints sit in the status bar and name the key, never a mouse gesture.</p></div>'
            '<div class="cat-tile"><h4>Chips on a primary action</h4>'
            '<div class="cat-demo"><button class="btn btn--primary" type="button"><span>Send</span>%s</button>'
            '<button class="btn" type="button"><span>Save</span>%s</button></div>'
            '<p class="cat-tile-note">On a primary fill the chip is surface.main @0.18 with no border.</p></div></div>'
            % (body, kbd("mod+k"), kbd("mod+o"), kbd("?"), kbd("mod+enter"), kbd("mod+s")))


def values_section():
    tiles = "".join(
        '<div class="cat-tile"><h4>%s</h4><span class="cat-fx">%s</span>%s<p class="cat-tile-note">%s</p></div>'
        % (esc(name), esc(value or ""),
           '<div class="cat-demo"><span class="cat-ruler" style="width:%s"></span></div>' % demo if demo else "",
           esc(note))
        for name, value, note, demo in VALUES_BOARD)
    states = "".join(
        '<div class="cat-key-row"><span class="cat-row"><span class="t-body" style="color:inherit">%s</span>'
        '<span class="cat-fx">%s</span></span><span class="t-caption">%s</span></div>'
        % (esc(state), esc(fx), esc(note))
        for state, fx, note in STATE_BOARD)
    return ('<section class="cat-section" id="values">'
            '<div class="cat-section-head"><h2>Values &amp; state language</h2>'
            '<span class="cat-count">DESIGN.md §4–§9 · the geometry and motion every specimen below must obey</span></div>'
            '<div class="cat-board">%s</div>'
            '<div class="cat-tile"><h4>State language</h4><div class="cat-keys-list" style="grid-template-columns:1fr">%s</div>'
            '<p class="cat-tile-note">Each matrix cell carries <code>data-fx</code> naming the rule it mirrors, so a '
            'specimen cannot claim a state the language does not have.</p></div>'
            '</section>' % (tiles, states))


def tabs_board():
    """A live tab strip: the specimen doubles as the keyboard model demo."""
    tabs = ["Files", "Memory", "Context", "Session Info", "Settings"]
    panels = {
        "Files": "design-artifacts/prototypes/quiet-instrument-migration/component-catalog.html",
        "Memory": "No memory entries",
        "Context": "13 skills loaded · graft (6 tools) · 79 files",
        "Session Info": "hyper/glm-5.3-flash · effort high · 0/1m",
        "Settings": "Row density · Motion · Content theme",
    }
    tab_html = "".join(
        '<button class="tab" type="button" role="tab" aria-selected="%s" tabindex="%d" id="demo-tab-%d" '
        'aria-controls="demo-panel-%d">%s</button>'
        % ("true" if i == 0 else "false", 0 if i == 0 else -1, i, i, esc(t))
        for i, t in enumerate(tabs))
    panel_html = "".join(
        '<div class="t-body" role="tabpanel" id="demo-panel-%d" aria-labelledby="demo-tab-%d"%s>%s</div>'
        % (i, i, "" if i == 0 else " hidden", esc(panels[t]))
        for i, t in enumerate(tabs))
    return ('<div class="cat-tile"><h4>Live tab strip (arrow keys)</h4>'
            '<div class="panel cat-static" style="min-width:0">'
            '<div class="tabs" role="tablist" aria-label="Inspector" data-tab-default="0">%s</div>'
            '<div class="panel-body">%s</div></div>'
            '<p class="cat-tile-note">tab.default.item.selected + tabBar.default.root: the strip carries the hairline, '
            'the selected item the accent tint. Arrow keys move selection, Tab leaves the strip.</p></div>'
            % (tab_html, panel_html))


def legacy_section():
    notes = ('<div class="cat-reserved"><h4>table — reserved kind, no specimen</h4>'
             '<p><code>table</code> is a reserved package-facing kind with no recipe family in any design system and no '
             'consuming surface. It gets no specimen and no invented keys: a table surface must declare a '
             '<code>table.default.*</code> family (and the kind must appear in the component catalog) before it ships. '
             'Bounds allow it; the contract does not have it yet.</p></div>'
             '<div class="cat-reserved"><h4>chat.default — removed, not re-skinned</h4>'
             '<p>The 12 <code>chat.default.*</code> keys are deleted with the chat surface (tasks 23–24), so they get no '
             'specimen and are not carried into <code>@clay/design-instrument</code> (130 keys, inventory finding 1). '
             'The launcher becomes the landing surface instead: a tab holds one workspace and one agent, and the agent is one of its two views.</p></div>'
             '<div class="cat-reserved"><h4>card.default — legacy family, keep or drop</h4>'
             '<p><code>card.default.root.{rest,hover}</code> is declared in both reference packages but appears in no '
             'component-kind list and no CSS module. Specimens are shown above so the decision is visible: either the '
             'card becomes a listed kind with a real consumer, or the two keys go with the reference packages. '
             'Recommendation: drop the keys (they are unconsumed dead data) unless a card surface is planned.</p></div>'
             '<div class="cat-reserved"><h4>welcome.default, transientMenu.default, completion.default — not in the contract</h4>'
             '<p>Core fallbacks exist for these names but no shipped package declares a family for them. The new package '
             'must not invent keys for them (task 10 decides whether the core fallbacks stay) — the transient menu we do '
             'ship is <code>menu.default.*</code>, shown above.</p></div>')
    return ('<section class="cat-section" id="legacy"><div class="cat-section-head"><h2>Legacy, reserved &amp; removed</h2>'
            '<span class="cat-count">declared-name inventory decisions</span></div>%s</section>' % notes)


# ---------------------------------------------------------------- assembly

def load_contract():
    data = json.loads(PACKAGE.read_text())
    recipes = data["clay"]["contributions"]["uiDesignSystem"]["recipes"]
    families = {}
    for key in recipes:
        component, variant, slot, state = key.split(".")
        families.setdefault("%s.%s" % (component, variant), {}).setdefault(slot, []).append(state)
    chat = sorted(k for k in recipes if k.split(".")[0] == "chat")
    return families, chat


def family_prefix(family):
    for prefix in KIND_OF:
        if family.startswith(prefix):
            return prefix
    raise SystemExit("no kind mapping for family %s" % family)


def render_family(family, slots):
    prefix = family_prefix(family)
    keys = ["%s.%s.%s" % (family, slot, state) for slot in slots for state in slots[slot]]
    global ABSENT
    declared = {st for slot in slots for st in slots[slot]}
    # Interactive families get all seven columns so an undeclared state reads as a
    # visible gap; a family that declares only `rest` gets one column.
    columns = STATES if declared - {"rest"} else ["rest"]
    noninteractive = columns == ["rest"]
    rows = []
    for slot in slots:
        cells = []
        for state in columns:
            key = "%s.%s.%s" % (family, slot, state)
            if state in slots[slot]:
                cells.append('<td>%s<code class="cat-key">%s</code></td>' % (specimen_html(family, slot, state), key))
            else:
                ABSENT += 1
                cells.append('<td data-absent="%s" title="not declared in the contract"><span>—</span></td>' % key)
        rows.append('<tr><th scope="row">%s</th>%s</tr>' % (esc(slot), "".join(cells)))
    head = "".join('<th scope="col">%s</th>' % s for s in columns)
    note = NOTES.get(family, "")
    note_html = ""
    if note:
        note_html = '<p class="cat-target">%s</p>' % re.sub(r"`([^`]+)`", r"<code>\1</code>", esc(note))
    label = "non-interactive: rest only" if noninteractive else "states: " + ", ".join(columns)
    return ('<div class="cat-family" id="fam-%s" data-family="%s" data-interactive="%s">'
            '<div class="cat-family-head"><h3>%s</h3><span class="cat-kind">%s</span>'
            '<span class="cat-keys">%s keys · %s</span></div>%s'
            '<table class="cat-matrix" aria-label="%s states"><thead><tr><th scope="col">slot</th>%s</tr></thead>'
            '<tbody>%s</tbody></table></div>'
            % (family.replace(".", "-"), family, "false" if noninteractive else "true",
               esc(family), esc(KIND_OF[prefix]), len(keys), esc(label), note_html,
               esc(family), head, "".join(rows)))


def render_section(section):
    sid, title, prefixes = section
    families = [f for f in FAMILIES if f.startswith(tuple(prefixes))]
    families.sort(key=lambda f: list(FAMILIES).index(f))
    if not families:
        return ""
    count = sum(len(FAMILIES[f][s]) for f in families for s in FAMILIES[f])
    return ('<section class="cat-section" id="sec-%s"><div class="cat-section-head"><h2>%s</h2>'
            '<span class="cat-count">%d families · %d keys</span></div>%s</section>'
            % (sid, title, len(families), count, "".join(render_family(f, FAMILIES[f]) for f in families)))


def build():
    global FAMILIES, THEME_ROWS, CHAT_KEYS, ABSENT
    ABSENT = 0
    FAMILIES, CHAT_KEYS = load_contract()
    THEME_ROWS = [
        ("modus-operandi", "Modus Operandi", "light"),
        ("modus-vivendi", "Modus Vivendi", "dark"),
        ("gruvbox-material-dark", "Gruvbox Material Dark", "dark"),
        ("gruvbox-material-light", "Gruvbox Material Light", "light"),
    ]
    FAMILIES = {f: s for f, s in FAMILIES.items() if f not in EXCLUDED_FAMILIES}
    declared_slots = {slot for fam in FAMILIES.values() for slot in fam}
    orphan = sorted(set(SLOT_DEFAULTS) - declared_slots)
    if orphan:
        raise SystemExit("generic renderers for undeclared slots: %s" % orphan)
    for fam, parts in FAMILY_PARTS.items():
        if fam in EXCLUDED_FAMILIES or fam not in FAMILIES:
            continue
        dead = sorted(set(parts) - set(FAMILIES[fam]))
        if dead:
            raise SystemExit("%s overrides undeclared slots: %s" % (fam, dead))
    placed = set()
    sections_html = []
    for section in SECTIONS:
        sid, _title, prefixes = section
        html = render_section(section)
        boards = ""
        if sid == "values":
            boards = values_section()
        elif sid == "keyboard":
            boards = keyboard_section()
        elif sid == "tabs":
            boards = '<div class="cat-board">%s</div>' % tabs_board()
        if html or boards:
            sections_html.append(boards + html)
        for f in FAMILIES:
            if prefixes and f.startswith(tuple(prefixes)):
                placed.add(f)
    unplaced = sorted(set(FAMILIES) - placed)
    if unplaced:
        raise SystemExit("families missing from a section: %s" % unplaced)
    total_keys = sum(len(FAMILIES[f][s]) for f in FAMILIES for s in FAMILIES[f])
    absent = ABSENT
    manifest = {
        "generatedBy": "design-artifacts/tools/make-component-catalog.py",
        "contract": "packages/design-instrument/package.json",
        "shippedKeys": total_keys,
        "families": len(FAMILIES),
        "states": STATES,
        "themes": [t[0] for t in THEME_ROWS],
        "excludedKeys": CHAT_KEYS,
        "keys": ["%s.%s.%s" % (f, slot, st) for f in FAMILIES for slot in FAMILIES[f] for st in FAMILIES[f][slot]],
    }
    html = """<!doctype html>
<html lang="en" data-theme="modus-operandi">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="theme-color" content="#f0f0f0">
<title>Quiet Instrument — component specimen</title>
<link rel="stylesheet" href="theme.css">
<link rel="stylesheet" href="ds-quiet.css">
<link rel="stylesheet" href="components.css">
<style>%s</style>
</head>
<body class="catalog">
<header class="cat-head">
  <h1>Quiet Instrument — component specimen</h1>
  <p class="cat-note">%d shipped recipe keys over %d families, every declared
  state, for review before task 7 freezes the migration prototype set.
  Prototype only: <code>DESIGN.md</code> and <code>design-artifacts/approved/</code> are the authority.</p>
  <div class="cat-head-actions">
    %s
    <span class="seg"><button class="seg-item" type="button" data-set-density="comfortable">comfortable</button><button class="seg-item" type="button" data-set-density="compact">compact</button></span>
    <span class="seg"><button class="seg-item" type="button" data-set-motion="full">motion</button><button class="seg-item" type="button" data-set-motion="reduced">reduced</button></span>
  </div>
</header>
<main class="cat-wrap">
%s
</main>
<script type="application/json" id="catalog-manifest">%s</script>
<script src="ds.js"></script>
</body>
</html>
""" % (PAGE_CSS,
       total_keys, len(FAMILIES),
       theme_switcher(),
       "\n".join(sections_html) + legacy_section(),
       json.dumps(manifest, indent=1))
    html = html.replace("for review before task 7 freezes", "(%d undeclared combinations read as “—”) for review before task 7 freezes" % absent)
    return html


def main():
    html = build()
    if "--check" in sys.argv:
        current = OUT.read_text() if OUT.exists() else ""
        if current != html:
            raise SystemExit("component-catalog.html is stale — re-run without --check")
        print("component-catalog.html is up to date")
        return
    OUT.write_text(html)
    print("wrote %s (%d lines)" % (OUT.relative_to(ROOT), len(html.splitlines())))


if __name__ == "__main__":
    main()
