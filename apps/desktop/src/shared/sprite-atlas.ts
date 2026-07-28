import type { BehaviorState } from "./runtime-state";

export interface SpriteRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface SpriteFrame {
  frame: SpriteRect;
  sourceSize: { w: number; h: number };
  spriteSource: SpriteRect;
}

export interface SpriteAction {
  frames: readonly string[];
  frameDurationMs: readonly number[];
  loop: boolean;
  fallback?: string | null;
  phaseSource?: "time" | "distance";
  strideLength?: number | null;
}

export interface SpriteAtlasDefinition {
  imageUrl: string;
  canvas: { width: number; height: number };
  frames: Readonly<Record<string, SpriteFrame>>;
  actions: Readonly<Record<string, SpriteAction>>;
}

export function resolveSpriteAction(
  behavior: BehaviorState,
  definition: SpriteAtlasDefinition,
): SpriteAction | null {
  const requested = definition.actions[behavior];
  if (requested) return requested;
  if (behavior === "perch_sleep") {
    return (
      definition.actions.perch ??
      definition.actions.sleep ??
      definition.actions.idle ??
      null
    );
  }
  return definition.actions.idle ?? null;
}

export function nextFrameIndex(
  current: number,
  action: SpriteAction,
): number {
  if (action.frames.length === 0) return 0;
  if (current + 1 < action.frames.length) return current + 1;
  return action.loop ? 0 : current;
}

export function frameDuration(action: SpriteAction, index: number): number {
  const duration = action.frameDurationMs[index];
  return Number.isFinite(duration) && duration >= 16 ? duration : 100;
}

export function distanceFrameIndex(
  distance: number,
  action: SpriteAction,
): number {
  if (action.frames.length === 0) return 0;
  const stride =
    Number.isFinite(action.strideLength) && (action.strideLength ?? 0) > 0
      ? action.strideLength!
      : 48;
  const safeDistance = Number.isFinite(distance) ? Math.max(0, distance) : 0;
  const phase = (safeDistance / stride) % 1;
  return Math.min(
    action.frames.length - 1,
    Math.floor(phase * action.frames.length),
  );
}
