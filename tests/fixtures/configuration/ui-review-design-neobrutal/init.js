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
setDesignSystem("@clay/design-neobrutal");

await publishTree(defineFlex({
  id: "review-design-neobrutal-root",
  direction: "row",
  children: [
    definePanel({
      id: "review-design-neobrutal-panel",
      title: "Neobrutal Design System",
      children: [
        defineStack({
          id: "review-design-neobrutal-stack",
          children: [
            defineLabel({ id: "review-design-neobrutal-label", text: "Default Neobrutal Design System" }),
            defineButton({
              id: "review-design-neobrutal-action",
              label: "Primary action",
              action: { commandId: "workspace.refresh", arguments: { force: true } },
            }),
            defineList({
              id: "review-design-neobrutal-states",
              items: [
                {
                  id: "review-design-neobrutal-enabled",
                  label: "Sharp 0px Geometry",
                  detail: "Hard 2px offset shadows",
                  action: { commandId: "workspace.refresh" },
                },
                {
                  id: "review-design-neobrutal-disabled",
                  label: "Structural 1px Borders",
                  detail: "Crisp 100ms snappy transitions",
                },
              ],
            }),
          ],
        }),
      ],
    }),
    defineEditorView({ id: "review-design-neobrutal-editor", documentId: 1, expectedVersion: 1 }),
  ],
}));
