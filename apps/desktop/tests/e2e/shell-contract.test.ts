import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";

import { DatabaseSync } from "node:sqlite";
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

  it("migrates the runtime to a two-subject character library", () => {
    const migration = readFileSync(
      new URL("../../src-tauri/migrations/0003-character-library.sql", import.meta.url),
      "utf8",
    );
    expect(migration).toContain("subject_kind IN ('pet_cat', 'human_avatar')");
    expect(migration).toContain("builtin-orange-tabby");
    expect(migration).toContain("builtin-forest-guide");
    expect(migration).toContain("active_character_id");
    expect(migration).toContain("PRAGMA user_version = 3");
  });

  it("executes fresh and v2-to-v5 SQLite migration paths", () => {
    const migrationUrls = [
      "../../src-tauri/migrations/0001-runtime-state.sql",
      "../../src-tauri/migrations/0002-monitor-restoration.sql",
      "../../src-tauri/migrations/0003-character-library.sql",
      "../../src-tauri/migrations/0004-always-on-top.sql",
      "../../src-tauri/migrations/0005-autonomous-movement.sql",
    ];
    const migrations = migrationUrls.map((url) =>
      readFileSync(new URL(url, import.meta.url), "utf8"),
    );

    const fresh = new DatabaseSync(":memory:");
    migrations.forEach((migration) => fresh.exec(migration));
    const freshVersion = fresh.prepare("PRAGMA user_version").get() as { user_version: number };
    const characterCount = fresh.prepare("SELECT COUNT(*) AS count FROM characters").get() as {
      count: number;
    };
    expect(freshVersion.user_version).toBe(5);
    expect(characterCount.count).toBe(2);
    fresh.close();

    const upgrade = new DatabaseSync(":memory:");
    upgrade.exec(migrations[0]);
    upgrade.exec(migrations[1]);
    upgrade.exec(`
      INSERT INTO runtime_state (
        singleton, active_pet_id, monitor_id, x, y, scale, visible,
        click_through, paused, last_behavior_state, runtime_version, pet_logical_size
      ) VALUES (
        1, 'builtin-orange-tabby', NULL, NULL, NULL, 0.8, 1,
        0, 0, 'idle', 2, 320
      )
    `);
    upgrade.exec(migrations[2]);
    upgrade.exec(migrations[3]);
    upgrade.exec(migrations[4]);
    const migrated = upgrade
      .prepare(
        "SELECT active_pet_id, active_character_id, always_on_top, autonomous_movement FROM runtime_state WHERE singleton = 1",
      )
      .get() as {
        active_pet_id: string;
        active_character_id: string;
        always_on_top: number;
        autonomous_movement: number;
      };
    expect(migrated).toEqual({
      active_pet_id: "builtin-orange-tabby",
      active_character_id: "builtin-orange-tabby",
      always_on_top: 1,
      autonomous_movement: 0,
    });
    upgrade.close();
  });

  it("restricts character switching to the workshop and installed ids", () => {
    const commands = readFileSync(
      new URL("../../src-tauri/src/commands.rs", import.meta.url),
      "utf8",
    );
    expect(commands).toMatch(
      /pub fn set_active_character[\s\S]*?ensure_caller\(&window, &\[windows::WORKSHOP_LABEL\]\)\?/,
    );
    expect(commands).toMatch(/set_active_character[\s\S]*?character_exists/);
  });

  it("keeps the always-on-top setting behind a caller-validated Rust command", () => {
    const commands = readFileSync(
      new URL("../../src-tauri/src/commands.rs", import.meta.url),
      "utf8",
    );
    expect(commands).toMatch(
      /pub fn set_always_on_top[\s\S]*?ensure_caller\(&window, &\[windows::WORKSHOP_LABEL\]\)\?/,
    );
    expect(commands).toMatch(/set_always_on_top_internal[\s\S]*?runtime\.always_on_top/);
  });

  it("keeps autonomous movement behind a caller-validated Rust command", () => {
    const commands = readFileSync(
      new URL("../../src-tauri/src/commands.rs", import.meta.url),
      "utf8",
    );
    expect(commands).toMatch(
      /pub fn set_autonomous_movement[\s\S]*?ensure_caller\(&window, &\[windows::WORKSHOP_LABEL\]\)\?/,
    );
    expect(commands).toMatch(
      /set_autonomous_movement_internal[\s\S]*?runtime\.autonomous_movement/,
    );
  });

  it("ships the declared transparent human avatar without hash drift", () => {
    const metadata = readJson<{ file: string; sha256: string; subject_kind: string }>(
      "../../../../assets/builtin-character/metadata.json",
    );
    const asset = readFileSync(
      new URL(`../../../../assets/builtin-character/${metadata.file}`, import.meta.url),
    );
    expect(metadata.subject_kind).toBe("human_avatar");
    expect(createHash("sha256").update(asset).digest("hex")).toBe(metadata.sha256);
  });
});
