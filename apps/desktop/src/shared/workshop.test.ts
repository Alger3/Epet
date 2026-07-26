import { describe, expect, it } from "vitest";

import {
  applyGenerationSnapshot,
  cropFromControls,
  reconnectDelay,
  type CreationDraft,
} from "./workshop";

function draft(version: number, id = "draft_test"): CreationDraft {
  return {
    id,
    subjectKind: "pet_cat",
    displayName: null,
    authorizationConfirmed: false,
    authorizationVersion: null,
    status: "checking",
    snapshotVersion: version,
    progressPercent: null,
    serverJobId: null,
    serverExpiresAt: null,
    errorCode: null,
    errorMessage: null,
    retryable: false,
    createdAt: "2026-07-26T00:00:00Z",
    updatedAt: "2026-07-26T00:00:00Z",
    photos: [],
  };
}

describe("workshop state recovery", () => {
  it("ignores stale snapshots and detects version gaps", () => {
    expect(applyGenerationSnapshot(draft(4), draft(3))).toMatchObject({
      applied: false,
      needsFullRefresh: false,
    });
    expect(applyGenerationSnapshot(draft(4), draft(5))).toMatchObject({
      applied: true,
      needsFullRefresh: false,
    });
    expect(applyGenerationSnapshot(draft(4), draft(7))).toMatchObject({
      applied: true,
      needsFullRefresh: true,
    });
  });

  it("uses bounded exponential reconnect delays", () => {
    expect([0, 1, 2, 8, 99].map(reconnectDelay)).toEqual([
      1_000, 2_000, 4_000, 30_000, 30_000,
    ]);
  });

  it("converts crop controls to a bounded normalized square", () => {
    expect(cropFromControls(2, 50, 50)).toEqual({
      x: 0.25,
      y: 0.25,
      width: 0.5,
      height: 0.5,
    });
    const bounded = cropFromControls(99, -5, 120);
    expect(bounded.x).toBeCloseTo(0);
    expect(bounded.y).toBeCloseTo(2 / 3);
    expect(bounded.width).toBeCloseTo(1 / 3);
    expect(bounded.height).toBeCloseTo(1 / 3);
  });
});
