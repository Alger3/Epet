import { invoke } from "@tauri-apps/api/core";

import type { CreationDraft, DraftStatus, PhotoRole } from "./workshop";

const API_BASE_URL = (
  import.meta.env.VITE_EPET_API_BASE_URL ?? "http://127.0.0.1:8000"
).replace(/\/$/, "");
const TERMINAL_STAGES = new Set(["ready", "failed", "canceled", "expired"]);

interface UploadGrant {
  upload_id: string;
  upload_url: string;
  allowed_headers: Record<string, string>;
}

interface GenerationSnapshot {
  job_id: string;
  version: number;
  stage: string;
  retryable: boolean;
  progress?: number | null;
  error?: { code: string; params: Record<string, unknown> } | null;
}

export interface ProviderSelection {
  providerMode: "configured" | "auto" | "manual";
  requestedProvider?: "mock" | "cuda" | "openvino-gpu" | "openvino-cpu";
  requestedDeviceId?: string;
}

export interface GenerationCapabilities {
  worker_online: boolean;
  configured_provider: string | null;
  unavailable_reason?: string;
  hardware: {
    computer_model: string;
    cpu: { id: string; name: string; memory_mb: number | "unknown" };
    gpus: Array<{
      id: string;
      name: string;
      vendor: string;
      memory_mb: number | "unknown";
      runtime: string;
    }>;
    system_memory_mb: number | "unknown";
    warnings: string[];
  } | null;
  automatic_plan:
    | {
        provider_id?: string;
        device_id?: string;
        estimated_speed?: string;
        warnings?: string[];
        error?: { message: string; details?: Record<string, unknown> };
      }
    | null;
  actual_plan: {
    provider_id: string;
    device_id: string;
    model_id?: string | null;
    estimated_speed: string;
  } | null;
  providers: Array<{
    provider_id: "mock" | "cuda" | "openvino-gpu" | "openvino-cpu";
    display_name: string;
    available: boolean;
    device_ids: string[];
    model_id?: string | null;
    model_downloaded: boolean;
    estimated_speed: string;
    unavailable_reason?: string | null;
    development_only: boolean;
  }>;
  models: Array<{
    model_id: string;
    provider_id: string;
    revision: string;
    downloaded: boolean;
    download_url?: string | null;
  }>;
}

interface Artifact {
  download_url: string;
  sha256: string;
}

interface InstalledCharacter {
  id: string;
}

interface DraftUpdate {
  status: DraftStatus;
  progressPercent: number | null;
  serverJobId: string | null;
  errorCode: string | null;
  errorMessage: string | null;
  retryable: boolean;
}

