import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useMemo, useState } from "react";

import type { SubjectKind } from "./characters";

export type PhotoRole = "primary" | "supplemental_1" | "supplemental_2";
export type DraftStatus =
  | "editing"
  | "ready"
  | "submitting"
  | "checking"
  | "queued"
  | "generating_portrait"
  | "awaiting_confirmation"
  | "generating_actions"
  | "packaging"
  | "completed"
  | "service_unavailable"
  | "failed"
  | "cancelled";

export interface DraftPhoto {
  role: PhotoRole;
  originalName: string;
  mimeType: "image/jpeg" | "image/png";
  width: number;
  height: number;
  byteSize: number;
  sha256: string;
  cropX: number;
  cropY: number;
  cropWidth: number;
  cropHeight: number;
  qualityStatus: "accepted" | "warning";
  qualityMessages: string[];
  previewDataUrl: string;
}

export interface CreationDraft {
  id: string;
  subjectKind: SubjectKind;
  displayName: string | null;
  authorizationConfirmed: boolean;
  authorizationVersion: string | null;
  status: DraftStatus;
  snapshotVersion: number;
  progressPercent: number | null;
  serverJobId: string | null;
  serverExpiresAt: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  retryable: boolean;
  createdAt: string;
  updatedAt: string;
  photos: DraftPhoto[];
}

export interface WorkshopSnapshot {
  drafts: CreationDraft[];
  generationServiceConfigured: boolean;
}

export interface CropRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PreparedPhoto {
  bytes: number[];
  crop: CropRect;
  outputWidth: number;
  outputHeight: number;
  estimatedBytes: number;
}

const EMPTY_SNAPSHOT: WorkshopSnapshot = {
  drafts: [],
  generationServiceConfigured: false,
};
const MAX_SOURCE_BYTES = 20 * 1024 * 1024;
const MAX_SOURCE_DIMENSION = 12_000;
const MAX_SOURCE_PIXELS = 64_000_000;
const OUTPUT_DIMENSION = 1_536;

export function useWorkshopState() {
  const [snapshot, setSnapshot] = useState(EMPTY_SNAPSHOT);
  const [selectedDraftId, setSelectedDraftId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isTauri = "__TAURI_INTERNALS__" in window;

  const reload = useCallback(async () => {
    if (!isTauri) {
      setLoading(false);
      return;
    }
    setError(null);
    try {
      const next = await invoke<WorkshopSnapshot>("get_workshop_snapshot");
      setSnapshot(next);
      setSelectedDraftId((current) =>
        current && next.drafts.some((draft) => draft.id === current)
          ? current
          : next.drafts[0]?.id ?? null,
      );
    } catch (reason) {
      setError(messageFrom(reason));
    } finally {
      setLoading(false);
    }
  }, [isTauri]);

  useEffect(() => {
    void reload();
  }, [reload]);

  const run = useCallback(
    async <T,>(operation: () => Promise<T>, refresh = true): Promise<T | undefined> => {
      if (!isTauri) {
        setError("请在 Epet Windows 桌面应用中使用草稿和照片处理功能。");
        return undefined;
      }
      setBusy(true);
      setError(null);
      try {
        const result = await operation();
        if (refresh) await reload();
        return result;
      } catch (reason) {
        setError(messageFrom(reason));
        return undefined;
      } finally {
        setBusy(false);
      }
    },
    [isTauri, reload],
  );

  const selectedDraft = useMemo(
    () => snapshot.drafts.find((draft) => draft.id === selectedDraftId) ?? null,
    [selectedDraftId, snapshot.drafts],
  );

  return {
    snapshot,
    selectedDraft,
    selectedDraftId,
    selectDraft: setSelectedDraftId,
    loading,
    busy,
    error,
    clearError: () => setError(null),
    reload,
    createDraft: async (
      subjectKind: SubjectKind,
      displayName: string,
      authorizationConfirmed: boolean,
    ) => {
      const draft = await run(
        () =>
          invoke<CreationDraft>("create_character_draft", {
            subjectKind,
            displayName,
            authorizationConfirmed,
          }),
        false,
      );
      if (draft) {
        await reload();
        setSelectedDraftId(draft.id);
      }
      return draft;
    },
    savePhoto: (
      draftId: string,
      role: PhotoRole,
      originalName: string,
      prepared: PreparedPhoto,
    ) =>
      run(() =>
        invoke<CreationDraft>("save_draft_photo", {
          draftId,
          role,
          originalName,
          encodedBytes: prepared.bytes,
          cropX: prepared.crop.x,
          cropY: prepared.crop.y,
          cropWidth: prepared.crop.width,
          cropHeight: prepared.crop.height,
        }),
      ),
    removePhoto: (draftId: string, role: PhotoRole) =>
      run(() => invoke<CreationDraft>("remove_draft_photo", { draftId, role })),
    startGeneration: (draftId: string) =>
      run(() => invoke<CreationDraft>("start_draft_generation", { draftId })),
    cancelDraft: (draftId: string) =>
      run(() => invoke<CreationDraft>("cancel_character_draft", { draftId })),
    deleteDraft: async (draftId: string) => {
      const deleted = await run(
        () => invoke<boolean>("delete_character_draft", { draftId }),
        false,
      );
      if (deleted) {
        setSelectedDraftId(null);
        await reload();
      }
    },
  };
}

export async function preparePhoto(
  file: File,
  crop: CropRect,
): Promise<PreparedPhoto> {
  validateCrop(crop);
  if (file.size === 0 || file.size > MAX_SOURCE_BYTES) {
    throw new Error("原照片大小必须在 1 字节到 20 MB 之间。");
  }
  if (!/\.(jpe?g|png|webp)$/i.test(file.name)) {
    throw new Error("只支持 JPG、JPEG、PNG 或 WebP 文件。");
  }

  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file, { imageOrientation: "from-image" });
  } catch {
    throw new Error("照片内容损坏，或扩展名与真实图片内容不一致。");
  }
  try {
    if (
      bitmap.width < 256 ||
      bitmap.height < 256 ||
      bitmap.width > MAX_SOURCE_DIMENSION ||
      bitmap.height > MAX_SOURCE_DIMENSION ||
      bitmap.width * bitmap.height > MAX_SOURCE_PIXELS
    ) {
      throw new Error("照片尺寸必须至少 256×256，且不超过 12000 像素和 6400 万像素。");
    }
    const sourceX = Math.round(crop.x * bitmap.width);
    const sourceY = Math.round(crop.y * bitmap.height);
    const sourceWidth = Math.max(1, Math.round(crop.width * bitmap.width));
    const sourceHeight = Math.max(1, Math.round(crop.height * bitmap.height));
    const longest = Math.max(sourceWidth, sourceHeight);
    const scale = Math.min(1, OUTPUT_DIMENSION / longest);
    const outputWidth = Math.max(256, Math.round(sourceWidth * scale));
    const outputHeight = Math.max(256, Math.round(sourceHeight * scale));
    const canvas = document.createElement("canvas");
    canvas.width = outputWidth;
    canvas.height = outputHeight;
    const context = canvas.getContext("2d", {
      alpha: true,
      colorSpace: "srgb",
      willReadFrequently: true,
    });
    if (!context) throw new Error("当前 WebView 无法创建照片处理画布。");
    context.drawImage(
      bitmap,
      sourceX,
      sourceY,
      sourceWidth,
      sourceHeight,
      0,
      0,
      outputWidth,
      outputHeight,
    );
    const pixels = context.getImageData(0, 0, outputWidth, outputHeight);
    let transparent = false;
    let visible = 0;
    for (let index = 3; index < pixels.data.length; index += 4) {
      const alpha = pixels.data[index];
      if (alpha < 255) transparent = true;
      if (alpha > 16) visible += 1;
    }
    if (visible / (outputWidth * outputHeight) < 0.01) {
      throw new Error("裁剪结果几乎完全透明，请重新选择裁剪区域。");
    }
    const mimeType = transparent ? "image/png" : "image/jpeg";
    const blob = await canvasToBlob(canvas, mimeType, 0.9);
    if (blob.size > 8 * 1024 * 1024) {
      throw new Error("清理和压缩后的照片仍超过 8 MB，请缩小裁剪区域。");
    }
    return {
      bytes: Array.from(new Uint8Array(await blob.arrayBuffer())),
      crop,
      outputWidth,
      outputHeight,
      estimatedBytes: blob.size,
    };
  } finally {
    bitmap.close();
  }
}

