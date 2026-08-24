import { invoke } from "@tauri-apps/api/core";
import { createRoot } from "react-dom/client";
import { StrictMode } from "react";

import { installInvoke } from "./lib/server";
import { App } from "./app/App";
import "./styles/global.css";

installInvoke(invoke);

const root = document.getElementById("root");
if (!root) throw new Error("missing #root mount element");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
