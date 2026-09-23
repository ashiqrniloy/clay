// Plan 112 visual/a11y review fixture (isolated capture run only).
// Regular icon pack + core Neobrutal design system + light theme: proves
// pack differences are visible (Regular outline glyphs). Package design
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

setAppearance("light");
setTheme("@clay/theme-modus-operandi");
setDesignSystem("@clay/core");
setIconPack("@clay/icons-phosphor-regular");

await publishTree(
  defineFlex({
    id: "review-icons-regular-root",
    direction: "row",
    children: [
      definePanel({
        id: "review-icons-regular-panel",
        title: "Icon Pack Review",
        children: [
          defineStack({
            id: "review-icons-regular-stack",
            children: [
              defineLabel({
                id: "review-icons-regular-branch",
                text: "Branch: main",
                icon: "git.branch",
              }),
              defineLabel({
                id: "review-icons-regular-clean",
                text: "Status: clean",
                icon: "status.success",
              }),
              defineButton({
                id: "review-icons-regular-preview",
                label: "Toggle Preview",
                icon: "preview.toggle",
                action: { commandId: "workspace.refresh" },
              }),
              defineList({
                id: "review-icons-regular-files",
                items: [
                  {
                    id: "review-icons-regular-src",
                    label: "src/",
                    detail: "folder row",
                    icon: "file.folder",
                    action: { commandId: "workspace.refresh" },
                  },
                  {
                    id: "review-icons-regular-main",
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
        id: "review-icons-regular-editor",
        documentId: 1,
        expectedVersion: 1,
      }),
    ],
  }),
);
