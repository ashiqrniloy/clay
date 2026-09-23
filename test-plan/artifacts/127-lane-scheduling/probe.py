#!/usr/bin/env python3
"""AT-SPI probe for the plan 126 access-path steps (live, no input synthesis).

    probe.py dump                      # whole tree: depth|role|states|name|text|extents|ifaces
    probe.py editor                    # editor nodes with interfaces, character count, caret
    probe.py wait-ready [seconds]      # poll until the editor exposes EditableText
    probe.py insert <text> [end|caret] # InsertText on the editor (keeps the document size)
    probe.py focus                     # grab editor focus
    probe.py click <name> [index]      # invoke an Action node by name substring
    probe.py completion [seconds]      # poll for a completion popup/status and report it
"""

from __future__ import annotations

import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

EDITOR = "document editor"


def clean(value) -> str:
    return str(value or "").replace("|", " ").replace("\n", " ")


def app():
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        node = desktop.get_child_at_index(index)
        if node is None:
            continue
        name = clean(node.get_name()).upper()
        if name in {"CLAY", "CLAY-DESKTOP"}:
            return node
    raise SystemExit("Clay application is not live")


def walk(node, out):
    out.append(node)
    for index in range(node.get_child_count()):
        try:
            walk(node.get_child_at_index(index), out)
        except Exception:
            continue


def nodes():
    out = []
    walk(app(), out)
    return out


def interfaces(node) -> str:
    names = []
    for name, getter in (
        ("Text", "get_text_iface"),
        ("EditableText", "get_editable_text_iface"),
        ("Action", "get_action_iface"),
        ("Component", "get_component_iface"),
        ("Value", "get_value_iface"),
    ):
        try:
            if getattr(node, getter)() is not None:
                names.append(name)
        except Exception:
            continue
    return ",".join(names)


def extents(node) -> str:
    try:
        box = node.get_extents(Atspi.CoordType.SCREEN)
    except Exception:
        return "-"
    return f"{box.x},{box.y},{box.width}x{box.height}"


def text_of(node) -> str:
    try:
        return clean(Atspi.Text.get_text(node, 0, -1))
    except Exception:
        return ""


def editor_nodes():
    return [n for n in nodes() if EDITOR in clean(n.get_name()).lower()]


def describe_editor(node) -> str:
    parts = [f"role={clean(node.get_role_name())}", f"name={clean(node.get_name())!r}"]
    parts.append(f"ifaces={interfaces(node)}")
    try:
        parts.append(f"chars={Atspi.Text.get_character_count(node)}")
    except Exception:
        parts.append("chars=-")
    try:
        parts.append(f"caret={Atspi.Text.get_caret_offset(node)}")
    except Exception:
        parts.append("caret=-")
    parts.append(f"states={','.join(sorted(s.value_nick for s in node.get_state_set().get_states()))}")
    parts.append(f"extents={extents(node)}")
    return " ".join(parts)


def completion_report() -> str:
    lines = []
    for node in nodes():
        role = clean(node.get_role_name()).lower()
        name = clean(node.get_name())
        if role in {"menu", "menu item", "list item", "list box"} and any(
            word in name.lower() for word in ("completion", "helper", "hello")
        ):
            lines.append(f"{role}|{name}|{extents(node)}")
        if role == "status bar":
            text = text_of(node)
            if text:
                lines.append(f"status|{text}")
    return "\n".join(lines)


