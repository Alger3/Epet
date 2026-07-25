import { useEffect, useRef, useState } from "react";

import { findCharacter } from "../shared/characters";
import { shouldReleasePressedState } from "../shared/runtime-state";
import { useRuntimeState } from "../shared/use-runtime-state";

export function PetOverlay() {
  const movedRef = useRef(false);
  const pointerStartRef = useRef<{ x: number; y: number } | null>(null);
  const [state, actions] = useRuntimeState();
  const [pressed, setPressed] = useState(false);
  const character = findCharacter(state.activeCharacterId);

  const finishPress = () => {
    movedRef.current = false;
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

  return (
    <main
      className={`pet-overlay pet-overlay-${character.subjectKind} ${state.paused ? "pet-paused" : ""} ${state.lastBehaviorState === "walk" ? "pet-walking" : ""} ${pressed ? "pet-pressed" : ""}`}
      onContextMenu={(event) => event.preventDefault()}
      onPointerDown={(event) => {
        if (event.button !== 0 || state.clickThrough) return;
        movedRef.current = false;
        pointerStartRef.current = { x: event.clientX, y: event.clientY };
        setPressed(true);
      }}
      onPointerMove={(event) => {
        const start = pointerStartRef.current;
        if (!start || movedRef.current) return;
        if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < 4) return;
        movedRef.current = true;
        void actions.beginDrag();
      }}
      onPointerUp={() => {
        finishPress();
      }}
      onPointerCancel={finishPress}
      onLostPointerCapture={finishPress}
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
        <img alt="" draggable={false} src={character.assetUrl} />
      </div>
    </main>
  );
}
