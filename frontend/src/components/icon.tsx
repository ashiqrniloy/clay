/**
 * Catalog icon slot (`paint_icon_slot` React projection, Plan 112 Task 7).
 *
 * Renders bounded, host-validated vector geometry from the active icon pack
 * (icon store) with the bundled Regular subset as the per-key fallback. No
 * runtime XML parsing, `dangerouslySetInnerHTML`, dynamic library loading, or
 * network: paths arrive as pre-validated d-strings and render as plain
 * `<path>` elements. Color is inherited via `currentColor`; sizing comes from
 * the `dimension.icon.size` token.
 *
 * Accessibility contract:
 * - Default (no `label`): decorative — `aria-hidden="true"`, announced by the
 *   parent control's label instead.
 * - With `label`: `role="img"` + `aria-label` (informative icon).
 * - Unknown key with no active/fallback geometry: renders an empty decorative
 *   span, never a broken glyph or a meaningless announcement.
 */

import { useSyncExternalStore, type CSSProperties } from "react";

import { iconStore } from "../state/stores";
import { FALLBACK_ICONS, type FallbackIcon } from "../icons/fallback.generated";
import type { IconGeometry } from "../state/icon-store";

import styles from "./icon.module.css";
import { recipeAttributes } from "./recipe-attributes";

export interface ClayIconProps {
  /** Semantic icon key (`action.close`, `document.save`, `vendor.custom`). */
  name: string;
  /** Set only when the icon itself is informative: renders `role="img"` with
   *  this accessible name. Omit for decorative icons inside labeled controls. */
  label?: string;
  className?: string;
  style?: CSSProperties;
}

function resolveGeometry(name: string): IconGeometry | FallbackIcon | null {
  return iconStore.getIcon(name) ?? FALLBACK_ICONS[name] ?? null;
}

/** Subscribes to active-pack changes for one semantic key. */
export function useIconGeometry(
  name: string,
): IconGeometry | FallbackIcon | null {
  return useSyncExternalStore(iconStore.subscribe, () => resolveGeometry(name));
}

function isValidPath(path: { d?: unknown; opacity?: unknown }): boolean {
  if (typeof path.d !== "string" || path.d === "") return false;
  if (
    path.opacity !== undefined &&
    (typeof path.opacity !== "number" ||
      !Number.isFinite(path.opacity) ||
      path.opacity < 0 ||
      path.opacity > 1)
  ) {
    return false;
  }
  return true;
}

export function ClayIcon({ name, label, className, style }: ClayIconProps) {
  const geometry = useIconGeometry(name);

  if (!geometry) {
    // Unknown key and no bundled fallback: keep layout stable, add no
    // announcement. Icon-capable controls replace themselves with a visible
    // text label (see ClayIconButton).
    return (
      <span
        className={`${styles.empty} ${className ?? ""}`}
        aria-hidden="true"
        {...recipeAttributes("iconSlot", "root")}
      />
    );
  }

  const paths = Array.isArray(geometry.paths) ? geometry.paths : [];
  const aria = label
    ? { role: "img" as const, "aria-label": label }
    : { "aria-hidden": true as const };

  return (
    <svg
      viewBox={geometry.viewBox.join(" ")}
      fill="currentColor"
      focusable="false"
      className={`${styles.icon} ${className ?? ""}`}
      style={style}
      data-icon-name={name}
      {...aria}
      {...recipeAttributes("iconSlot", "root")}
    >
      {paths
        .filter(isValidPath)
        .map((path, index) => (
          <path
            key={index}
            d={path.d as string}
            opacity={path.opacity as number | undefined}
          />
        ))}
    </svg>
  );
}
