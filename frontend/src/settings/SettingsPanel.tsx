import { useMemo, useState, useSyncExternalStore } from "react";
import type { ReactNode } from "react";

import {
  ClayButton,
  ClayCollapse,
  ClayDropdown,
  ClayKbd,
  ClayText,
  ClayTextField,
  recipeAttributes,
} from "../components";
import { designSystemStore, themeStore } from "../state/stores";
import {
  packageIntent,
  sduiActionPayload,
  type IntentSender,
} from "../sdui/actions";
import type { UiChoiceOption } from "../sdui/types";
import type { TypographySnapshot } from "../theme/types";

import styles from "./settings-panel.module.css";

/** Server snapshots carry no label for theme packages; derive one from the
 * short name (`@clay/theme-modus-operandi` → "Modus Operandi"). */
function choiceLabel(option: UiChoiceOption): string {
  if (option.displayName) return option.displayName;
  const short = option.specifier
    .replace("@clay/theme-", "")
    .replace("@clay/design-", "")
    .replace("@clay/", "");
  return short
    .split("-")
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(" ");
}

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
  const designSystem = useSyncExternalStore(
    designSystemStore.subscribe,
    designSystemStore.get,
  );
  const choices = theme.uiChoices;
  const themeOptions = (choices?.themes ?? []).map((option) => ({
    id: option.specifier,
    label: choiceLabel(option),
  }));
  const designSystemOptions = (choices?.designSystems ?? []).map((option) => ({
    id: option.specifier,
    label: choiceLabel(option),
  }));
  const [values, setValues] = useState(() => initialValues(theme.typography));
  // Hydrated from the persisted preference carried by the runtime snapshot;
  // local override takes over after the user picks (panel remounts on open).
  const [appearanceOverride, setAppearanceOverride] = useState<string | null>(
    null,
  );
  const appearance = appearanceOverride ?? choices?.appearance ?? "system";
  const set = (key: keyof typeof values) => (value: string) =>
    setValues((current) => ({ ...current, [key]: value }));
  const request = useMemo(() => typographyRequest(values), [values]);
  // Per-field validation: the panel used to tone every field when one was
  // invalid; the fields now carry their own message (plan 118 task E6).
  const errors = useMemo(() => typographyErrors(values), [values]);
  const fieldState = (key: keyof typeof values) =>
    errors[key] ? "error" : "none";
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
    <aside
      className={styles.panel}
      data-clay-ds="settingsPanel.panel"
      aria-label="Settings"
      onKeyDown={(event) => {
        // The panel is a slot, not a modal: Escape closes it from anywhere
        // inside it (the head also carries the keycap, as approved).
        if (event.key !== "Escape") return;
        event.stopPropagation();
        void intent("settings.close");
      }}
    >
      <header className={styles.heading}>
        <ClayText variant="title">Settings</ClayText>
        <span className={styles.headSpacer} />
        <ClayKbd>esc</ClayKbd>
        <ClayButton
          variant="muted"
          onPress={() => void intent("settings.close")}
          data-settings-close
        >
          Close
        </ClayButton>
      </header>
      <div className={styles.scroll} data-settings-scroll>
        <div className={styles.column}>
          <ClayText variant="caption" muted>
            appearance · typography · the design system in use
          </ClayText>
          <section className={styles.section} data-settings-section="theme">
            <ClayCollapse title={<Eyebrow>Theme</Eyebrow>} defaultExpanded>
              <SettingsRow label="Theme">
                <ClayDropdown
                  label="Theme"
                  options={themeOptions}
                  selectedId={theme.theme?.specifier ?? null}
                  onSelect={(id) => void intent("settings.setTheme", id)}
                />
              </SettingsRow>
              <SettingsRow label="Design system">
                <ClayDropdown
                  label="Design system"
                  options={designSystemOptions}
                  selectedId={
                    designSystem.designSystem?.specifier ?? "@clay/core"
                  }
                  onSelect={(id) => void intent("settings.setDesignSystem", id)}
                />
              </SettingsRow>
              <SettingsRow
                label="Appearance"
                note="System follows the OS: light palette on a light OS, dark on a dark one."
              >
                <ClayDropdown
                  label="Appearance"
                  options={[
                    { id: "light", label: "Light" },
                    { id: "dark", label: "Dark" },
                    { id: "system", label: "System" },
                  ]}
                  selectedId={appearance}
                  onSelect={(id) => {
                    setAppearanceOverride(id);
                    void intent("settings.setAppearance", id);
                  }}
                />
              </SettingsRow>
            </ClayCollapse>
          </section>
          <section
            className={styles.section}
            data-settings-section="typography"
          >
            <ClayCollapse title={<Eyebrow>Typography</Eyebrow>}>
              <SettingsRow label="UI families">
                <ClayTextField
                  label="UI families"
                  labelHidden
                  value={values.uiFamilies}
                  onChange={set("uiFamilies")}
                  description="Comma-separated fallback order"
                  validationState={fieldState("uiFamilies")}
                  errorMessage={errors.uiFamilies}
                />
              </SettingsRow>
              <SettingsRow label="Monospace families" note="used by the editor">
                <ClayTextField
                  label="Monospace families"
                  labelHidden
                  value={values.monospaceFamilies}
                  onChange={set("monospaceFamilies")}
                  validationState={fieldState("monospaceFamilies")}
                  errorMessage={errors.monospaceFamilies}
                />
              </SettingsRow>
              <SettingsRow label="Proportional families">
                <ClayTextField
                  label="Proportional families"
                  labelHidden
                  value={values.proportionalFamilies}
                  onChange={set("proportionalFamilies")}
                  validationState={fieldState("proportionalFamilies")}
                  errorMessage={errors.proportionalFamilies}
                />
              </SettingsRow>
              <SettingsRow label="UI size">
                <ClayTextField
                  label="UI size"
                  labelHidden
                  role="monospace"
                  value={values.uiSize}
                  onChange={set("uiSize")}
                  validationState={fieldState("uiSize")}
                  errorMessage={errors.uiSize}
                />
              </SettingsRow>
              <SettingsRow label="Monospace size">
                <ClayTextField
                  label="Monospace size"
                  labelHidden
                  role="monospace"
                  value={values.monospaceSize}
                  onChange={set("monospaceSize")}
                  validationState={fieldState("monospaceSize")}
                  errorMessage={errors.monospaceSize}
                />
              </SettingsRow>
              <SettingsRow label="Proportional size">
                <ClayTextField
                  label="Proportional size"
                  labelHidden
                  role="monospace"
                  value={values.proportionalSize}
                  onChange={set("proportionalSize")}
                  validationState={fieldState("proportionalSize")}
                  errorMessage={errors.proportionalSize}
                />
              </SettingsRow>
              <SettingsRow
                label="Hierarchy ratios"
                note="display, title, section, body, status, detail, caption"
              >
                <ClayTextField
                  label="Hierarchy ratios"
                  labelHidden
                  role="monospace"
                  value={values.hierarchy}
                  onChange={set("hierarchy")}
                  validationState={fieldState("hierarchy")}
                  errorMessage={errors.hierarchy}
                />
              </SettingsRow>
            </ClayCollapse>
          </section>
        </div>
      </div>
      <div className={styles.actions} data-settings-actions>
        <ClayText variant="caption" muted>
          {request
            ? "Sizes apply to this window only; the server validates every change again."
            : "Nothing applies until the families, sizes (6–96) and seven ratios (0–4) are valid."}
        </ClayText>
        <div className={styles.actionsRow}>
          <ClayButton
            variant="danger"
            onPress={() => void intent("settings.reset")}
          >
            Reset preferences
          </ClayButton>
          <ClayButton
            variant="primary"
            isDisabled={!request}
            onPress={() =>
              void intent("settings.setTypography", undefined, {
                typography: JSON.stringify(request),
              })
            }
          >
            Apply typography
          </ClayButton>
        </div>
      </div>
    </aside>
  );
}

