import { Fragment, useState, type CSSProperties, type ReactNode } from "react";

import {
  ClayButton,
  ClayCollapse,
  ClayDropdown,
  ClayList,
  ClayModal,
  ClayText,
  ClayTextField,
} from "../components";
import { packageIntent, sduiActionPayload, type IntentSender } from "./actions";
import type {
  FontRole,
  PackageComponentNode,
  PackageSurface,
  SduiActionIntent,
  TextVariant,
} from "./types";

import styles from "./registry.module.css";

interface RegistryProps {
  node: PackageComponentNode;
  uiVersion: number;
  send: IntentSender;
  editorSlot?: ReactNode;
}

function token(name: string): string {
  return `var(--clay-${name.replaceAll(".", "-")})`;
}

function componentStyle(node: PackageComponentNode): CSSProperties {
  const source = node.style ?? {};
  return {
    backgroundColor: source.background ? token(source.background) : undefined,
    color: source.contentColor ? token(source.contentColor) : undefined,
    borderColor: source.borderColor ? token(source.borderColor) : undefined,
    padding: source.padding ? token(source.padding) : undefined,
    gap: source.gap ? token(source.gap) : undefined,
    minHeight: source.rowHeight ? token(source.rowHeight) : undefined,
    inset: source.inset ? token(source.inset) : undefined,
    borderRadius: source.radius ? token(source.radius) : undefined,
    opacity: source.opacity ? token(source.opacity) : undefined,
  } as CSSProperties;
}

function variant(node: PackageComponentNode): TextVariant {
  const name = node.style?.typography?.replace("typography.", "");
  return (name as TextVariant | undefined) ?? "body";
}

function role(node: PackageComponentNode): FontRole {
  return node.style?.fontRole ?? "ui";
}

function sendIntent(
  send: IntentSender,
  uiVersion: number,
  intent: SduiActionIntent,
) {
  void send(sduiActionPayload(uiVersion, intent));
}

export function PackageComponent({
  node,
  uiVersion,
  send,
  editorSlot,
}: RegistryProps) {
  const children = (node.children ?? []).map((child) => (
    <PackageComponent
      key={child.id}
      node={child}
      uiVersion={uiVersion}
      send={send}
      editorSlot={editorSlot}
    />
  ));
  const style = componentStyle(node);

  switch (node.kind) {
    case "editorView":
      return <Fragment>{editorSlot}</Fragment>;
    case "panel":
      return (
        <section
          className={styles.panel}
          style={style}
          aria-labelledby={node.title ? `${node.id}-title` : undefined}
        >
          {node.title && (
            <ClayText id={`${node.id}-title`} variant="title" role={role(node)}>
              {node.title}
            </ClayText>
          )}
          {children}
        </section>
      );
    case "label":
      return (
        <ClayText
          variant={variant(node)}
          role={role(node)}
          disabled={node.disabled}
          style={style}
        >
          {node.text ?? node.label ?? ""}
        </ClayText>
      );
    case "statusItem":
      return (
        <span role="status">
          <ClayText
            variant={variant(node) === "body" ? "status" : variant(node)}
            role={role(node)}
            disabled={node.disabled}
            style={style}
          >
            {node.text ?? node.label ?? node.title ?? ""}
          </ClayText>
        </span>
      );
    case "button":
      return (
        <ClayButton
          variant={node.style?.variant}
          isDisabled={node.disabled || !node.action}
          onPress={() => {
            if (node.action) {
              sendIntent(send, uiVersion, packageIntent(node.action, node.id));
            }
          }}
          style={style}
        >
          {node.label ?? node.title ?? "Action"}
        </ClayButton>
      );
    case "list":
      return (
        <ClayList
          ariaLabel={node.title ?? node.label ?? "Package list"}
          items={(node.items ?? []).map((item) => ({
            id: item.id,
            title: item.label,
            detail: item.detail,
            disabled: item.disabled || !item.action,
          }))}
          selectedId={(node.items ?? []).find((item) => item.selected)?.id}
          onAction={(itemId) => {
            const item = node.items?.find(
              (candidate) => candidate.id === itemId,
            );
            if (item?.action) {
              sendIntent(
                send,
                uiVersion,
                packageIntent(item.action, node.id, item.id),
              );
            }
          }}
        />
      );
    case "dropdown":
      return <PackageDropdown node={node} uiVersion={uiVersion} send={send} />;
    case "collapse":
      return (
        <div className={styles.container} style={style}>
          <ClayCollapse title={node.title ?? node.label ?? "Section"}>
            {children}
          </ClayCollapse>
        </div>
      );
    case "modal":
      return (
        <PackageModal node={node} uiVersion={uiVersion} send={send}>
          {children}
        </PackageModal>
      );
    case "textInput":
      return <PackageTextInput node={node} uiVersion={uiVersion} send={send} />;
    case "flex":
      return (
        <div
          className={node.direction === "row" ? styles.row : styles.column}
          style={style}
        >
          {children}
        </div>
      );
    case "stack":
    case "overlay":
    case "portal":
      return (
        <div className={styles.stack} style={style}>
          {children}
        </div>
      );
    case "scroll":
      return (
        <div
          className={styles.scroll}
          style={style}
          tabIndex={0}
          aria-label={node.title ?? node.label ?? "Scrollable package content"}
        >
          {children}
        </div>
      );
  }
}

