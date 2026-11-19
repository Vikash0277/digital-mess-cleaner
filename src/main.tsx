import React from "react";
import ReactDOM from "react-dom/client";
import "./styles/theme.css";
import "./styles/tailwind.css";
import { initTheme } from "./lib/theme";
initTheme();

import { App } from "./App";

const root = ReactDOM.createRoot(document.getElementById("root")!);
root.render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);