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
  | "set_click_through"
  | "reset_pet_position"
  | "adjust_pet_scale"
  | "begin_pet_drag"
  | "show_workshop";

export interface RuntimeActions {
  setVisible(visible: boolean): Promise<void>;
  setPaused(paused: boolean): Promise<void>;
  setClickThrough(clickThrough: boolean): Promise<void>;
  resetPosition(): Promise<void>;
  adjustScale(delta: number): Promise<void>;
  beginDrag(): Promise<void>;
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
    setPaused: (paused) =>
      execute("set_paused", { paused }, (current) => ({
        ...current,
        paused,
        lastBehaviorState: paused ? "paused" : "idle",
      })),
    setClickThrough: (clickThrough) =>
      execute("set_click_through", { clickThrough }, (current) => ({
        ...current,
        clickThrough,
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
        lastBehaviorState: "drag",
      })),
    showWorkshop: () => execute("show_workshop", {}, (current) => current),
  };

  return [state, actions, error];
}
