import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useRuntimeState } from "../shared/use-runtime-state";
import { BUILTIN_CHARACTERS } from "../shared/characters";
import { useCharacter } from "../shared/use-character";
import { CharacterLibraryPanel } from "./CharacterLibraryPanel";
import { CreateCharacterPanel } from "./CreateCharacterPanel";
import { PrivacyPanel } from "./PrivacyPanel";

type WorkshopPage = "home" | "library" | "create" | "settings" | "privacy";

const PAGE_COPY: Record<WorkshopPage, { title: string; description: string }> = {
  home: {
    title: "角色工坊",
    description: "选择陪伴角色，查看当前桌宠并控制显示状态。",
  },
  library: {
    title: "我的角色",
    description: "安全安装、更新、回滚和删除本地 .epet 角色包。",
  },
  create: {
    title: "创建角色",
    description: "猫咪与 Q 版人物使用独立的输入规则、授权门禁和生成流程。",
  },
  settings: {
    title: "运行设置",
    description: "控制桌宠窗口层级、交互、移动、睡眠和启动行为。",
  },
  privacy: {
    title: "设置与隐私",
    description: "查看本地数据边界，并单独删除草稿和清理后的照片副本。",
  },
};

interface ToggleProps {
  checked: boolean;
  label: string;
  description: string;
  onChange(checked: boolean): void;
}

function Toggle({ checked, label, description, onChange }: ToggleProps) {
  return (
    <label className="setting-row">
      <span>
        <strong>{label}</strong>
        <small>{description}</small>
      </span>
      <input
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
        type="checkbox"
      />
    </label>
  );
}