/** Section eyebrow (DESIGN.md §8/§11): micro-label treatment on the caption
 *  role, so the group's heading reads as a label rather than a title. */
function Eyebrow({ children }: { children: string }) {
  return (
    <span
      className={styles.eyebrow}
      {...recipeAttributes("label", "root", "caption")}
    >
      {children}
    </span>
  );
}

/** One settings row: micro label, control, optional note. Rows are separated
 *  by the shared hairline, never framed — the settings column has no nested
 *  boxes (approved composition). */
function SettingsRow({
  label,
  note,
  children,
}: {
  label: string;
  note?: string;
  children: ReactNode;
}) {
  return (
    <div className={styles.row} data-settings-row>
      {/* Decorative: the control inside carries the same name for AT. */}
      <span className={styles.rowLabel} aria-hidden="true">
        {label}
      </span>
      <div className={styles.rowControl}>{children}</div>
      {note && (
        <ClayText variant="caption" muted className={styles.rowNote}>
          {note}
        </ClayText>
      )}
    </div>
  );
}

/** One message per invalid field, in that field's own words (plan 118 task
 * E6): the row note stays the summary, and the field says what is wrong with
 * it. Empty when every value parses. */
function typographyErrors(
  values: ReturnType<typeof initialValues>,
): Partial<Record<keyof ReturnType<typeof initialValues>, string>> {
  const errors: Partial<
    Record<keyof ReturnType<typeof initialValues>, string>
  > = {};
  const checkFamilies = (
    key: "monospaceFamilies" | "proportionalFamilies" | "uiFamilies",
  ) => {
    const list = values[key]
      .split(",")
      .map((entry) => entry.trim())
      .filter(Boolean);
    if (list.length === 0) errors[key] = "Enter at least one family name.";
  };
  checkFamilies("monospaceFamilies");
  checkFamilies("proportionalFamilies");
  checkFamilies("uiFamilies");
  const checkSize = (key: "monospaceSize" | "proportionalSize" | "uiSize") => {
    const value = Number(values[key]);
    if (!Number.isFinite(value) || value < 6 || value > 96)
      errors[key] = "Enter a size between 6 and 96.";
  };
  checkSize("monospaceSize");
  checkSize("proportionalSize");
  checkSize("uiSize");
  const hierarchy = values.hierarchy
    .split(",")
    .map((entry) => Number(entry.trim()));
  if (
    hierarchy.length !== 7 ||
    hierarchy.some(
      (scale) => !Number.isFinite(scale) || scale <= 0 || scale > 4,
    )
  )
    errors.hierarchy =
      "Enter seven ratios between 0 and 4, in order: display, title, section, body, status, detail, caption.";
  return errors;
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
  if (Object.keys(typographyErrors(values)).length > 0) return null;
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
