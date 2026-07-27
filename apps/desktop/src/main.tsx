import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@qzip/ui/styles.css";
import "./styles/globals.css";
import "./styles/home.css";
import "./styles/workspace.css";
import "./styles/settings.css";
import { App } from "./app/App";

const root = document.getElementById("root");

if (!root) {
  throw new Error("QZip root element is missing");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>
);
