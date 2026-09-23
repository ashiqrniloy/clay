#!/usr/bin/env python3
"""Numeric checks for the plan 125 launch-test captures (live, canonical config).

    python3 launch-check.py screenshots/01-rest.png screenshots/02-palette-titlebar.png 8,23

OFFSET is the capture crop origin in AX space (the live window frame sits at
8,39 on this compositor and WebKit reports AX y 23px above the crop row, so the
AX-to-crop offset is 8,23). Probe rectangles are AX extents taken from the
matching `.ax.txt` dumps:

  dialog "Commands"  44,531 1124x420   palette sheet
  entry "Message"    53,966 1106x48    composer input (field shell = +9px pad)
  footer "Agent lane" 26,946 1160x200  lane strip
  section "Session environment" 26,1122 1160x24

Checks: sheet cap/width/gap versus the lane and the composer box, veil deltas
per region, the halo profile across the sheet's top border, and the theme
canvas match.
"""
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf

rest = GdkPixbuf.Pixbuf.new_from_file(sys.argv[1])
open_shot = GdkPixbuf.Pixbuf.new_from_file(sys.argv[2])
ox, oy = (int(part) for part in sys.argv[3].split(","))

SHEET = (44, 531, 1124, 420)
LANE_INNER = 1124          # lane 1160 - 2x18px form padding
FIELD_TOP = 957            # entry 966 - 9px field padding
ENTRY = (53, 966, 1106, 48)
THEME_SHELL_BG = (40, 40, 40)  # @clay/theme-gruvbox-material-dark


def sampler(image):
    data = image.get_pixels()
    stride = image.get_rowstride()
    channels = image.get_n_channels()

    def pixel(x, y):
        index = stride * (y - oy) + (x - ox) * channels
        return (data[index], data[index + 1], data[index + 2])

    def mean(x, y, w, h):
        total = [0, 0, 0]
        count = 0
        step_x = max(1, w // 24)
        step_y = max(1, h // 16)
        for row in range(y, y + h, step_y):
            for column in range(x, x + w, step_x):
                value = pixel(column, row)
                for index in range(3):
                    total[index] += value[index]
                count += 1
        return tuple(round(component / count) for component in total)

    return pixel, mean


rest_pixel, rest_mean = sampler(rest)
open_pixel, open_mean = sampler(open_shot)
lum = lambda color: sum(color) / 3

x, y, w, h = SHEET
print("geometry")
print(f"  sheet (AX)                    {x},{y} {w}x{h}")
print(f"  height cap 420                {'OK' if h == 420 else f'UNEXPECTED {h}'}")
print(f"  width vs lane inner ({LANE_INNER})  "
      f"{'MATCH' if w == LANE_INNER else f'MISMATCH {w}'}")
print(f"  gap above field shell (y {FIELD_TOP})  {FIELD_TOP - (y + h)}px "
      f"-> {'6px OK' if FIELD_TOP - (y + h) == 6 else 'UNEXPECTED'}")
print(f"  entry Message (AX)            {ENTRY[0]},{ENTRY[1]} {ENTRY[2]}x{ENTRY[3]}")

print()
print("region means (rest | palette open | delta)")
regions = {
    "sidebar (60,300 180x340)": (60, 300, 180, 340),
    "pane (420,300 480x220)": (420, 300, 480, 220),
    "rail (1200,300 240x340)": (1200, 300, 240, 340),
    "lane foot strip (700,1124 400x18)": (700, 1124, 400, 18),
    "composer box (60,990 540x10)": (60, 990, 540, 10),
    "status bar (300,1148 600x12)": (300, 1148, 600, 12),
}
for label, box in regions.items():
    before = rest_mean(*box)
    after = open_mean(*box)
    print(f"  {label:34s} {before} | {after} | {lum(after) - lum(before):+.1f}")

print()
print("halo profile across the sheet's top border (probe x 600; border row 531)")
print("  AX y | open | rest | open-rest")
for probe_y in range(y - 7, y + 2):
    after = open_pixel(600, probe_y)
    before = rest_pixel(600, probe_y)
    print(f"  {probe_y:4d} | {after} | {before} | {lum(after) - lum(before):+.1f}")

print()
print("below the sheet's bottom edge (border row 950; a drop shadow would darken)")
for offset in (2, 4, 8, 12):
    probe = open_pixel(600, y + h - 1 + offset)
    print(f"  {offset:3d}px below: open {probe} (lum {lum(probe):5.1f})")

print()
veiled_canvas = open_mean(200, 200, 500, 120)
rest_canvas = rest_mean(200, 200, 500, 120)
print(f"canvas: rest {rest_canvas} vs gruvbox-material-dark shellBg {THEME_SHELL_BG} -> "
      f"{'MATCH' if rest_canvas == THEME_SHELL_BG else 'CHECK'}")
print(f"        veiled (palette open) {veiled_canvas} -> "
      f"delta {lum(veiled_canvas) - lum(rest_canvas):+.1f}")
