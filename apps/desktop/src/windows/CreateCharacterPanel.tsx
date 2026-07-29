import { useEffect, useMemo, useState } from "react";

import type { SubjectKind } from "../shared/characters";
import {
  getGenerationCapabilities,
  getPortraitPreview,
  requestCapabilityProbe,
  requestModelDownload,
  type GenerationCapabilities,
  type PortraitPreview,
  type ProviderSelection,
} from "../shared/generation-service";
import {
  cropFromControls,
  preparePhoto,
  type CreationDraft,
  type PhotoRole,
  useWorkshopState,
} from "../shared/workshop";

const PHOTO_ROLES: readonly { role: PhotoRole; label: string; description: string }[] = [
  { role: "primary", label: "主照片", description: "必选，清晰展示主体主要外观。" },
  {
    role: "supplemental_1",
    label: "补充照片 1",
    description: "可选，补充侧面、发型、服装或花纹。",
  },
  {
    role: "supplemental_2",
    label: "补充照片 2",
    description: "可选，补充尾巴、细节或另一角度。",
  },
];

const GENERATION_STAGES = [
  ["checking", "检查照片"],
  ["queued", "等待队列"],
  ["generating_portrait", "生成标准形象"],
  ["awaiting_confirmation", "等待确认"],
  ["generating_actions", "生成动作"],
  ["packaging", "安全打包"],
  ["completed", "完成"],
] as const;

interface PendingPhoto {
  file: File;
  role: PhotoRole;
  previewUrl: string;
}

