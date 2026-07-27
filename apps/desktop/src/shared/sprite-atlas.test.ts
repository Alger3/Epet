import { describe, expect, it } from "vitest";

import {
  distanceFrameIndex,
  frameDuration,
  nextFrameIndex,
  resolveSpriteAction,
  type SpriteAtlasDefinition,
} from "./sprite-atlas";

const definition: SpriteAtlasDefinition = {
  imageUrl: "atlas.png",
  canvas: { width: 64, height: 64 },
  frames: {},
  actions: {
    idle: {
      frames: ["idle_000", "idle_001"],
      frameDurationMs: [80, 120],
      loop: true,
    },
    tap: {
      frames: ["tap_000", "tap_001"],
      frameDurationMs: [50, 50],
      loop: false,
      fallback: "idle",
    },
  },
};

describe("sprite atlas actions", () => {
  it("selects requested actions and falls back to idle", () => {
    expect(resolveSpriteAction("tap", definition)).toBe(definition.actions.tap);
    expect(resolveSpriteAction("walk", definition)).toBe(definition.actions.idle);
    expect(resolveSpriteAction("wake", definition)).toBe(definition.actions.idle);
  });

  it("loops idle and holds the last non-looping frame", () => {
    expect(nextFrameIndex(0, definition.actions.idle)).toBe(1);
    expect(nextFrameIndex(1, definition.actions.idle)).toBe(0);
    expect(nextFrameIndex(1, definition.actions.tap)).toBe(1);
  });

  it("uses a safe duration for malformed runtime data", () => {
    expect(frameDuration(definition.actions.idle, 0)).toBe(80);
    expect(frameDuration(definition.actions.idle, 99)).toBe(100);
  });

  it("selects walk frames from accumulated distance without foot sliding", () => {
    const walk = {
      frames: ["walk_000", "walk_001", "walk_002", "walk_003"],
      frameDurationMs: [90, 90, 90, 90],
      loop: true,
      phaseSource: "distance" as const,
      strideLength: 40,
    };
    expect(distanceFrameIndex(0, walk)).toBe(0);
    expect(distanceFrameIndex(10, walk)).toBe(1);
    expect(distanceFrameIndex(39, walk)).toBe(3);
    expect(distanceFrameIndex(40, walk)).toBe(0);
  });
});
