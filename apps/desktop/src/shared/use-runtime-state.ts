import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";

import {
  clampScale,
  DEFAULT_RUNTIME_STATE,
  type RuntimeState,
} from "./runtime-state";

const isTauriRuntime = (): boolean => "__TAURI_INTERNALS__" in window;

type RuntimeCommand =
  | "set_pet_visible"
  | "set_paused"
  | "set_active_character"
  | "set_click_through"
  | "set_always_on_top"
  | "set_autonomous_movement"
  | "set_sleep_after_minutes"
  | "reset_pet_position"
  | "adjust_pet_scale"
  | "begin_pet_drag"
  | "trigger_pet_tap"
  | "restore_pet_focus"
  | "show_workshop";

export interface RuntimeActions {
  setVisible(visible: boolean): Promise<void>;
  setActiveCharacter(characterId: string): Promise<void>;
  setPaused(paused: boolean): Promise<void>;
  setClickThrough(clickThrough: boolean): Promise<void>;
  setAlwaysOnTop(alwaysOnTop: boolean): Promise<void>;
  setAutonomousMovement(enabled: boolean): Promise<void>;
  setSleepAfterMinutes(minutes: number): Promise<void>;
  resetPosition(): Promise<void>;
  adjustScale(delta: number): Promise<void>;
  beginDrag(): Promise<void>;
  triggerTap(): Promise<void>;
  restoreFocus(): Promise<void>;
  showWorkshop(): Promise<void>;
}

export function useRuntimeState(): [RuntimeState, RuntimeActions, string | null] {
  const [state, setState] = useState<RuntimeState>(DEFAULT_RUNTIME_STATE);
  const [error, setError] = useState<string | null>(null);

  const execute = useCallback(
    async (
      command: RuntimeCommand,
      args: Record<string, unknown>,
      preview: (current: RuntimeState) => RuntimeState,
    ) => {
      setError(null);

      if (!isTauriRuntime()) {
        setState(preview);
        return;
      }

      try {
        const next = await invoke<RuntimeState>(command, args);
        setState(next);
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : String(reason));
      }
    },
    [],
  );

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | undefined;

    void invoke<RuntimeState>("get_runtime_state")
      .then((snapshot) => {
        if (!disposed) setState(snapshot);
      })
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      });

    void listen<RuntimeState>("runtime-state-changed", (event) => {
      if (!disposed) setState(event.payload);
    }).then((removeListener) => {
      if (disposed) removeListener();
      else unlisten = removeListener;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const actions: RuntimeActions = {
    setVisible: (visible) =>
      execute("set_pet_visible", { visible }, (current) => ({ ...current, visible })),
    setActiveCharacter: (characterId) =>
      execute("set_active_character", { characterId }, (current) => ({
        ...current,
        activeCharacterId: characterId,
        visible: true,
      })),
    setPaused: (paused) =>
      execute("set_paused", { paused }, (current) => ({
        ...current,
        paused,
        lastBehaviorState: "idle",
      })),
    setClickThrough: (clickThrough) =>
      execute("set_click_through", { clickThrough }, (current) => ({
        ...current,
        clickThrough,
      })),
    setAlwaysOnTop: (alwaysOnTop) =>
      execute("set_always_on_top", { alwaysOnTop }, (current) => ({
        ...current,
        alwaysOnTop,
      })),
    setAutonomousMovement: (enabled) =>
      execute("set_autonomous_movement", { enabled }, (current) => ({
        ...current,
        autonomousMovement: enabled,
        lastBehaviorState:
          !enabled && current.lastBehaviorState === "walk"
            ? "idle"
            : current.lastBehaviorState,
      })),
    setSleepAfterMinutes: (minutes) =>
      execute("set_sleep_after_minutes", { minutes }, (current) => ({
        ...current,
        sleepAfterMinutes: minutes,
      })),
    resetPosition: () => execute("reset_pet_position", {}, (current) => current),
    adjustScale: (delta) =>
      execute("adjust_pet_scale", { delta }, (current) => ({
        ...current,
        scale: clampScale(current.scale + delta),
      })),
    beginDrag: () =>
      execute("begin_pet_drag", {}, (current) => ({
        ...current,
        lastBehaviorState:
          current.lastBehaviorState === "sleep" ? "sleep" : "drag",
      })),
    triggerTap: () =>
      execute("trigger_pet_tap", {}, (current) => ({
        ...current,
        lastBehaviorState:
          current.paused || current.lastBehaviorState === "sleep"
            ? current.lastBehaviorState
            : "tap",
      })),
    restoreFocus: () => execute("restore_pet_focus", {}, (current) => current),
    showWorkshop: () => execute("show_workshop", {}, (current) => current),
  };

  return [state, actions, error];
}
