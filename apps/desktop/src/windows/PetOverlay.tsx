import { useEffect, useRef, useState } from "react";

import { shouldReleasePressedState } from "../shared/runtime-state";
import { useCharacter } from "../shared/use-character";
import { useRuntimeState } from "../shared/use-runtime-state";
import { SpriteAtlas } from "./SpriteAtlas";

export function PetOverlay() {
  const movedRef = useRef(false);
  const pointerStartRef = useRef<{ x: number; y: number } | null>(null);
  const stirTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [state, actions] = useRuntimeState();
  const [pressed, setPressed] = useState(false);
  const [sleepStir, setSleepStir] = useState(false);
  const { character } = useCharacter(state.activeCharacterId);

  const finishPress = (resetMovement = true) => {
    if (resetMovement) movedRef.current = false;
    pointerStartRef.current = null;
    setPressed(false);
  };

  useEffect(() => {
    if (shouldReleasePressedState(movedRef.current, state.lastBehaviorState)) {
      movedRef.current = false;
      pointerStartRef.current = null;
      setPressed(false);
    }
  }, [state.lastBehaviorState]);

  useEffect(
    () => () => {
      if (stirTimerRef.current !== null) clearTimeout(stirTimerRef.current);
    },
    [],
  );

  const stirSleepingPet = () => {
    if (stirTimerRef.current !== null) clearTimeout(stirTimerRef.current);
    setSleepStir(false);
    requestAnimationFrame(() => setSleepStir(true));
    stirTimerRef.current = setTimeout(() => {
      setSleepStir(false);
      stirTimerRef.current = null;
    }, 320);
  };

  if (!character) {
    return <main className="pet-overlay pet-paused" aria-label="正在加载桌宠角色" />;
  }

  return (
    <main
      className={`pet-overlay pet-overlay-${character.subjectKind} pet-behavior-${state.lastBehaviorState} ${state.paused ? "pet-paused" : ""} ${pressed ? "pet-pressed" : ""} ${sleepStir && state.lastBehaviorState === "sleep" ? "pet-sleep-stir" : ""}`}
      onContextMenu={(event) => {
        event.preventDefault();
        void actions.restoreFocus();
      }}
      onPointerDown={(event) => {
        if (event.button !== 0 || state.clickThrough) return;
        movedRef.current = false;
        pointerStartRef.current = { x: event.clientX, y: event.clientY };
        event.currentTarget.setPointerCapture(event.pointerId);
        setPressed(true);
      }}
      onPointerMove={(event) => {
        const start = pointerStartRef.current;
        if (!start || movedRef.current) return;
        if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < 4) return;
        movedRef.current = true;
        void actions.beginDrag();
      }}
      onPointerUp={(event) => {
        const moved = movedRef.current;
        finishPress();
        if (event.button !== 0) {
          void actions.restoreFocus();
        } else if (!moved) {
          if (state.lastBehaviorState === "sleep") stirSleepingPet();
          void actions.triggerTap();
        }
      }}
      onPointerCancel={() => {
        const moved = movedRef.current;
        finishPress(!moved);
        if (!moved) void actions.restoreFocus();
      }}
      onLostPointerCapture={() => {
        const moved = movedRef.current;
        finishPress(!moved);
        if (!moved) void actions.restoreFocus();
      }}
      onWheel={(event) => {
        event.preventDefault();
        void actions.adjustScale(event.deltaY > 0 ? -0.1 : 0.1);
      }}
    >
      <div
        className="pet-float"
        role="img"
        aria-label={`${character.name}，${character.subjectLabel}桌面角色`}
      >
        <SpriteAtlas
          alt=""
          behavior={state.lastBehaviorState}
          definition={character.animation}
          fallbackUrl={character.assetUrl}
          paused={state.paused}
        />
      </div>
    </main>
  );
}
