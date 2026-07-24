import { Application, Assets, Sprite, Texture } from "pixi.js";
import { useEffect, useRef, useState } from "react";

import { findCharacter } from "../shared/characters";
import { useRuntimeState } from "../shared/use-runtime-state";

export function PetOverlay() {
  const hostRef = useRef<HTMLDivElement>(null);
  const movedRef = useRef(false);
  const pointerStartRef = useRef<{ x: number; y: number } | null>(null);
  const [state, actions] = useRuntimeState();
  const [pressed, setPressed] = useState(false);
  const character = findCharacter(state.activeCharacterId);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;

    let disposed = false;
    const app = new Application();

    void app.init({
      antialias: true,
      autoDensity: true,
      backgroundAlpha: 0,
      resizeTo: host,
      resolution: Math.min(window.devicePixelRatio, 2),
    }).then(async () => {
      if (disposed) return;
      host.appendChild(app.canvas);

      const texture = await Assets.load<Texture>(character.assetUrl);
      if (disposed) return;

      const sprite = new Sprite(texture);
      sprite.anchor.set(0.5, 1);
      app.stage.addChild(sprite);

      const layout = () => {
        const width = app.screen.width;
        const height = app.screen.height;
        const scale = Math.min(width / texture.width, height / texture.height) * 0.96;
        sprite.scale.set(scale);
        sprite.position.set(width / 2, height);
      };

      layout();
      app.renderer.on("resize", layout);
    });

    return () => {
      disposed = true;
      app.destroy(true, { children: true });
    };
  }, [character.assetUrl]);

  return (
    <main
      className={`pet-overlay pet-overlay-${character.subjectKind} ${state.paused ? "pet-paused" : ""} ${pressed ? "pet-pressed" : ""}`}
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
        pointerStartRef.current = null;
        setPressed(false);
      }}
      onWheel={(event) => {
        event.preventDefault();
        void actions.adjustScale(event.deltaY > 0 ? -0.1 : 0.1);
      }}
    >
      <div
        className="pet-float"
        ref={hostRef}
        role="img"
        aria-label={`${character.name}，${character.subjectLabel}桌面角色`}
      />
    </main>
  );
}
