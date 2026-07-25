import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useRuntimeState } from "../shared/use-runtime-state";
import { BUILTIN_CHARACTERS, findCharacter } from "../shared/characters";

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
  const [autostart, setAutostart] = useState(false);
  const [autostartError, setAutostartError] = useState<string | null>(null);
  const activeCharacter = findCharacter(state.activeCharacterId);

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
          <button className="nav-item nav-item-active" type="button">角色主页</button>
          <button className="nav-item" type="button">内置角色</button>
          <button className="nav-item" disabled type="button">照片生成</button>
          <button className="nav-item" type="button">运行设置</button>
        </nav>
        <span className="phase-badge">离线 Alpha · 双角色</span>
      </aside>

      <section className="workspace">
        <header className="workspace-header">
          <div>
            <p className="eyebrow">EPET DESKTOP</p>
            <h1>桌面角色工坊</h1>
            <p>选择猫咪或原创 Q 版人物，立即显示到 Windows 桌面并离线运行。</p>
          </div>
          <div className="status-pill">
            <span className={state.visible ? "status-dot status-online" : "status-dot"} />
            {state.visible ? "角色已显示" : "角色已隐藏"}
          </div>
        </header>

        {error || autostartError ? (
          <div className="error-banner" role="alert">{error ?? autostartError}</div>
        ) : null}

        <div className="dashboard-grid">
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

          <article className="diagnostics-card">
            <p className="eyebrow">运行快照</p>
            <dl>
              <div><dt>角色 ID</dt><dd>{state.activeCharacterId}</dd></div>
              <div><dt>显示器</dt><dd>{state.monitorId ?? "等待首次定位"}</dd></div>
              <div><dt>坐标</dt><dd>{state.x === null ? "自动" : `${Math.round(state.x)}, ${Math.round(state.y ?? 0)}`}</dd></div>
              <div><dt>窗口层级</dt><dd>{state.alwaysOnTop ? "始终置顶" : "普通窗口"}</dd></div>
              <div><dt>状态版本</dt><dd>v{state.runtimeVersion}</dd></div>
            </dl>
          </article>
        </div>
      </section>
    </main>
  );
}
