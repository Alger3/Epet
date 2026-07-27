import { describe, expect, it } from "vitest";

import {
  BUILTIN_CHARACTERS,
  DEFAULT_CHARACTER_ID,
  findCharacter,
  isSubjectKind,
} from "./characters";

describe("built-in character catalog", () => {
  it("contains one cat and one original human avatar", () => {
    expect(BUILTIN_CHARACTERS.map((character) => character.subjectKind)).toEqual([
      "pet_cat",
      "human_avatar",
    ]);
  });

  it("falls back to the safe default for unknown ids", () => {
    expect(findCharacter("missing").id).toBe(DEFAULT_CHARACTER_ID);
  });

  it("accepts only frozen MVP subject kinds", () => {
    expect(isSubjectKind("pet_cat")).toBe(true);
    expect(isSubjectKind("human_avatar")).toBe(true);
    expect(isSubjectKind("dog")).toBe(false);
  });

  it("provides the complete multi-frame baseline for both built-in characters", () => {
    for (const character of BUILTIN_CHARACTERS) {
      const animation = character.animation;
      expect(animation).toBeDefined();
      expect(Object.keys(animation?.frames ?? {})).toHaveLength(60);
      for (const actionName of [
        "idle",
        "walk",
        "sleep",
        "tap",
        "drag",
        "wake",
        "perch",
      ]) {
        expect(animation?.actions[actionName].frames.length).toBeGreaterThan(1);
      }
      expect(animation?.actions.walk.phaseSource).toBe("distance");
      expect(animation?.actions.wake.loop).toBe(false);
    }
  });
});