export function cropFromControls(zoom: number, horizontal: number, vertical: number): CropRect {
  const safeZoom = Math.min(3, Math.max(1, zoom));
  const size = 1 / safeZoom;
  const x = (Math.min(100, Math.max(0, horizontal)) / 100) * (1 - size);
  const y = (Math.min(100, Math.max(0, vertical)) / 100) * (1 - size);
  return { x, y, width: size, height: size };
}

export function applyGenerationSnapshot(
  current: CreationDraft | null,
  incoming: CreationDraft,
): { snapshot: CreationDraft; needsFullRefresh: boolean; applied: boolean } {
  if (current && incoming.id !== current.id) {
    return { snapshot: current, needsFullRefresh: true, applied: false };
  }
  if (current && incoming.snapshotVersion <= current.snapshotVersion) {
    return { snapshot: current, needsFullRefresh: false, applied: false };
  }
  const needsFullRefresh = Boolean(
    current && incoming.snapshotVersion > current.snapshotVersion + 1,
  );
  return { snapshot: incoming, needsFullRefresh, applied: true };
}

export function reconnectDelay(attempt: number): number {
  const safeAttempt = Math.max(0, Math.min(8, Math.floor(attempt)));
  return Math.min(30_000, 1_000 * 2 ** safeAttempt);
}

function validateCrop(crop: CropRect) {
  if (
    !Object.values(crop).every(Number.isFinite) ||
    crop.x < 0 ||
    crop.y < 0 ||
    crop.width <= 0 ||
    crop.height <= 0 ||
    crop.x + crop.width > 1.000001 ||
    crop.y + crop.height > 1.000001
  ) {
    throw new Error("裁剪区域无效。");
  }
}

function canvasToBlob(canvas: HTMLCanvasElement, mimeType: string, quality: number) {
  return new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (blob) => (blob ? resolve(blob) : reject(new Error("照片重新编码失败。"))),
      mimeType,
      quality,
    );
  });
}

function messageFrom(reason: unknown): string {
  return reason instanceof Error ? reason.message : String(reason);
}
