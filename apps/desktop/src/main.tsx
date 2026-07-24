import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import "./styles.css";
import { PetOverlay } from "./windows/PetOverlay";
import { Workshop } from "./windows/Workshop";

const params = new URLSearchParams(window.location.search);
const windowKind = params.get("window");
const isPetOverlay = windowKind === "pet-overlay";

document.documentElement.dataset.window = isPetOverlay ? "pet-overlay" : "workshop";
document.title = isPetOverlay ? "Epet 桌面角色" : "Epet 角色工坊";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Missing #root element");
}

createRoot(root).render(
  <StrictMode>{isPetOverlay ? <PetOverlay /> : <Workshop />}</StrictMode>,
);
