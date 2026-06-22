import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./index.css";
import App from "./App";
import { api } from "./api";

// Open http/https links in the OS browser instead of navigating the webview.
document.addEventListener("click", (e) => {
  const anchor = (e.target as HTMLElement)?.closest?.("a");
  const href = anchor?.getAttribute("href");
  if (href && /^https?:\/\//i.test(href)) {
    e.preventDefault();
    void api.openExternal(href);
  }
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
