import { DEFAULT_CHARACTER_ID } from "./characters";

export const MIN_CHARACTER_SCALE = 0.5;
export const MAX_CHARACTER_SCALE = 1.5;
export const MIN_PET_SCALE = MIN_CHARACTER_SCALE;
export const MAX_PET_SCALE = MAX_CHARACTER_SCALE;

export type BehaviorState =
  | "idle"
  | "walk"
  | "sleep"
  | "tap"
  | "drag"
  | "drop";

export interface RuntimeState {
  activeCharacterId: string;
  monitorId: string | null;
  x: number | null;
  y: number | null;
  workAreaWidth: number | null;
  workAreaHeight: number | null;
  dpiScale: number | null;
  petLogicalSize: number;
  footAnchorX: number | null;
  footAnchorY: number | null;
  scale: number;
  visible: boolean;
  clickThrough: boolean;
  alwaysOnTop: boolean;
  autonomousMovement: boolean;
  paused: boolean;
  lastBehaviorState: BehaviorState;
  diagnostic: string | null;
  runtimeVersion: number;
}

export const DEFAULT_RUNTIME_STATE: RuntimeState = {
  activeCharacterId: DEFAULT_CHARACTER_ID,
  monitorId: null,
  x: null,
  y: null,
  workAreaWidth: null,
  workAreaHeight: null,
  dpiScale: null,
  petLogicalSize: 320,
  footAnchorX: null,
  footAnchorY: null,
  scale: 0.8,
  visible: true,
  clickThrough: false,
  alwaysOnTop: true,
  autonomousMovement: false,
  paused: false,
  lastBehaviorState: "idle",
  diagnostic: null,
  runtimeVersion: 5,
};

export function clampScale(scale: number): number {
  if (!Number.isFinite(scale)) {
    return DEFAULT_RUNTIME_STATE.scale;
  }

  return Math.min(MAX_CHARACTER_SCALE, Math.max(MIN_CHARACTER_SCALE, scale));
}

export function shouldReleasePressedState(
  moved: boolean,
  behaviorState: BehaviorState,
): boolean {
  return moved && behaviorState !== "drag";
}
