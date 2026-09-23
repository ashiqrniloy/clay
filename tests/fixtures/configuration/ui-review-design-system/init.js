import {
  setAppearance,
  setDesignSystem,
  setTheme,
} from "clay:theme";
import {
  defineButton,
  defineEditorView,
  defineFlex,
  defineLabel,
  defineList,
  definePanel,
  defineStack,
  publishTree,
} from "clay:sdui";

setTheme("@clay/theme-gruvbox-material-dark");
setAppearance("dark");
setDesignSystem("@clay/design-instrument");

await publishTree(defineFlex({
  id: "review-design-system-root",
  direction: "row",
  children: [
    definePanel({
      id: "review-design-system-panel",
      title: "Design system review",
      children: [
        defineStack({
          id: "review-design-system-stack",
          children: [
            defineLabel({ id: "review-design-system-label", text: "Shipped @clay/design-instrument activation" }),
            defineButton({
              id: "review-design-system-action",
              label: "Primary action",
              action: { commandId: "workspace.refresh", arguments: { force: true } },
            }),
            defineList({
              id: "review-design-system-states",
              items: [
                {
                  id: "review-design-system-enabled",
                  label: "Enabled state",
                  detail: "Host-owned action",
                  action: { commandId: "workspace.refresh" },
                },
                {
                  id: "review-design-system-disabled",
                  label: "Disabled state",
                  detail: "No action authority",
                },
              ],
            }),
          ],
        }),
      ],
    }),
    defineEditorView({ id: "review-design-system-editor", documentId: 1, expectedVersion: 1 }),
  ],
}));
