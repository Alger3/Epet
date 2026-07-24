import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

interface TauriWindow {
  label: string;
  transparent?: boolean;
  alwaysOnTop?: boolean;
  skipTaskbar?: boolean;
  decorations?: boolean;
  resizable?: boolean;
  shadow?: boolean;
  focus?: boolean;
}

interface TauriConfig {
  app: {
    windows: TauriWindow[];
  };
}

function readJson<T>(relativeUrl: string): T {
  return JSON.parse(readFileSync(new URL(relativeUrl, import.meta.url), "utf8")) as T;
}

describe("desktop shell configuration", () => {
  it("defines separate workshop and least-privilege overlay windows", () => {
    const config = readJson<TauriConfig>("../../src-tauri/tauri.conf.json");
    const workshop = config.app.windows.find((window) => window.label === "workshop");
    const overlay = config.app.windows.find((window) => window.label === "pet-overlay");

    expect(workshop).toBeDefined();
    expect(overlay).toMatchObject({
      transparent: true,
      alwaysOnTop: true,
      skipTaskbar: true,
      decorations: false,
      resizable: false,
      shadow: false,
      focus: false,
    });
  });

  it("does not grant filesystem, shell, dialog, or network plugin permissions", () => {
    for (const capability of ["workshop", "pet-overlay"]) {
      const config = readJson<{ permissions: string[] }>(
        `../../src-tauri/capabilities/${capability}.json`,
      );
      expect(config.permissions).toEqual(["core:event:default"]);
      const forbidden = config.permissions.filter((permission) =>
        /^(fs|shell|dialog|http|autostart):/.test(permission),
      );
      expect(forbidden).toEqual([]);
    }
  });

  it("keeps autostart behind caller-validated Rust commands", () => {
    const commands = readFileSync(
      new URL("../../src-tauri/src/commands.rs", import.meta.url),
      "utf8",
    );
    expect(commands).toMatch(
      /pub fn get_autostart_enabled[\s\S]*?ensure_caller\(&window, &\[windows::WORKSHOP_LABEL\]\)\?/,
    );
    expect(commands).toMatch(
      /pub fn set_autostart_enabled[\s\S]*?ensure_caller\(&window, &\[windows::WORKSHOP_LABEL\]\)\?/,
    );
  });
});