export function CreateCharacterPanel() {
  const workshop = useWorkshopState();
  const [authorizationConfirmed, setAuthorizationConfirmed] = useState(false);
  const [pending, setPending] = useState<PendingPhoto | null>(null);
  const [zoom, setZoom] = useState(1);
  const [horizontal, setHorizontal] = useState(50);
  const [vertical, setVertical] = useState(50);
  const [localError, setLocalError] = useState<string | null>(null);
  const [capabilities, setCapabilities] = useState<GenerationCapabilities | null>(null);
  const [capabilityError, setCapabilityError] = useState<string | null>(null);
  const [capabilityRefresh, setCapabilityRefresh] = useState(0);
  const [providerSelection, setProviderSelection] = useState<ProviderSelection>({
    providerMode: "configured",
  });
  const draft = workshop.selectedDraft;

  useEffect(() => {
    if (!draft) return;
    let active = true;
    void getGenerationCapabilities()
      .then((value) => {
        if (active) {
          setCapabilities(value);
          setCapabilityError(null);
        }
      })
      .catch((reason) => {
        if (active) setCapabilityError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      active = false;
    };
  }, [draft?.id, capabilityRefresh]);

  useEffect(
    () => () => {
      if (pending) URL.revokeObjectURL(pending.previewUrl);
    },
    [pending],
  );

  const clearPending = () => {
    if (pending) URL.revokeObjectURL(pending.previewUrl);
    setPending(null);
    setZoom(1);
    setHorizontal(50);
    setVertical(50);
  };

  const createDraft = async (subjectKind: SubjectKind, displayName: string) => {
    setLocalError(null);
    await workshop.createDraft(
      subjectKind,
      displayName,
      subjectKind === "human_avatar" && authorizationConfirmed,
    );
  };

  const selectPhoto = (role: PhotoRole, file: File | undefined) => {
    if (!file) return;
    setLocalError(null);
    clearPending();
    setPending({ file, role, previewUrl: URL.createObjectURL(file) });
  };

  const savePending = async () => {
    if (!pending || !draft) return;
    setLocalError(null);
    try {
      const prepared = await preparePhoto(
        pending.file,
        cropFromControls(zoom, horizontal, vertical),
      );
      const saved = await workshop.savePhoto(
        draft.id,
        pending.role,
        pending.file.name,
        prepared,
      );
      if (saved) clearPending();
    } catch (reason) {
      setLocalError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  if (workshop.loading) {
    return <section className="create-page empty-library">正在恢复本地草稿…</section>;
  }

  if (!draft) {
    return (
      <SubjectSelection
        authorizationConfirmed={authorizationConfirmed}
        busy={workshop.busy}
        drafts={workshop.snapshot.drafts}
        error={workshop.error}
        onAuthorizationChange={setAuthorizationConfirmed}
        onCreate={createDraft}
        onDelete={(draftId) => void workshop.deleteDraft(draftId)}
        onResume={workshop.selectDraft}
      />
    );
  }

  return (
    <section className="create-page" aria-labelledby="draft-title">
      <div className="draft-heading">
        <div>
          <p className="eyebrow">
            本地草稿 · {draft.subjectKind === "pet_cat" ? "猫咪" : "Q 版人物"}
          </p>
          <h2 id="draft-title">
            {draft.displayName ??
              (draft.status === "service_unavailable" ? "草稿已安全保存" : "准备生成照片")}
          </h2>
          <p className="draft-id">草稿 {draft.id}</p>
        </div>
        <button
          className="secondary-button compact-button"
          onClick={() => {
            clearPending();
            workshop.selectDraft(null);
          }}
          type="button"
        >
          返回草稿列表
        </button>
      </div>

      {workshop.error || localError ? (
        <div className="error-banner" role="alert">{localError ?? workshop.error}</div>
      ) : null}

      {draft.subjectKind === "human_avatar" ? (
        <div className="consent-summary">
          <strong>授权声明已记录</strong>
          <span>
            {draft.authorizationVersion} · 仅本人或已获明确授权的成年人；禁止未成年人、公众人物模仿、多人合照和未授权第三方。
          </span>
        </div>
      ) : null}

      <div className="photo-workspace">
        <div className="photo-role-list">
          {PHOTO_ROLES.map(({ role, label, description }) => {
            const photo = draft.photos.find((item) => item.role === role);
            return (
              <article className={`photo-slot ${photo ? "photo-slot-filled" : ""}`} key={role}>
                {photo ? (
                  <img alt={`${label}裁剪结果`} src={photo.previewDataUrl} />
                ) : (
                  <div className="photo-placeholder" aria-hidden="true">＋</div>
                )}
                <div>
                  <strong>{label}</strong>
                  <small>{description}</small>
                  {photo ? (
                    <>
                      <span>{photo.width}×{photo.height} · {formatBytes(photo.byteSize)}</span>
                      <span className={`quality-${photo.qualityStatus}`}>
                        {photo.qualityStatus === "accepted" ? "基础质量通过" : "建议检查"}
                      </span>
                    </>
                  ) : null}
                </div>
                <div className="photo-slot-actions">
                  <label className="secondary-button compact-button">
                    {photo ? "替换" : "选择"}
                    <input
                      accept=".jpg,.jpeg,.png,.webp,image/jpeg,image/png,image/webp"
                      disabled={workshop.busy}
                      onChange={(event) => {
                        selectPhoto(role, event.target.files?.[0]);
                        event.currentTarget.value = "";
                      }}
                      type="file"
                    />
                  </label>
                  {photo ? (
                    <button
                      className="danger-link"
                      disabled={workshop.busy}
                      onClick={() => void workshop.removePhoto(draft.id, role)}
                      type="button"
                    >
                      删除
                    </button>
                  ) : null}
                </div>
                {photo?.qualityMessages.length ? (
                  <ul className="quality-messages">
                    {photo.qualityMessages.map((message) => <li key={message}>{message}</li>)}
                  </ul>
                ) : null}
              </article>
            );
          })}
        </div>

        {pending ? (
          <div className="crop-editor">
            <div
              aria-label="裁剪预览"
              className="crop-preview"
              role="img"
              style={{
                backgroundImage: `url("${pending.previewUrl}")`,
                backgroundPosition: `${horizontal}% ${vertical}%`,
                backgroundSize: `${zoom * 100}%`,
              }}
            />
            <div>
              <p className="eyebrow">本地裁剪与元数据清理</p>
              <h3>{pending.file.name}</h3>
              <p>
                浏览器解码时应用 EXIF 方向；确认后重新绘制、最长边压缩到 1536px，再由 Rust 重新解码和编码，原 EXIF/GPS 不会进入上传副本。
              </p>
            </div>
            <label>
              <span>缩放 {zoom.toFixed(1)}×</span>
              <input
                max={3}
                min={1}
                onChange={(event) => setZoom(Number(event.target.value))}
                step={0.1}
                type="range"
                value={zoom}
              />
            </label>
            <label>
              <span>水平位置</span>
              <input
                max={100}
                min={0}
                onChange={(event) => setHorizontal(Number(event.target.value))}
                type="range"
                value={horizontal}
              />
            </label>
            <label>
              <span>垂直位置</span>
              <input
                max={100}
                min={0}
                onChange={(event) => setVertical(Number(event.target.value))}
                type="range"
                value={vertical}
              />
            </label>
            <div className="crop-actions">
              <button className="secondary-button" onClick={clearPending} type="button">取消</button>
              <button
                className="primary-button"
                disabled={workshop.busy}
                onClick={() => void savePending()}
                type="button"
              >
                {workshop.busy ? "正在清理和校验…" : "确认裁剪"}
              </button>
            </div>
          </div>
        ) : (
          <DraftProgress
            draft={draft}
            onConfirm={() => void workshop.confirmPortrait(draft.id)}
            onResume={() => void workshop.resumeGenerationInstall(draft.id)}
          />
        )}
      </div>

      <ProviderPanel
        capabilities={capabilities}
        error={capabilityError}
        selection={providerSelection}
        onChange={setProviderSelection}
        onRefresh={() => {
          void requestCapabilityProbe()
            .then(() => {
              window.setTimeout(
                () => setCapabilityRefresh((value) => value + 1),
                1200,
              );
            })
            .catch((reason) =>
              setCapabilityError(reason instanceof Error ? reason.message : String(reason)),
            );
        }}
      />

      <div className="draft-actions">
        <button
          className="primary-button"
          disabled={
            !draft.photos.some((photo) => photo.role === "primary") ||
            workshop.busy ||
            draft.status === "awaiting_confirmation"
          }
          onClick={() => void workshop.startGeneration(draft.id, providerSelection)}
          type="button"
        >
          {draft.status === "service_unavailable" ? "重新检查生成服务" : "确认照片并开始生成"}
        </button>
        <button
          className="secondary-button"
          disabled={workshop.busy || draft.status === "cancelled"}
          onClick={() => void workshop.cancelDraft(draft.id)}
          type="button"
        >
          取消草稿
        </button>
        <button
          className="danger-link"
          disabled={workshop.busy}
          onClick={() => {
            if (window.confirm("永久删除此本地草稿及清理后的照片副本？")) {
              void workshop.deleteDraft(draft.id);
            }
          }}
          type="button"
        >
          删除本地草稿
        </button>
      </div>
    </section>
  );
}

function ProviderPanel({
  capabilities,
  error,
  selection,
  onChange,
  onRefresh,
}: {
  capabilities: GenerationCapabilities | null;
  error: string | null;
  selection: ProviderSelection;
  onChange(value: ProviderSelection): void;
  onRefresh(): void;
}) {
  const selectedProvider =
    selection.requestedProvider ??
    (selection.providerMode === "configured" ? capabilities?.configured_provider : undefined);
  const selectedCapability = capabilities?.providers.find(
    (provider) => provider.provider_id === selectedProvider,
  );
  const selectedModel = capabilities?.models.find(
    (model) => model.model_id === selectedCapability?.model_id,
  );
  const speedLabel = (speed?: string) =>
    ({ fast: "较快", medium: "中等", slow: "较慢", unknown: "未知" })[speed ?? "unknown"];
  const openvino = capabilities?.runtime_probes?.openvino;

  return (
    <section className="provider-panel" aria-labelledby="provider-title">
      <div className="provider-heading">
        <div>
          <p className="eyebrow">本机生成能力</p>
          <h3 id="provider-title">设备与运行方式</h3>
        </div>
        <button
          className="secondary-button compact-button"
          onClick={onRefresh}
          type="button"
        >
          重新检测
        </button>
      </div>
      {error ? <p className="availability-note">{error}</p> : null}
      {!capabilities ? (
        <p>正在读取 Worker 的硬件探测结果…</p>
      ) : !capabilities.worker_online || !capabilities.hardware ? (
        <p className="availability-note">
          {capabilities.unavailable_reason ?? "Worker 不在线，无法读取硬件能力。"}
        </p>
      ) : (
        <>
          <div className="hardware-summary">
            <span><strong>电脑</strong>{capabilities.hardware.computer_model}</span>
            <span><strong>CPU</strong>{capabilities.hardware.cpu.name}</span>
            <span>
              <strong>GPU</strong>
              {capabilities.hardware.gpus.length
                ? capabilities.hardware.gpus.map((gpu) => gpu.name).join(" / ")
                : "unknown"}
            </span>
          </div>
          {openvino ? (
            <div className="runtime-probe" aria-label="OpenVINO 运行探针">
              <div>
                <strong>OpenVINO</strong>
                <span>{openvino.runtime_version}</span>
              </div>
              <ProbeState label="运行时" passed={openvino.runtime_available} />
              <ProbeState label="GPU 编译" passed={openvino.compile_verified} />
              <ProbeState label="测试推理" passed={openvino.inference_verified} />
              <small>
                设备：{openvino.target_device} · {openvino.full_device_name}
              </small>
              {openvino.compile_verified ? (
                <small>
                  编译 {openvino.compile_time_ms} ms · 推理 {openvino.inference_time_ms} ms ·
                  精度 {openvino.supported_precisions.join(", ") || "unknown"}
                </small>
              ) : null}
              <small>
                架构：{openvino.device_architecture} · 驱动：{openvino.driver_version}
              </small>
              {openvino.error_message ? (
                <small className="probe-error">
                  {openvino.error_code}：{openvino.error_message}
                </small>
              ) : null}
            </div>
          ) : null}
          <div className="provider-mode" role="group" aria-label="Provider 选择方式">
            <button
              aria-pressed={selection.providerMode === "auto"}
              onClick={() => onChange({ providerMode: "auto" })}
              type="button"
            >
              自动选择
            </button>
            <button
              aria-pressed={selection.providerMode !== "auto"}
              onClick={() =>
                onChange({
                  providerMode: "manual",
                  requestedProvider:
                    (capabilities.configured_provider as ProviderSelection["requestedProvider"]) ??
                    "mock",
                })
              }
              type="button"
            >
              手动选择
            </button>
          </div>
          {selection.providerMode === "auto" ? (
            <div className="provider-result">
              <strong>自动选择结果</strong>
              <span>
                {capabilities.automatic_plan?.provider_id ??
                  (capabilities.automatic_plan?.error
                    ? "暂无可用的真实推理 Provider"
                    : "等待 Planner 返回结果")}
              </span>
            </div>
          ) : (
            <label className="field-stack">
              <span>实际 Provider</span>
              <select
                onChange={(event) =>
                  onChange({
                    providerMode: "manual",
                    requestedProvider: event.target
                      .value as ProviderSelection["requestedProvider"],
                  })
                }
                value={selectedProvider ?? "mock"}
              >
                {capabilities.providers.map((provider) => (
                  <option
                    disabled={!provider.available}
                    key={provider.provider_id}
                    value={provider.provider_id}
                  >
                    {provider.display_name}
                    {!provider.available ? "（不可用）" : ""}
                  </option>
                ))}
              </select>
            </label>
          )}
          <div className="provider-result">
            <span>
              模型：
              {selectedCapability?.model_downloaded ? "已下载 / 内置" : "未下载"}
            </span>
            <span>预计速度：{speedLabel(selectedCapability?.estimated_speed)}</span>
            {selectedCapability?.provider_id === "openvino-cpu" ? (
              <strong className="cpu-warning">CPU 可以运行，但生成时间可能明显更长。</strong>
            ) : null}
            {selectedCapability?.unavailable_reason ? (
              <small>不可用原因：{selectedCapability.unavailable_reason}</small>
            ) : null}
            {selectedCapability?.model_id &&
            !selectedCapability.model_downloaded &&
            (selectedModel?.download_url ||
              selectedCapability.model_id === "epet-portrait-openvino-v1") ? (
              <button
                className="secondary-button compact-button"
                onClick={() => {
                  void requestModelDownload(selectedCapability.model_id!).then(() => {
                    window.setTimeout(onRefresh, 800);
                  });
                }}
                type="button"
              >
                {selectedCapability.model_id === "epet-portrait-openvino-v1"
                  ? "准备 OpenVINO FP16 模型"
                  : "下载所需模型"}
              </button>
            ) : selectedCapability?.model_id && !selectedCapability.model_downloaded ? (
              <small>模型下载源尚未配置；当前不能把该 Provider 标记为可用。</small>
            ) : null}
            {capabilities.actual_plan ? (
              <small>
                最近实际运行：{capabilities.actual_plan.provider_id} /{" "}
                {capabilities.actual_plan.device_id}
              </small>
            ) : null}
          </div>
        </>
      )}
    </section>
  );
}

function ProbeState({ label, passed }: { label: string; passed: boolean }) {
  return (
    <span className={passed ? "probe-state probe-state-ok" : "probe-state probe-state-off"}>
      {label}：{passed ? "通过" : "未通过"}
    </span>
  );
}

function SubjectSelection({
  authorizationConfirmed,
  busy,
  drafts,
  error,
  onAuthorizationChange,
  onCreate,
  onDelete,
  onResume,
}: {
  authorizationConfirmed: boolean;
  busy: boolean;
  drafts: CreationDraft[];
  error: string | null;
  onAuthorizationChange(value: boolean): void;
  onCreate(subjectKind: SubjectKind, displayName: string): Promise<void>;
  onDelete(draftId: string): void;
  onResume(draftId: string): void;
}) {
  const [selectedSubject, setSelectedSubject] = useState<SubjectKind>("pet_cat");
  const [draftName, setDraftName] = useState("");
  const normalizedDraftName = draftName.trim();

  return (
    <section className="create-page" aria-labelledby="create-title">
      <div>
        <p className="eyebrow">创建角色</p>
        <h2 id="create-title">先选择主体类型</h2>
        <p>创建后主体类型会锁定；草稿、授权和照片状态写入 SQLite，可在重启后恢复。</p>
      </div>
      {error ? <div className="error-banner" role="alert">{error}</div> : null}
      <div aria-label="桌宠主体类型" className="subject-choice-grid" role="group">
        <button
          aria-pressed={selectedSubject === "pet_cat"}
          className={`subject-choice-card ${
            selectedSubject === "pet_cat" ? "subject-choice-card-selected" : ""
          }`}
          disabled={busy}
          onClick={() => setSelectedSubject("pet_cat")}
          type="button"
        >
          <b aria-hidden="true">🐈</b>
          <strong>猫咪桌宠</strong>
          <span>一只猫、清晰头部，尽量包含身体、尾巴与花纹。</span>
        </button>
        <button
          aria-pressed={selectedSubject === "human_avatar"}
          className={`subject-choice-card ${
            selectedSubject === "human_avatar" ? "subject-choice-card-selected" : ""
          }`}
          disabled={busy}
          onClick={() => setSelectedSubject("human_avatar")}
          type="button"
        >
          <b aria-hidden="true">人</b>
          <strong>Q 版人物</strong>
          <span>仅本人或已明确授权的成年人。</span>
        </button>
      </div>
      <label className="field-stack subject-draft-name">
        <span>草稿名</span>
        <input
          autoComplete="off"
          disabled={busy}
          maxLength={64}
          onChange={(event) => setDraftName(event.target.value)}
          placeholder="例如：橘子、我的 Q 版形象"
          type="text"
          value={draftName}
        />
        <small>用于区分本地草稿，最多 64 个字符。</small>
      </label>
      {selectedSubject === "human_avatar" ? (
        <label className="consent-row subject-consent">
          <input
            checked={authorizationConfirmed}
            onChange={(event) => onAuthorizationChange(event.target.checked)}
            type="checkbox"
          />
          <span>我确认主体是本人，或已获得该成年人的明确授权。</span>
        </label>
      ) : null}
      <div className="subject-create-action">
        <button
          className="primary-button"
          disabled={
            busy ||
            !normalizedDraftName ||
            (selectedSubject === "human_avatar" && !authorizationConfirmed)
          }
          onClick={() => void onCreate(selectedSubject, normalizedDraftName)}
          type="button"
        >
          自定义桌宠
        </button>
      </div>
      <p className="availability-note">
        人物流程拒绝未成年人、公众人物模仿、多人合照、未经授权第三方和成人内容。
      </p>
      {drafts.length ? (
        <div className="draft-list">
          <h3>恢复本地草稿</h3>
          {drafts.map((draft) => (
            <div key={draft.id}>
              <button onClick={() => onResume(draft.id)} type="button">
                <strong>
                  {draft.displayName ??
                    (draft.subjectKind === "pet_cat" ? "猫咪" : "Q 版人物")}
                </strong>
                <span>{draft.photos.length} 张照片 · {statusLabel(draft.status)}</span>
              </button>
              <button
                className="danger-link"
                onClick={() => {
                  if (window.confirm("删除此草稿及其本地照片副本？")) onDelete(draft.id);
                }}
                type="button"
              >
                删除
              </button>
            </div>
          ))}
        </div>
      ) : null}
    </section>
  );
}

function DraftProgress({
  draft,
  onConfirm,
  onResume,
}: {
  draft: CreationDraft;
  onConfirm(): void;
  onResume(): void;
}) {
  const [portrait, setPortrait] = useState<PortraitPreview | null>(null);
  const [portraitError, setPortraitError] = useState<string | null>(null);
  useEffect(() => {
    if (draft.status !== "awaiting_confirmation" || !draft.serverJobId) {
      setPortrait(null);
      return;
    }
    let active = true;
    void getPortraitPreview(draft.serverJobId)
      .then((value) => {
        if (active) {
          setPortrait(value);
          setPortraitError(null);
        }
      })
      .catch((reason) => {
        if (active) setPortraitError(reason instanceof Error ? reason.message : String(reason));
      });
    return () => {
      active = false;
    };
  }, [draft.serverJobId, draft.status]);
  const currentIndex = useMemo(
    () => GENERATION_STAGES.findIndex(([status]) => status === draft.status),
    [draft.status],
  );
  return (
    <div className="draft-progress">
      <div>
        <p className="eyebrow">生成任务</p>
        <h3>{statusLabel(draft.status)}</h3>
        <p>
          {draft.errorMessage ??
            "照片确认后将显示服务端真实阶段；没有真实子任务进度时不会伪造百分比。"}
        </p>
      </div>
      {draft.errorMessage ? (
        <div className="availability-note" role="status">
          <strong>{draft.retryable ? "可恢复" : "需要处理"}</strong>
          <span>{draft.errorMessage}</span>
        </div>
      ) : null}
      <ol className="generation-timeline">
        {GENERATION_STAGES.map(([status, label], index) => (
          <li
            className={
              draft.status === "completed" || (currentIndex >= 0 && index <= currentIndex)
                ? "stage-active"
                : ""
            }
            key={status}
          >
            <span />
            {label}
          </li>
        ))}
      </ol>
      {draft.status === "awaiting_confirmation" ? (
        <div className="portrait-confirmation">
          {portrait ? (
            <img alt="OpenVINO 生成的 Q 版静态预览" src={portrait.dataUrl} />
          ) : (
            <div className="photo-placeholder">正在校验预览…</div>
          )}
          <div>
            <strong>确认静态 Q 版形象</strong>
            <p>
              确认后会保留这张形象；人物将检测姿态、拆分头身和四肢、绑定骨骼并生成
              Atlas，猫咪当前仍使用整体形变动画。
            </p>
            {portrait ? (
              <small>
                加载 {String(portrait.metrics.load_time_ms ?? "unknown")} ms · 推理{" "}
                {String(portrait.metrics.inference_time_ms ?? "unknown")} ms · 峰值进程内存{" "}
                {String(portrait.metrics.peak_process_memory_mb ?? "unknown")} MB
                {" · "}身份条件{" "}
                {portrait.metrics.identity_conditioning ===
                "segmented-img2img-reference-v1"
                  ? "分割参考图（非 FaceID）"
                  : "unknown"}
              </small>
            ) : null}
            {portraitError ? <small className="probe-error">{portraitError}</small> : null}
            <button
              className="primary-button"
              disabled={!portrait}
              onClick={onConfirm}
              type="button"
            >
              确认并生成桌宠动画
            </button>
          </div>
        </div>
      ) : null}
      {draft.serverJobId &&
      (draft.status === "packaging" ||
        draft.status === "completed" ||
        (draft.status === "failed" && draft.retryable)) ? (
        <button className="primary-button" onClick={onResume} type="button">
          {draft.status === "completed"
            ? "重新打包并替换旧模板角色"
            : "检查任务并继续安装"}
        </button>
      ) : null}
      <div className="preview-boundary">
        <strong>标准立绘确认与动作预览</strong>
        <p>
          服务端返回 `awaiting_confirmation` 后在这里显示标准立绘；人物确认后会生成语义骨骼
          Sprite Atlas，不会用 Mock 模板重画角色。
        </p>
      </div>
    </div>
  );
}

function statusLabel(status: CreationDraft["status"]): string {
  return {
    editing: "编辑照片",
    ready: "可以提交",
    submitting: "正在提交",
    checking: "检查照片",
    queued: "等待队列",
    generating_portrait: "生成标准形象",
    awaiting_confirmation: "等待确认标准形象",
    generating_actions: "生成动作",
    packaging: "安全打包",
    completed: "已完成",
    service_unavailable: "生成服务尚未配置",
    failed: "生成失败",
    cancelled: "已取消",
  }[status];
}

function formatBytes(bytes: number): string {
  return bytes < 1024 * 1024
    ? `${Math.max(1, Math.round(bytes / 1024))} KB`
    : `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
