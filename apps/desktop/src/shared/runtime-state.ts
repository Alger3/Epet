export const MIN_PET_SCALE = 0.5;
export const MAX_PET_SCALE = 1.5;

export type BehaviorState = "idle" | "drag" | "paused";

export interface RuntimeState {
  activePetId: string;
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
  paused: boolean;
  lastBehaviorState: BehaviorState;
  runtimeVersion: number;
}

export const DEFAULT_RUNTIME_STATE: RuntimeState = {
  activePetId: "builtin-orange-tabby",
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
  paused: false,
  lastBehaviorState: "idle",
  runtimeVersion: 2,
};

export function clampScale(scale: number): number {
  if (!Number.isFinite(scale)) {
    return DEFAULT_RUNTIME_STATE.scale;
  }

  return Math.min(MAX_PET_SCALE, Math.max(MIN_PET_SCALE, scale));
}
