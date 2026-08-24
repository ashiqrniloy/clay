import { Compartment } from "@codemirror/state";

/**
 * One compartment per independently reconfigurable editor facet. Theme,
 * keymap, language, behavior, read-only, and decoration updates must go
 * through these — never `EditorView` recreation.
 */
export const readOnlyCompartment = new Compartment();
export const themeCompartment = new Compartment();
export const keymapCompartment = new Compartment();
export const languageCompartment = new Compartment();
export const behaviorCompartment = new Compartment();
export const decorationCompartment = new Compartment();
