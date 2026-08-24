import { useMemo, useState, useSyncExternalStore } from "react";

import {
  ClayButton,
  ClayCollapse,
  ClayDropdown,
  ClayText,
  ClayTextField,
} from "../components";
import { themeStore } from "../state/stores";
import {
  packageIntent,
  sduiActionPayload,
  type IntentSender,
} from "../sdui/actions";
import type { TypographySnapshot } from "../theme/types";

import styles from "./settings-panel.module.css";

const THEMES = [
  ["@clay/theme-modus-operandi", "Modus Operandi"],
  ["@clay/theme-modus-vivendi", "Modus Vivendi"],
  ["@clay/theme-gruvbox-material-light", "Gruvbox Material Light"],
  ["@clay/theme-gruvbox-material-dark", "Gruvbox Material Dark"],
] as const;

function initialValues(typography: TypographySnapshot | null) {
  return {
    monospaceFamilies: typography?.monospace.families.join(", ") ?? "monospace",
    proportionalFamilies:
      typography?.proportional.families.join(", ") ?? "sans-serif",
    uiFamilies: typography?.ui.families.join(", ") ?? "system-ui",
    monospaceSize: String(typography?.monospace.size ?? 16),
    proportionalSize: String(typography?.proportional.size ?? 16),
    uiSize: String(typography?.ui.size ?? 12),
    hierarchy: typography
      ? Object.values(typography.hierarchy).join(", ")
      : "1.5, 1.1667, 1.0833, 1, 1, 0.8333, 0.75",
  };
}

export function SettingsPanel({
  uiVersion,
  send,
}: {
  uiVersion: number;
  send: IntentSender;
}) {
  const theme = useSyncExternalStore(themeStore.subscribe, themeStore.get);
  const [values, setValues] = useState(() => initialValues(theme.typography));
  const [appearance, setAppearance] = useState("system");
  const [error, setError] = useState<string | null>(null);
  const set = (key: keyof typeof values) => (value: string) =>
    setValues((current) => ({ ...current, [key]: value }));
  const request = useMemo(() => typographyRequest(values), [values]);
  const intent = (
    commandId: string,
    itemId?: string,
    arguments_?: Record<string, string>,
  ) =>
    send(
      sduiActionPayload(
        uiVersion,
        packageIntent(
          { commandId },
          `settings.react.${commandId}`,
          itemId,
          arguments_,
        ),
      ),
    );

  return (
    <aside className={styles.panel} aria-label="Settings">
      <div className={styles.heading}>
        <ClayText variant="title">Settings</ClayText>
        <ClayButton
          variant="muted"
          onPress={() => void intent("settings.close")}
        >
          Close
        </ClayButton>
      </div>
      <div className={styles.scroll}>
        <ClayCollapse title="Theme" defaultExpanded>
          <div className={styles.fields}>
            <ClayDropdown
              label="Theme"
              options={THEMES.map(([id, label]) => ({ id, label }))}
              selectedId={theme.theme?.specifier ?? null}
              onSelect={(id) => void intent("settings.setTheme", id)}
            />
            <ClayDropdown
              label="Appearance"
              options={[
                { id: "light", label: "Light" },
                { id: "dark", label: "Dark" },
                { id: "system", label: "System" },
              ]}
              selectedId={appearance}
              onSelect={(id) => {
                setAppearance(id);
                void intent("settings.setAppearance", id);
              }}
            />
          </div>
        </ClayCollapse>
        <ClayCollapse title="Typography">
          <div className={styles.fields}>
            <ClayTextField
              label="Monospace families"
              value={values.monospaceFamilies}
              onChange={set("monospaceFamilies")}
              description="Comma-separated fallback order"
            />
            <ClayTextField
              label="Proportional families"
              value={values.proportionalFamilies}
              onChange={set("proportionalFamilies")}
            />
            <ClayTextField
              label="UI families"
              value={values.uiFamilies}
              onChange={set("uiFamilies")}
            />
            <ClayTextField
              label="Monospace size"
              value={values.monospaceSize}
              onChange={set("monospaceSize")}
              validationState={request ? "none" : "error"}
            />
            <ClayTextField
              label="Proportional size"
              value={values.proportionalSize}
              onChange={set("proportionalSize")}
              validationState={request ? "none" : "error"}
            />
            <ClayTextField
              label="UI size"
              value={values.uiSize}
              onChange={set("uiSize")}
              validationState={request ? "none" : "error"}
            />
            <ClayTextField
              label="Hierarchy ratios"
              value={values.hierarchy}
              onChange={set("hierarchy")}
              description="Display, title, section, body, status, detail, caption"
              validationState={request ? "none" : "error"}
            />
          </div>
        </ClayCollapse>
      </div>
      {error && (
        <div className={styles.error} role="alert">
          <ClayText variant="status">{error}</ClayText>
        </div>
      )}
      <div className={styles.actions}>
        <ClayButton
          variant="primary"
          isDisabled={!request}
          onPress={() => {
            if (!request) {
              setError(
                "Enter non-empty families, sizes from 6 to 96, and seven ratios from 0 to 4.",
              );
              return;
            }
            setError(null);
            void intent("settings.setTypography", undefined, {
              typography: JSON.stringify(request),
            });
          }}
        >
          Apply typography
        </ClayButton>
        <ClayButton
          variant="danger"
          onPress={() => void intent("settings.reset")}
        >
          Reset preferences
        </ClayButton>
      </div>
    </aside>
  );
}

function typographyRequest(values: ReturnType<typeof initialValues>) {
  const families = (value: string) =>
    value
      .split(",")
      .map((entry) => entry.trim())
      .filter(Boolean);
  const monospace = families(values.monospaceFamilies);
  const proportional = families(values.proportionalFamilies);
  const ui = families(values.uiFamilies);
  const sizes = [
    Number(values.monospaceSize),
    Number(values.proportionalSize),
    Number(values.uiSize),
  ];
  const hierarchy = values.hierarchy
    .split(",")
    .map((entry) => Number(entry.trim()));
  if (
    [monospace, proportional, ui].some((stack) => stack.length === 0) ||
    sizes.some((size) => !Number.isFinite(size) || size < 6 || size > 96) ||
    hierarchy.length !== 7 ||
    hierarchy.some(
      (scale) => !Number.isFinite(scale) || scale <= 0 || scale > 4,
    )
  )
    return null;
  return {
    monospace: { families: monospace, size: sizes[0] },
    proportional: { families: proportional, size: sizes[1] },
    ui: { families: ui, size: sizes[2] },
    hierarchy: Object.fromEntries(
      [
        "display",
        "title",
        "section",
        "body",
        "status",
        "detail",
        "caption",
      ].map((name, index) => [name, hierarchy[index]]),
    ),
  };
}
