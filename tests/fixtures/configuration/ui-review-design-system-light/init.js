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

setTheme("@clay/theme-gruvbox-material-light");
setAppearance("light");
setDesignSystem("@clay/design-instrument");

await publishTree(defineFlex({
  id: "review-design-system-light-root",
  direction: "row",
  children: [
    definePanel({
      id: "review-design-system-light-panel",
      title: "Design system review",
      children: [
        defineStack({
          id: "review-design-system-light-stack",
          children: [
            defineLabel({ id: "review-design-system-light-label", text: "Shipped @clay/design-instrument activation" }),
            defineButton({
              id: "review-design-system-light-action",
              label: "Primary action",
              action: { commandId: "workspace.refresh", arguments: { force: true } },
            }),
            defineList({
              id: "review-design-system-light-states",
              items: [
                {
                  id: "review-design-system-light-enabled",
                  label: "Enabled state",
                  detail: "Host-owned action",
                  action: { commandId: "workspace.refresh" },
                },
                {
                  id: "review-design-system-light-disabled",
                  label: "Disabled state",
                  detail: "No action authority",
                },
              ],
            }),
          ],
        }),
      ],
    }),
    defineEditorView({ id: "review-design-system-light-editor", documentId: 1, expectedVersion: 1 }),
  ],
}));