function PackageDropdown({ node, uiVersion, send }: RegistryProps) {
  const initial = node.items?.find((item) => item.selected)?.id ?? null;
  const [selected, setSelected] = useState<string | null>(initial);
  return (
    <ClayDropdown
      label={node.title ?? node.label ?? "Select"}
      options={(node.items ?? []).map((item) => ({
        id: item.id,
        label: item.label,
        disabled: item.disabled || !item.action,
      }))}
      selectedId={selected}
      disabled={node.disabled}
      onSelect={(itemId) => {
        setSelected(itemId);
        const item = node.items?.find((candidate) => candidate.id === itemId);
        if (item?.action) {
          sendIntent(
            send,
            uiVersion,
            packageIntent(item.action, node.id, item.id),
          );
        }
      }}
    />
  );
}

function PackageTextInput({ node, uiVersion, send }: RegistryProps) {
  const [value, setValue] = useState("");
  const submit = () => {
    if (!node.action) return;
    sendIntent(
      send,
      uiVersion,
      packageIntent(node.action, node.id, undefined, { value, text: value }),
    );
  };
  return (
    <ClayTextField
      label={node.title ?? node.label ?? "Text"}
      value={value}
      onChange={setValue}
      validationState={node.style?.validationState}
      disabled={node.disabled}
      onSubmit={submit}
    />
  );
}

function PackageModal({
  node,
  uiVersion,
  send,
  children,
}: RegistryProps & { children: ReactNode }) {
  const [open, setOpen] = useState(true);
  return (
    <ClayModal
      title={node.title ?? node.label ?? "Package dialog"}
      open={open}
      onClose={() => {
        setOpen(false);
        if (node.action) {
          sendIntent(send, uiVersion, packageIntent(node.action, node.id));
        }
      }}
    >
      {children}
    </ClayModal>
  );
}

export function PackageSurfaceView({
  surface,
  uiVersion,
  send,
  editorSlot,
}: {
  surface: PackageSurface;
  uiVersion: number;
  send: IntentSender;
  editorSlot?: ReactNode;
}) {
  const domain =
    surface.provenance.trustDomain === "trusted"
      ? "trusted package"
      : "third-party shared runtime";
  return (
    <section className={styles.surface} data-package-surface={surface.id}>
      <ClayText variant="caption" muted>
        Provided by {surface.provenance.packageName} ({domain})
      </ClayText>
      <PackageComponent
        key={surface.component.id}
        node={surface.component}
        uiVersion={uiVersion}
        send={send}
        editorSlot={editorSlot}
      />
    </section>
  );
}
