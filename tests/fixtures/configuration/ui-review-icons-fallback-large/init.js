// Plan 112 visual/a11y review fixture (isolated capture run only).
// Zero icon-pack configuration (bundled Regular fallback subset) + large
// typography + dark Neobrutal: proves icons work with zero init.js icon
// lines and that hit targets scale with increased UI typography.

import {
  setAppearance,
  setDesignSystem,
  setTheme,
  setTypography,
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
setTypography({
  monospace: { families: ["monospace"], size: 20 },
  proportional: { families: ["sans-serif"], size: 21 },
  ui: { families: ["system-ui"], size: 24 },
});
setTheme("@clay/theme-gruvbox-material-dark");
setDesignSystem("@clay/core");
await publishTree(
  defineFlex({
    id: "review-icons-fallback-large-root",
    direction: "row",
    children: [
      definePanel({
        id: "review-icons-fallback-large-panel",
        title: "Icon Pack Review (fallback-large)",
        children: [
          defineStack({
            id: "review-icons-fallback-large-stack",
            children: [
              defineLabel({
                id: "review-icons-fallback-large-branch",
                text: "Branch: main",
                icon: "git.branch",
              }),
              defineLabel({
                id: "review-icons-fallback-large-clean",
                text: "Status: clean",
                icon: "status.success",
              }),
              defineButton({
                id: "review-icons-fallback-large-preview",
                label: "Toggle Preview",
                icon: "preview.toggle",
                action: { commandId: "workspace.refresh" },
              }),
              defineList({
                id: "review-icons-fallback-large-files",
                items: [
                  {
                    id: "review-icons-fallback-large-src",
                    label: "src/",
                    detail: "folder row",
                    icon: "file.folder",
                    action: { commandId: "workspace.refresh" },
                  },
                  {
                    id: "review-icons-fallback-large-main",
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
        id: "review-icons-fallback-large-editor",
        documentId: 1,
        expectedVersion: 1,
      }),
    ],
  }),
);
