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
});
