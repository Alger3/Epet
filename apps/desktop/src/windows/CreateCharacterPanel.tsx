import { useEffect, useMemo, useState } from "react";

import type { SubjectKind } from "../shared/characters";
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
  const draft = workshop.selectedDraft;

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
          <DraftProgress draft={draft} />
        )}
      </div>

      <div className="draft-actions">
        <button
          className="primary-button"
          disabled={!draft.photos.some((photo) => photo.role === "primary") || workshop.busy}
          onClick={() => void workshop.startGeneration(draft.id)}
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

function DraftProgress({ draft }: { draft: CreationDraft }) {
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
      <div className="preview-boundary">
        <strong>标准立绘确认与动作预览</strong>
        <p>
          服务端返回 `awaiting_confirmation` 后在这里显示标准立绘；动作完成后使用与桌宠相同的 Sprite Atlas 播放器预览。
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
