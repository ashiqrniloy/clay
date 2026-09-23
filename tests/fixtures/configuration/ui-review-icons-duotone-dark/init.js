// Plan 112 visual/a11y review fixture (isolated capture run only).
// Duotone icon pack + core Neobrutal design system + dark theme: proves the
// duotone shade layers render (opacity 0.2 background paths). Package design
// systems are avoided: setDesignSystem("@clay/design-*") in init.js hits the
// documented plan-110 task-18 runtime deadlock (pre-existing, not icon work).

import {
  setAppearance,
  setDesignSystem,
  setIconPack,
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

setAppearance("dark");
setTheme("@clay/theme-gruvbox-material-dark");
setDesignSystem("@clay/core");
setIconPack("@clay/icons-phosphor-duotone");

await publishTree(
  defineFlex({
    id: "review-icons-duotone-dark-root",
    direction: "row",
    children: [
      definePanel({
        id: "review-icons-duotone-dark-panel",
        title: "Icon Pack Review (duotone-dark)",
        children: [
          defineStack({
            id: "review-icons-duotone-dark-stack",
            children: [
              defineLabel({
                id: "review-icons-duotone-dark-branch",
                text: "Branch: main",
                icon: "git.branch",
              }),
              defineLabel({
                id: "review-icons-duotone-dark-clean",
                text: "Status: clean",
                icon: "status.success",
              }),
              defineButton({
                id: "review-icons-duotone-dark-preview",
                label: "Toggle Preview",
                icon: "preview.toggle",
                action: { commandId: "workspace.refresh" },
              }),
              defineList({
                id: "review-icons-duotone-dark-files",
                items: [
                  {
                    id: "review-icons-duotone-dark-src",
                    label: "src/",
                    detail: "folder row",
                    icon: "file.folder",
                    action: { commandId: "workspace.refresh" },
                  },
                  {
                    id: "review-icons-duotone-dark-main",
                    label: "main.rs",
                    detail: "file row",
                    icon: "file.file",
                    action: { commandId: "workspace.refresh" },
                  },
                ],
              }),
            ],
          }),
        ],
      }),
      defineEditorView({
        id: "review-icons-duotone-dark-editor",
        documentId: 1,
        expectedVersion: 1,
      }),
    ],
  }),
);
