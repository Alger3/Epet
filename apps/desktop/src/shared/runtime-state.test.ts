import { describe, expect, it } from "vitest";

import {
  clampScale,
  DEFAULT_RUNTIME_STATE,
  MAX_PET_SCALE,
  MIN_PET_SCALE,
  resolveVisualBehavior,
  shouldReleasePressedState,
} from "./runtime-state";

describe("clampScale", () => {
  it("keeps values inside the supported range", () => {
    expect(clampScale(0.9)).toBe(0.9);
  });

  it("clamps both boundaries", () => {
    expect(clampScale(-10)).toBe(MIN_PET_SCALE);
    expect(clampScale(10)).toBe(MAX_PET_SCALE);
  });

  it("returns the safe default for non-finite input", () => {
    expect(clampScale(Number.NaN)).toBe(DEFAULT_RUNTIME_STATE.scale);
  });
});

describe("resolveVisualBehavior", () => {
  it("combines sleep and edge docking without changing runtime sleep state", () => {
    expect(resolveVisualBehavior("sleep", "bottom")).toBe("perch_sleep");
    expect(resolveVisualBehavior("sleep", null)).toBe("sleep");
  });

  it("returns an edge wake to the awake perch pose", () => {
    expect(resolveVisualBehavior("wake", "left")).toBe("perch");
  });
});

describe("shouldReleasePressedState", () => {
  it("keeps the pressed feedback while native dragging is active", () => {
    expect(shouldReleasePressedState(true, "drag")).toBe(false);
  });

  it("releases pressed feedback when dragging settles or is interrupted", () => {
    expect(shouldReleasePressedState(true, "idle")).toBe(true);
    expect(shouldReleasePressedState(true, "drop")).toBe(true);
    expect(shouldReleasePressedState(true, "walk")).toBe(true);
  });

  it("does not alter a normal non-drag pointer interaction", () => {
    expect(shouldReleasePressedState(false, "idle")).toBe(false);
  });
});