function key(): string {
  return crypto.randomUUID().replaceAll("-", "");
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response;
  try {
    response = await fetch(`${API_BASE_URL}${path}`, init);
  } catch {
    throw new Error("无法连接本地生成服务。请先启动 Docker 基础设施、FastAPI 和 Mock Worker。");
  }
  if (!response.ok) {
    let detail = `${response.status} ${response.statusText}`;
    try {
      const body = (await response.json()) as {
        detail?: { code?: string; title?: string } | string;
      };
      detail =
        typeof body.detail === "string"
          ? body.detail
          : body.detail?.title ?? body.detail?.code ?? detail;
    } catch {
      // Keep the HTTP status when the response is not JSON.
    }
    throw new Error(`本地生成服务请求失败：${detail}`);
  }
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

export function getGenerationCapabilities(): Promise<GenerationCapabilities> {
  return request<GenerationCapabilities>("/v1/capabilities");
}

export function requestModelDownload(modelId: string): Promise<void> {
  return request(`/v1/models/${encodeURIComponent(modelId)}/download`, {
    method: "POST",
  });
}

async function persist(draftId: string, update: DraftUpdate): Promise<CreationDraft> {
  return invoke<CreationDraft>("update_draft_generation", { draftId, update });
}

function uploadRole(role: PhotoRole): "primary" | "side" | "detail" {
  if (role === "primary") return "primary";
  return role === "supplemental_1" ? "side" : "detail";
}

async function uploadPhoto(photo: CreationDraft["photos"][number]): Promise<string> {
  const bytes = await fetch(photo.previewDataUrl).then((response) => response.blob());
  const grant = await request<UploadGrant>("/v1/uploads", {
    method: "POST",
    headers: { "Content-Type": "application/json", "Idempotency-Key": key() },
    body: JSON.stringify({
      role: uploadRole(photo.role),
      size: bytes.size,
      mime_type: photo.mimeType,
      sha256: photo.sha256,
    }),
  });
  await fetch(grant.upload_url, {
    method: "PUT",
    headers: { ...grant.allowed_headers, "Content-Type": photo.mimeType },
    body: bytes,
  }).then((response) => {
    if (!response.ok) throw new Error(`照片上传失败：HTTP ${response.status}`);
  });
  await request(`/v1/uploads/${grant.upload_id}/complete`, {
    method: "POST",
    headers: { "Idempotency-Key": key() },
  });
  return grant.upload_id;
}

function toDraftUpdate(snapshot: GenerationSnapshot): DraftUpdate {
  const statusByStage: Record<string, DraftStatus> = {
    created: "queued",
    validating: "checking",
    generating_portrait: "generating_portrait",
    awaiting_portrait_confirmation: "awaiting_confirmation",
    generating_actions: "generating_actions",
    postprocessing: "packaging",
    quality_check: "packaging",
    packaging: "packaging",
    ready: "packaging",
    failed: "failed",
    canceled: "cancelled",
    expired: "failed",
  };
  const errorMessage = snapshot.error
    ? `${snapshot.error.code}${
        snapshot.error.params.message ? `：${String(snapshot.error.params.message)}` : ""
      }`
    : null;
  return {
    status: statusByStage[snapshot.stage] ?? "queued",
    progressPercent:
      snapshot.progress == null ? null : Math.round(snapshot.progress * 100),
    serverJobId: snapshot.job_id,
    errorCode: snapshot.error?.code ?? null,
    errorMessage,
    retryable: snapshot.retryable,
  };
}

async function watchGeneration(
  jobId: string,
  onSnapshot: (snapshot: GenerationSnapshot) => Promise<void>,
): Promise<GenerationSnapshot> {
  return new Promise((resolve, reject) => {
    let settled = false;
    let latestVersion = -1;
    let chain = Promise.resolve();
    let events: EventSource;
    let poll = 0;
    let timeout = 0;
    const cleanup = () => {
      events?.close();
      window.clearInterval(poll);
      window.clearTimeout(timeout);
    };
    const fail = (reason: unknown) => {
      if (settled) return;
      settled = true;
      cleanup();
      reject(reason);
    };
    const finish = (snapshot: GenerationSnapshot) => {
      if (settled) return;
      settled = true;
      cleanup();
      resolve(snapshot);
    };
    const accept = (snapshot: GenerationSnapshot) => {
      if (snapshot.version <= latestVersion) return;
      latestVersion = snapshot.version;
      chain = chain.then(() => onSnapshot(snapshot));
      if (TERMINAL_STAGES.has(snapshot.stage)) {
        chain.then(() => finish(snapshot)).catch(fail);
      }
    };
    events = new EventSource(`${API_BASE_URL}/v1/generations/${jobId}/events`);
    events.addEventListener("snapshot", (event) => {
      accept(JSON.parse((event as MessageEvent<string>).data) as GenerationSnapshot);
    });
    poll = window.setInterval(() => {
      void request<GenerationSnapshot>(`/v1/generations/${jobId}`)
        .then(accept)
        .catch((error) => {
          fail(error);
        });
    }, 1500);
    timeout = window.setTimeout(() => {
      fail(new Error("生成任务等待超时，请稍后重试。"));
    }, 2 * 60 * 1000);
    void request<GenerationSnapshot>(`/v1/generations/${jobId}`).then(accept).catch(fail);
  });
}

export async function generateInstallAndActivate(
  draft: CreationDraft,
  onChanged: () => Promise<void>,
  selection: ProviderSelection = { providerMode: "configured" },
): Promise<void> {
  await invoke<CreationDraft>("start_draft_generation", { draftId: draft.id });
  await onChanged();
  try {
    await persist(draft.id, {
      status: "checking",
      progressPercent: 5,
      serverJobId: null,
      errorCode: null,
      errorMessage: null,
      retryable: false,
    });
    await onChanged();
    const ordered = [...draft.photos].sort((left, right) =>
      left.role.localeCompare(right.role),
    );
    const uploadIds: Record<string, string> = {};
    for (const photo of ordered) uploadIds[photo.role] = await uploadPhoto(photo);
    const job = await request<GenerationSnapshot>("/v1/generations", {
      method: "POST",
      headers: { "Content-Type": "application/json", "Idempotency-Key": key() },
      body: JSON.stringify({
        primary_upload_id: uploadIds.primary,
        additional_upload_ids: ordered
          .filter((photo) => photo.role !== "primary")
          .map((photo) => uploadIds[photo.role]),
        style_id: draft.subjectKind === "human_avatar" ? "chibi-local-mock" : "cat-local-mock",
        species: draft.subjectKind === "human_avatar" ? "human" : "cat",
        subject_kind: draft.subjectKind,
        display_name: draft.displayName ?? "自定义桌宠",
        provider_mode: selection.providerMode,
        requested_provider: selection.requestedProvider,
        requested_device_id: selection.requestedDeviceId,
      }),
    });
    await persist(draft.id, toDraftUpdate(job));
    await onChanged();
    const ready = await watchGeneration(job.job_id, async (next) => {
      await persist(draft.id, toDraftUpdate(next));
      await onChanged();
    });
    if (ready.stage !== "ready") {
      throw new Error(ready.error?.code ?? `生成任务结束于 ${ready.stage}`);
    }
    const artifact = await request<Artifact>(`/v1/generations/${job.job_id}/artifact`);
    const installed = await invoke<InstalledCharacter>("install_pet_package_from_url", {
      url: artifact.download_url,
      expectedSha256: artifact.sha256,
    });
    await invoke("set_active_character", { characterId: installed.id });
    await persist(draft.id, {
      status: "completed",
      progressPercent: 100,
      serverJobId: job.job_id,
      errorCode: null,
      errorMessage: null,
      retryable: false,
    });
    await onChanged();
  } catch (reason) {
    const message = reason instanceof Error ? reason.message : String(reason);
    await persist(draft.id, {
      status: message.includes("无法连接本地生成服务")
        ? "service_unavailable"
        : "failed",
      progressPercent: null,
      serverJobId: null,
      errorCode: "local_generation_failed",
      errorMessage: message,
      retryable: true,
    });
    await onChanged();
    throw reason;
  }
}
