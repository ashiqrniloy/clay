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
setDesignSystem("@clay/design-neobrutal");

await publishTree(defineFlex({
  id: "review-design-neobrutal-light-neobrutal-root",
  direction: "row",
  children: [
    definePanel({
      id: "review-design-neobrutal-light-neobrutal-panel",
      title: "Neobrutal Design System",
      children: [
        defineStack({
          id: "review-design-neobrutal-light-neobrutal-stack",
          children: [
            defineLabel({ id: "review-design-neobrutal-light-neobrutal-label", text: "Default Neobrutal Design System" }),
            defineButton({
              id: "review-design-neobrutal-light-neobrutal-action",
              label: "Primary action",
              action: { commandId: "workspace.refresh", arguments: { force: true } },
            }),
            defineList({
              id: "review-design-neobrutal-light-neobrutal-states",
              items: [
                {
                  id: "review-design-neobrutal-light-neobrutal-enabled",
                  label: "Sharp 0px Geometry",
                  detail: "Hard 2px offset shadows",
                  action: { commandId: "workspace.refresh" },
                },
                {
                  id: "review-design-neobrutal-light-neobrutal-disabled",
                  label: "Structural 1px Borders",
                  detail: "Crisp 100ms snappy transitions",
                },
              ],
            }),
          ],
        }),
      ],
    }),
    defineEditorView({ id: "review-design-neobrutal-light-neobrutal-editor", documentId: 1, expectedVersion: 1 }),
  ],
}));