def main(argv):
    command = argv[1] if len(argv) > 1 else "dump"
    if command == "dump":
        print("depth|role|states|name|text|extents|ifaces")
        for node in nodes():
            depth = 0
            parent = node
            while True:
                try:
                    parent = parent.get_parent()
                except Exception:
                    break
                if parent is None:
                    break
                depth += 1
            print(
                "|".join(
                    [
                        str(depth),
                        clean(node.get_role_name()),
                        ",".join(sorted(s.value_nick for s in node.get_state_set().get_states())),
                        clean(node.get_name()),
                        text_of(node) if clean(node.get_role_name()) == "status bar" else "",
                        extents(node),
                        interfaces(node),
                    ]
                )
            )
        return 0
    if command == "editor":
        found = editor_nodes()
        if not found:
            print("no Document editor node")
            return 1
        for node in found:
            print(describe_editor(node))
        return 0
    if command == "wait-ready":
        deadline = time.time() + float(argv[2] if len(argv) > 2 else 30)
        while time.time() < deadline:
            for node in editor_nodes():
                if "EditableText" in interfaces(node):
                    print(f"ready after {describe_editor(node)}")
                    return 0
            time.sleep(0.25)
        print("editor never exposed EditableText")
        for node in editor_nodes():
            print(describe_editor(node))
        return 1
    if command == "focus":
        for node in editor_nodes():
            if Atspi.Component.grab_focus(node):
                print(f"focused {describe_editor(node)}")
                return 0
        print("focus refused")
        return 1
    if command == "click":
        wanted = argv[2].lower()
        index = int(argv[3]) if len(argv) > 3 else 0
        hits = [
            node
            for node in nodes()
            if wanted in clean(node.get_name()).lower()
            and "Action" in interfaces(node)
        ]
        if len(hits) <= index:
            print(f"no action node matching {argv[2]!r} (hits={len(hits)})")
            return 1
        node = hits[index]
        try:
            action = node.get_action_iface()
            count = action.get_n_actions()
            names = [action.get_action_name(i) for i in range(count)]
            action.do_action(0)
        except Exception as error:  # noqa: BLE001 - reported, not raised
            print(f"action failed on {clean(node.get_name())!r}: {error}")
            return 1
        print(
            f"clicked {clean(node.get_name())!r}"
            f" role={clean(node.get_role_name())} actions={names}"
        )
        return 0
    if command == "insert":
        text = argv[2]
        where = argv[3] if len(argv) > 3 else "end"
        for node in editor_nodes():
            if "EditableText" not in interfaces(node):
                continue
            Atspi.Component.grab_focus(node)
            if where == "end":
                offset = Atspi.Text.get_character_count(node)
            else:
                offset = Atspi.Text.get_caret_offset(node)
            started = time.time()
            Atspi.EditableText.insert_text(node, offset, text, len(text))
            elapsed = (time.time() - started) * 1000
            print(
                f"inserted {text!r} at {offset} in {elapsed:.1f} ms;"
                f" now {describe_editor(node)}"
            )
            return 0
        print("no editable editor node")
        return 1
    if command == "popup-watch":
        # Fast poller: walk only the editor's parent subtree (the pane) instead
        # of the whole application tree, so the poll interval stays tens of
        # milliseconds and the recorded time is dominated by the app, not by
        # D-Bus round trips.
        seconds = float(argv[2] if len(argv) > 2 else 20)
        editor = None
        deadline = time.time() + 5
        while editor is None and time.time() < deadline:
            found = editor_nodes()
            editor = found[0] if found else None
        if editor is None:
            print("no editor node")
            return 1
        pane = editor
        for _ in range(3):
            pane = pane.get_parent()
            if pane is None:
                break
        started = time.time()
        while time.time() - started < seconds:
            rows = []
            walk(pane, rows)
            hits = [
                n
                for n in rows
                if str(n.get_role_name() or "").lower() in {"list item", "menu item"}
                and any(
                    key in str(n.get_name() or "").lower()
                    for key in ("fn", "rust", "snippet")
                )
            ]
            if hits:
                elapsed = (time.time() - started) * 1000
                print(f"popup first seen {elapsed:.0f} ms after the watcher started")
                for node in hits:
                    print(" ", node.get_role_name(), "|", str(node.get_name()))
                return 0
            time.sleep(0.02)
        print(f"no popup within {seconds:.0f} s")
        return 1
    if command == "completion":
        deadline = time.time() + float(argv[2] if len(argv) > 2 else 5)
        while time.time() < deadline:
            report = completion_report()
            if report:
                print(report)
                return 0
            time.sleep(0.25)
        print("no completion surface or status text")
        return 1
    raise SystemExit(__doc__)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