export function Workshop() {
  const [state, actions, error] = useRuntimeState();
  const [page, setPage] = useState<WorkshopPage>("home");
  const [autostart, setAutostart] = useState(false);
  const [autostartError, setAutostartError] = useState<string | null>(null);
  const {
    character: activeCharacter,
    error: characterError,
  } = useCharacter(state.activeCharacterId);

  useEffect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    void invoke<boolean>("get_autostart_enabled")
      .then(setAutostart)
      .catch((reason) => setAutostartError(String(reason)));
  }, []);

  const updateAutostart = async (enabled: boolean) => {
    setAutostartError(null);
    if (!("__TAURI_INTERNALS__" in window)) {
      setAutostart(enabled);
      return;
    }

    try {
      setAutostart(await invoke<boolean>("set_autostart_enabled", { enabled }));
    } catch (reason) {
      setAutostartError(String(reason));
    }
  };

  return (
    <main className="workshop-shell">
      <aside className="sidebar">
        <div className="brand-mark" aria-hidden="true">E</div>
        <nav aria-label="主要导航">
          <button
            className={`nav-item ${page === "home" ? "nav-item-active" : ""}`}
            onClick={() => setPage("home")}
            type="button"
          >
            角色主页
          </button>
          <button
            className={`nav-item ${page === "library" ? "nav-item-active" : ""}`}
            onClick={() => setPage("library")}
            type="button"
          >
            我的角色
          </button>
          <button
            className={`nav-item ${page === "create" ? "nav-item-active" : ""}`}
            onClick={() => setPage("create")}
            type="button"
          >
            创建角色
          </button>
          <button
            className={`nav-item ${page === "settings" ? "nav-item-active" : ""}`}
            onClick={() => setPage("settings")}
            type="button"
          >
            运行设置
          </button>
          <button
            className={`nav-item ${page === "privacy" ? "nav-item-active" : ""}`}
            onClick={() => setPage("privacy")}
            type="button"
          >
            设置与隐私
          </button>
        </nav>
        <span className="phase-badge">阶段 4 · 角色工坊</span>
      </aside>

      <section className="workspace">
        <header className="workspace-header">
          <div>
            <p className="eyebrow">EPET DESKTOP</p>
            <h1>{PAGE_COPY[page].title}</h1>
            <p>{PAGE_COPY[page].description}</p>
          </div>
          <div className="status-pill">
            <span className={state.visible ? "status-dot status-online" : "status-dot"} />
            {state.visible ? "角色已显示" : "角色已隐藏"}
          </div>
        </header>

        {error || autostartError || characterError || state.diagnostic ? (
          <div className="error-banner" role="alert">
            {error ?? autostartError ?? characterError ?? state.diagnostic}
          </div>
        ) : null}

        {page === "library" ? (
          <CharacterLibraryPanel
            activeCharacterId={state.activeCharacterId}
            onActivate={actions.setActiveCharacter}
          />
        ) : null}
        {page === "create" ? <CreateCharacterPanel /> : null}
        {page === "privacy" ? <PrivacyPanel /> : null}

        {page === "home" || page === "settings" ? <div className="dashboard-grid">
          {page === "home" && activeCharacter ? (
          <article className="pet-card">
            <div className="pet-preview" aria-hidden="true">
              <img src={activeCharacter.assetUrl} alt="" />
            </div>
            <div>
              <p className="eyebrow">当前角色 · {activeCharacter.subjectLabel}</p>
              <h2>{activeCharacter.name}</h2>
              <p>{activeCharacter.description}</p>
            </div>
            <button
              className="primary-button"
              onClick={() => void actions.setVisible(!state.visible)}
              type="button"
            >
              {state.visible ? "隐藏角色" : "显示到桌面"}
            </button>
          </article>
          ) : null}

          {page === "home" ? (
          <article className="character-library-card">
            <div>
              <p className="eyebrow">内置角色库</p>
              <h2>选择陪伴角色</h2>
              <p className="card-description">无需账号和网络；选择会自动保存，下次启动继续使用。</p>
            </div>
            <div className="character-options">
              {BUILTIN_CHARACTERS.map((character) => {
                const selected = character.id === state.activeCharacterId;
                return (
                  <button
                    className={`character-option ${selected ? "character-option-active" : ""}`}
                    key={character.id}
                    onClick={() => void actions.setActiveCharacter(character.id)}
                    type="button"
                  >
                    <img alt="" src={character.assetUrl} />
                    <span><strong>{character.name}</strong><small>{character.subjectLabel}</small></span>
                    <b>{selected ? "使用中" : "选择"}</b>
                  </button>
                );
              })}
            </div>
            <p className="availability-note">
              照片生成需要后续配置独立 AI 服务；本离线版不会上传照片或伪造生成结果。
            </p>
          </article>
          ) : null}

          {page === "settings" ? (
          <article className="settings-card">
            <div>
              <p className="eyebrow">运行设置</p>
              <h2>交互与状态</h2>
            </div>
            <Toggle
              checked={state.paused}
              description="停止动画和自主行为，保留当前画面"
              label="暂停桌宠"
              onChange={(value) => void actions.setPaused(value)}
            />
            <Toggle
              checked={state.clickThrough}
              description="鼠标事件交给桌宠后方窗口，可从托盘关闭"
              label="鼠标穿透"
              onChange={(value) => void actions.setClickThrough(value)}
            />
            <Toggle
              checked={state.alwaysOnTop}
              description="关闭后使用普通窗口层级，会被其他应用覆盖"
              label="始终置顶"
              onChange={(value) => void actions.setAlwaysOnTop(value)}
            />
            <Toggle
              checked={state.autonomousMovement}
              description="以低频移动桌宠窗口，暂停或拖拽时自动停止"
              label="自主移动"
              onChange={(value) => void actions.setAutonomousMovement(value)}
            />
            <label className="setting-row">
              <span>
                <strong>无操作后睡觉</strong>
                <small>读取 Windows 最后输入时间；睡着后仅可在 4 秒内连续点击 3 次唤醒</small>
              </span>
              <select
                aria-label="无操作后睡觉"
                onChange={(event) => void actions.setSleepAfterMinutes(Number(event.target.value))}
                value={state.sleepAfterMinutes}
              >
                <option value={1}>1 分钟</option>
                <option value={5}>5 分钟</option>
                <option value={10}>10 分钟</option>
                <option value={20}>20 分钟</option>
                <option value={30}>30 分钟</option>
                <option value={0}>永不</option>
              </select>
            </label>
            <Toggle
              checked={autostart}
              description="登录系统后静默启动到托盘，不主动打开工坊"
              label="开机启动"
              onChange={(value) => void updateAutostart(value)}
            />
            <div className="scale-control">
              <span>
                <strong>显示大小</strong>
                <small>{Math.round(state.scale * 100)}%</small>
              </span>
              <div>
                <button onClick={() => void actions.adjustScale(-0.1)} type="button">−</button>
                <button onClick={() => void actions.adjustScale(0.1)} type="button">＋</button>
              </div>
            </div>
            <button
              className="secondary-button"
              onClick={() => void actions.resetPosition()}
              type="button"
            >
              重置到主屏安全位置
            </button>
          </article>
          ) : null}

          {page === "settings" ? (
          <article className="diagnostics-card">
            <p className="eyebrow">运行快照</p>
            <dl>
              <div><dt>角色 ID</dt><dd>{state.activeCharacterId}</dd></div>
              <div><dt>显示器</dt><dd>{state.monitorId ?? "等待首次定位"}</dd></div>
              <div><dt>坐标</dt><dd>{state.x === null ? "自动" : `${Math.round(state.x)}, ${Math.round(state.y ?? 0)}`}</dd></div>
              <div><dt>窗口层级</dt><dd>{state.alwaysOnTop ? "始终置顶" : "普通窗口"}</dd></div>
              <div><dt>自主移动</dt><dd>{state.autonomousMovement ? "已开启" : "已关闭"}</dd></div>
              <div><dt>自动睡觉</dt><dd>{state.sleepAfterMinutes === 0 ? "永不" : `${state.sleepAfterMinutes} 分钟`}</dd></div>
              <div><dt>状态版本</dt><dd>v{state.runtimeVersion}</dd></div>
            </dl>
          </article>
          ) : null}
        </div> : null}
      </section>
    </main>
  );
}
