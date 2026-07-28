import { useEffect, useMemo, useRef, useState } from "react";

import type { BehaviorState, EdgeDock } from "../shared/runtime-state";
import {
  distanceFrameIndex,
  frameDuration,
  nextFrameIndex,
  resolveSpriteAction,
  type SpriteAtlasDefinition,
} from "../shared/sprite-atlas";

interface SpriteAtlasProps {
  alt: string;
  behavior: BehaviorState;
  definition?: SpriteAtlasDefinition;
  fallbackUrl: string;
  facing?: "left" | "right";
  edgeDock?: EdgeDock | null;
  paused: boolean;
  movementDistance?: number;
}

export function SpriteAtlas({
  alt,
  behavior,
  definition,
  fallbackUrl,
  facing = "left",
  edgeDock = null,
  paused,
  movementDistance = 0,
}: SpriteAtlasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const [frameIndex, setFrameIndex] = useState(0);
  const [imageRevision, setImageRevision] = useState(0);
  const [failed, setFailed] = useState(!definition);
  const action = useMemo(
    () => (definition ? resolveSpriteAction(behavior, definition) : null),
    [behavior, definition],
  );

  useEffect(() => {
    setFrameIndex(0);
  }, [action]);

  useEffect(() => {
    if (!definition) {
      setFailed(true);
      return;
    }
    let disposed = false;
    const image = new Image();
    image.decoding = "async";
    image.onload = () => {
      if (disposed) return;
      imageRef.current = image;
      setFailed(false);
      setImageRevision((revision) => revision + 1);
    };
    image.onerror = () => {
      if (!disposed) setFailed(true);
    };
    image.src = definition.imageUrl;
    return () => {
      disposed = true;
      imageRef.current = null;
    };
  }, [definition]);

  useEffect(() => {
    if (
      !action ||
      paused ||
      action.phaseSource === "distance" ||
      action.frames.length <= 1
    ) return;
    const timeout = window.setTimeout(() => {
      setFrameIndex((current) => nextFrameIndex(current, action));
    }, frameDuration(action, frameIndex));
    return () => window.clearTimeout(timeout);
  }, [action, frameIndex, paused]);

  const renderedFrameIndex =
    action?.phaseSource === "distance"
      ? distanceFrameIndex(movementDistance, action)
      : frameIndex;
  const hasEdgeAction =
    behavior === "perch_sleep"
      ? Boolean(definition?.actions.perch_sleep ?? definition?.actions.perch)
      : Boolean(definition?.actions.perch);
  const spriteTransform =
    (behavior === "perch" || behavior === "perch_sleep") &&
    edgeDock &&
    !failed &&
    hasEdgeAction
      ? {
          bottom: undefined,
          top: "rotate(180deg)",
          left: "rotate(90deg)",
          right: "rotate(-90deg)",
        }[edgeDock]
      : facing === "right"
        ? "scaleX(-1)"
        : undefined;

  useEffect(() => {
    if (!definition || !action || failed) return;
    const frameName =
      action.frames[Math.min(renderedFrameIndex, action.frames.length - 1)];
    const frame = definition.frames[frameName];
    const image = imageRef.current;
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!frame || !image || !canvas || !context) {
      setFailed(true);
      return;
    }

    context.clearRect(0, 0, canvas.width, canvas.height);
    const scaleX = definition.canvas.width / frame.sourceSize.w;
    const scaleY = definition.canvas.height / frame.sourceSize.h;
    context.drawImage(
      image,
      frame.frame.x,
      frame.frame.y,
      frame.frame.w,
      frame.frame.h,
      frame.spriteSource.x * scaleX,
      frame.spriteSource.y * scaleY,
      frame.spriteSource.w * scaleX,
      frame.spriteSource.h * scaleY,
    );
  }, [action, definition, failed, renderedFrameIndex, imageRevision]);

  return (
    <>
      {definition && !failed ? (
        <canvas
          ref={canvasRef}
          aria-label={alt}
          className="pet-atlas-canvas"
          height={definition.canvas.height}
          role="img"
          style={{ transform: spriteTransform }}
          width={definition.canvas.width}
        />
      ) : (
        <img
          alt={alt}
          className="pet-static-fallback"
          draggable={false}
          src={fallbackUrl}
          style={{ transform: spriteTransform }}
        />
      )}
    </>
  );
}
