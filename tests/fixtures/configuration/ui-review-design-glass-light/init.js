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
setDesignSystem("@clay/design-glass");

await publishTree(defineFlex({
  id: "review-design-glass-light-glass-root",
  direction: "row",
  children: [
    definePanel({
      id: "review-design-glass-light-glass-panel",
      title: "Glass Reference Design System",
      children: [
        defineStack({
          id: "review-design-glass-light-glass-stack",
          children: [
            defineLabel({ id: "review-design-glass-light-glass-label", text: "Frosted Glass Reference System" }),
            defineButton({
              id: "review-design-glass-light-glass-action",
              label: "Primary action",
              action: { commandId: "workspace.refresh", arguments: { force: true } },
            }),
            defineList({
              id: "review-design-glass-light-glass-states",
              items: [
                {
                  id: "review-design-glass-light-glass-enabled",
                  label: "Refractive 8-16px Blur",
                  detail: "Translucent layers + inner highlight",
                  action: { commandId: "workspace.refresh" },
                },
                {
                  id: "review-design-glass-light-glass-disabled",
                  label: "6-14px Radii",
                  detail: "Solid active-theme fallback",
                },
              ],
            }),
          ],
        }),
      ],
    }),
    defineEditorView({ id: "review-design-glass-light-glass-editor", documentId: 1, expectedVersion: 1 }),
  ],
}));
