#!/usr/bin/env python3
"""Portal screenshot cropped to the live Clay window (plan 126 access-path steps).

    portal-shot.py OUTPUT.png [WxH+X+Y]

The crop bounds come from the computer-use-linux GNOME Shell extension (the same
source scripts/capture-ui-review.sh uses). Full-desktop captures are never
retained: only the cropped window is written.
"""

from __future__ import annotations

import json
import subprocess
import sys

import gi

gi.require_version("Gio", "2.0")
gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, Gio, GLib  # noqa: E402

DEST = "dev.avifenesh.ComputerUseLinux.WindowControl"
PATH = "/dev/avifenesh/ComputerUseLinux/WindowControl"


def clay_bounds() -> str:
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
    if out.startswith("('"):
        out = out[2:-3]
    for window in json.loads(out):
        if str(window.get("wm_class") or "") == "clay-desktop":
            bounds = window["bounds"]
            return f"{bounds['width']}x{bounds['height']}+{bounds['x']}+{bounds['y']}"
    raise SystemExit("no clay-desktop window is exposed")


def screenshot(destination: str, bounds: str) -> None:
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
                with open("/tmp/plan126-full.png", "wb") as output:
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
    pixbuf = GdkPixbuf.Pixbuf.new_from_file("/tmp/plan126-full.png")
    if bounds:
        size, x, y = bounds.split("+")
        width, height = (int(value) for value in size.split("x"))
        x, y = int(x), int(y)
        # Clamp to the actual screen: an off-screen window yields a smaller crop
        # (recorded by the caller) rather than a padded one.
        x = max(0, x)
        y = max(0, y)
        width = min(width, pixbuf.get_width() - x)
        height = min(height, pixbuf.get_height() - y)
        pixbuf = pixbuf.new_subpixbuf(x, y, width, height)
    pixbuf.savev(destination, "png", [], [])
    print(f"wrote {destination} {pixbuf.get_width()}x{pixbuf.get_height()}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    bounds = sys.argv[2] if len(sys.argv) > 2 else clay_bounds()
    screenshot(sys.argv[1], bounds)
