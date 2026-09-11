import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles.css";
import App from "./App";
import Preferences from "./Preferences";

const label = getCurrentWindow().label;
const Root = label === "preferences" ? Preferences : App;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Root />
  </StrictMode>
);
