import { useState, type ReactNode } from "react";

import { ClayModal } from "../components";
import { PackageComponent, PackageSurfaceView } from "../sdui/registry";
import { SettingsPanel } from "../settings/SettingsPanel";
import { SduiRenderer } from "../sdui/renderer";
import type { IntentSender } from "../sdui/actions";
import type { PackageOverlay, PackageUiSnapshot } from "../sdui/types";
import type { SduiState } from "../sdui/state";

import styles from "./package-workspace.module.css";

export function PackageWorkspace({
  sdui,
  packageUi,
  send,
  editorSlot,
  settingsOpen = false,
}: {
  sdui: SduiState | null;
  packageUi: PackageUiSnapshot | null;
  send: IntentSender;
  editorSlot: ReactNode;
  settingsOpen?: boolean;
}) {
  const settings = packageUi?.panels.find(
    (panel) => panel.provenance.packageName === "@clay/settings",
  );
  const panels = packageUi?.panels.filter(
    (panel) =>
      panel.visibility === "visible" &&
      panel.provenance.packageName !== "@clay/settings",
  );
  const panel = (slot: "left" | "right" | "top" | "bottom") => {
    const surface = panels?.find((candidate) => candidate.slot === slot);
    return surface ? (
      <PackageSurfaceView
        key={surface.id}
        surface={surface}
        uiVersion={packageUi?.version ?? 0}
        send={send}
      />
    ) : null;
  };
  const main = sdui ? (
    <SduiRenderer state={sdui} send={send} editorSlot={editorSlot} />
  ) : (
    editorSlot
  );
  const statuses = packageUi?.components.filter(
    (surface) => surface.component.kind === "statusItem",
  );

  return (
    <div className={styles.workspace} data-package-workspace>
      {panels?.some((candidate) => candidate.slot === "top") && (
        <div className={styles.top}>{panel("top")}</div>
      )}
      <div className={styles.middle}>
        {panels?.some((candidate) => candidate.slot === "left") && (
          <div className={styles.left}>{panel("left")}</div>
        )}
        <main className={styles.main}>{main}</main>
        {panels?.some((candidate) => candidate.slot === "right") && (
          <div className={styles.right}>{panel("right")}</div>
        )}
        {settingsOpen && settings && (
          <SettingsPanel uiVersion={packageUi?.version ?? 0} send={send} />
        )}
      </div>
      {panels?.some((candidate) => candidate.slot === "bottom") && (
        <div className={styles.bottom}>{panel("bottom")}</div>
      )}
      {statuses && statuses.length > 0 && (
        <footer className={styles.status} aria-label="Package status">
          {statuses.map((surface) => (
            <PackageComponent
              key={surface.id}
              node={surface.component}
              uiVersion={packageUi?.version ?? 0}
              send={send}
            />
          ))}
        </footer>
      )}
      {packageUi?.overlays.map((overlay) => (
        <PackageOverlayView
          key={overlay.id}
          overlay={overlay}
          uiVersion={packageUi.version}
          send={send}
        />
      ))}
    </div>
  );
}

function PackageOverlayView({
  overlay,
  uiVersion,
  send,
}: {
  overlay: PackageOverlay;
  uiVersion: number;
  send: IntentSender;
}) {
  const [open, setOpen] = useState(true);
  if (!open) return null;
  if (overlay.focusPolicy === "trap") {
    return (
      <ClayModal
        title={overlay.component.title ?? "Package dialog"}
        open
        onClose={() => setOpen(false)}
      >
        <PackageComponent
          node={overlay.component}
          uiVersion={uiVersion}
          send={send}
        />
      </ClayModal>
    );
  }
  const outside =
    overlay.dismissalPolicy === "outside" ||
    overlay.dismissalPolicy === "escape-or-outside";
  const escape =
    overlay.dismissalPolicy === "escape" ||
    overlay.dismissalPolicy === "escape-or-outside";
  return (
    <div
      className={styles.overlay}
      onPointerDown={(event) => {
        if (outside && event.target === event.currentTarget) setOpen(false);
      }}
      onKeyDown={(event) => {
        if (escape && event.key === "Escape") setOpen(false);
      }}
    >
      <PackageSurfaceView surface={overlay} uiVersion={uiVersion} send={send} />
    </div>
  );
}
