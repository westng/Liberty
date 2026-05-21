import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "@/app/App";
import "@/shared/styles/global.css";

const root = document.getElementById("app");

if (!root) {
  throw new Error("Missing #app root element.");
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
