#!/usr/bin/env python3
"""Minimal AT-SPI driver used by the plan 125 manual-test run.

    python3 atspi.py dump  <app-name>                 # tree dump to stdout
    python3 atspi.py click <app-name> <role> <name>   # action(0) on first match

<app-name> matches the application's accessible name (`clay-desktop`). The
dump line format is the one the test-plan artifacts use:

    depth|role|selected|states|name|text|extents|app
"""
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi

WANTED_STATES = (
    Atspi.StateType.VISIBLE,
    Atspi.StateType.SHOWING,
    Atspi.StateType.FOCUSED,
    Atspi.StateType.SELECTED,
    Atspi.StateType.ENABLED,
)


def apps():
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(index)
        if app is not None:
            yield app


def find_app(name: str):
    for app in apps():
        if name.lower() in (app.get_name() or "").lower():
            return app
    raise SystemExit(f"no application matching {name!r}")


def extents(node):
    try:
        rect = node.get_extents(Atspi.CoordType.SCREEN)
    except Exception:
        return "-"
    if rect is None or rect.width <= 0:
        return "-"
    return f"{rect.x},{rect.y},{rect.width}x{rect.height}"


def text_of(node):
    try:
        text = node.get_text_iface()
        if text is not None:
            return (text.get_text(0, text.get_character_count()) or "").strip()
    except Exception:
        pass
    return ""


def states_of(node):
    try:
        state_set = node.get_state_set()
    except Exception:
        return "-"
    return ",".join(
        state.value_nick for state in WANTED_STATES if state_set.contains(state)
    ) or "-"


def dump(node, depth, lines, cap=14):
    if node is None or depth > cap:
        return
    lines.append(
        "|".join(
            [
                str(depth),
                node.get_role_name() or "-",
                "selected" if _selected(node) else "-",
                states_of(node),
                (node.get_name() or "").strip(),
                text_of(node),
                extents(node),
                "CLAY-DESKTOP",
            ]
        )
    )
    for index in range(node.get_child_count()):
        dump(node.get_child_at_index(index), depth + 1, lines, cap)


def _selected(node):
    try:
        return node.get_state_set().contains(Atspi.StateType.SELECTED)
    except Exception:
        return False


def walk(node, out, depth=0):
    out.append(node)
    for index in range(node.get_child_count()):
        walk(node.get_child_at_index(index), out, depth + 1)


def main(argv):
    if len(argv) < 3:
        raise SystemExit(__doc__)
    command, app_name = argv[1], argv[2]
    app = find_app(app_name)
    if command == "dump":
        lines = []
        dump(app, 0, lines)
        print("depth|role|selected|states|name|text|extents|app")
        print("\n".join(lines))
        return
    if command == "click":
        role, name = argv[3], argv[4]
        # `=<name>` matches the whole accessible name (the titlebar's
        # `Hide outline` versus the hint's `hide outline Ctrl I`).
        exact = name.startswith("=")
        wanted = name[1:] if exact else name
        nodes = []
        walk(app, nodes)
        matches = [
            node
            for node in nodes
            if (node.get_role_name() or "") == role
            and (
                (node.get_name() or "") == wanted
                if exact
                else wanted.lower() in (node.get_name() or "").lower()
            )
        ]
        print(f"matches: {[(m.get_role_name(), m.get_name(), extents(m)) for m in matches]}")
        if not matches:
            raise SystemExit(f"no {role} matching {name!r}")
        if len(matches) > 1:
            print("refusing to click: ambiguous match", file=sys.stderr)
            raise SystemExit(2)
        target = matches[0]
        action = target.get_action_iface()
        if action is None or action.get_n_actions() < 1:
            raise SystemExit(f"{role} {name!r} exposes no action")
        action.do_action(0)
        print(f"clicked {target.get_role_name()} {target.get_name()!r}")
        return
    raise SystemExit(f"unknown command {command!r}")


if __name__ == "__main__":
    main(sys.argv)
