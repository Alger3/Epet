import { describe, expect, it } from "vitest";

import {
  clampScale,
  DEFAULT_RUNTIME_STATE,
  MAX_PET_SCALE,
  MIN_PET_SCALE,
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
