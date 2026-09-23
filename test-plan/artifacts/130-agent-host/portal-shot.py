#!/usr/bin/env python3
"""Portal screenshot cropped to the live Clay window (plan 130 manual pass).

    portal-shot.py OUTPUT.png

The crop rectangle comes from **Clay's own AT-SPI frame extents** and is
cross-checked against the compositor's window bounds before anything is
written. The two sources must agree (within a few pixels); a disagreement —
a stale/incorrect compositor geometry, which is how an unrelated window once
ended up inside a retained capture — fails closed with exit 2 instead of
writing a PNG.

Ported from `scripts/capture-ui-review.sh::crop_window.py`, whose AT-SPI-first
crop was the correct source of truth; the plan 126/129 copy in this directory
trusted the compositor listing alone. No full-desktop capture is ever written:
the crop is always inside the Clay window, inset when the compositor bounds are
unavailable.
"""

from __future__ import annotations

import json
import subprocess
import sys

import gi

gi.require_version("Atspi", "2.0")
gi.require_version("Gio", "2.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import Atspi, GdkPixbuf, Gio, GLib  # noqa: E402

DEST = "dev.avifenesh.ComputerUseLinux.WindowControl"
PATH = "/dev/avifenesh/ComputerUseLinux/WindowControl"
TOLERANCE = 8
FULL_SHOT = "/tmp/clay-plan130-portal-full.png"


def clean(value) -> str:
    return str(value or "").strip()


def atspi_extents():
    """Clay's window frame extents, straight off the accessibility bus."""
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        application = desktop.get_child_at_index(index)
        if clean(application.get_name()).upper() != "CLAY-DESKTOP":
            continue
        try:
            pid = application.get_process_id()
        except Exception:
            pid = 0
        if pid:
            from pathlib import Path

            if not Path(f"/proc/{pid}").exists():
                continue
        for child_index in range(application.get_child_count()):
            child = application.get_child_at_index(child_index)
            if clean(child.get_name()) != "Clay":
                continue
            frame = child.get_extents(Atspi.CoordType.SCREEN)
            if frame.width > 0 and frame.height > 0:
                return (frame.x, frame.y, frame.width, frame.height)
    return None


def clay_windows():
    """Every clay-desktop window the compositor reports (stale ones included)."""
    try:
        out = subprocess.run(
            [
                "gdbus",
                "call",
                "--session",
                "--dest",
                DEST,
                "--object-path",
                PATH,
                "--method",
                f"{DEST}.ListWindows",
            ],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except Exception:
        return None
    if out.startswith("('"):
        out = out[2:-3]
    try:
        windows = json.loads(out)
    except Exception:
        return None
    found = []
    for window in windows:
        if not str(window.get("wm_class") or "").startswith("clay"):
            continue
        bounds = window.get("bounds") or {}
        found.append((bounds.get("x"), bounds.get("y"), bounds.get("width"), bounds.get("height")))
    return found


def portal_full_screenshot() -> GdkPixbuf.Pixbuf:
    bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
    proxy = Gio.DBusProxy.new_sync(
        bus,
        Gio.DBusProxyFlags.NONE,
        None,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Screenshot",
        None,
    )
    reply = proxy.call_sync(
        "Screenshot", GLib.Variant("(sa{sv})", ("", {})), Gio.DBusCallFlags.NONE, 5000, None
    )
    handle = reply.unpack()[0]
    loop = GLib.MainLoop()
    error: list[str] = []

    def response(_connection, _sender, _path, _interface, _signal, parameters, _data):
        result, details = parameters.unpack()
        if result != 0:
            error.append(f"portal response code {result}")
        else:
            uri = details.get("uri")
            if not uri:
                error.append("portal response omitted uri")
            else:
                contents = Gio.File.new_for_uri(uri).load_contents(None)[1]
                with open(FULL_SHOT, "wb") as output:
                    output.write(contents)
        loop.quit()

    bus.signal_subscribe(
        "org.freedesktop.portal.Desktop",
        "org.freedesktop.portal.Request",
        "Response",
        handle,
        None,
        Gio.DBusSignalFlags.NONE,
        response,
        None,
    )
    loop.run()
    if error:
        raise SystemExit(error[0])
    return GdkPixbuf.Pixbuf.new_from_file(FULL_SHOT)


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: portal-shot.py OUTPUT.png", file=sys.stderr)
        return 2
    destination = argv[1]

    frame = atspi_extents()
    if frame is None:
        print("refusing to capture: no Clay frame extents in the AT-SPI tree", file=sys.stderr)
        return 2

    fx, fy, fw, fh = frame
    # The compositor lists windows from every workspace, including dead ones
    # with stale geometry (observed: the same window id reported at 632, -325
    # and 1845 within one run). Only a rect that lies inside Clay's *live*
    # AT-SPI frame is trusted — the frame is stable and carries the window's
    # real position plus its invisible Wayland shadow padding.
    inside = [
        bounds
        for bounds in (clay_windows() or [])
        if all(isinstance(value, int) for value in bounds)
        and bounds[0] >= fx - TOLERANCE
        and bounds[1] >= fy - TOLERANCE
        and bounds[0] + bounds[2] <= fx + fw + TOLERANCE
        and bounds[1] + bounds[3] <= fy + fh + TOLERANCE
    ]
    bounds = inside[0] if inside else None
    if bounds:
        x, y, width, height = bounds
    else:
        # No compositor geometry to cross-check: stay strictly inside the
        # AT-SPI frame (Wayland frame extents include shadow padding).
        x, y, width, height = frame
        width -= max(1, width // 32)
        height -= max(1, height // 32)

    full = portal_full_screenshot()
    if x < 0 or y < 0 or x + width > full.get_width() or y + height > full.get_height():
        print(
            f"refusing to capture: crop {width}x{height}+{x}+{y} is outside the "
            f"{full.get_width()}x{full.get_height()} screenshot",
            file=sys.stderr,
        )
        return 2
    full.new_subpixbuf(x, y, width, height).savev(destination, "png", [], [])
    print(f"wrote {destination} {width}x{height} (AT-SPI frame {frame}, compositor {bounds})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
