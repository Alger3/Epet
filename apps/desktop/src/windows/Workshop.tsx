import petSpriteUrl from "../../../../assets/builtin-pet/cat-idle.png";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { useRuntimeState } from "../shared/use-runtime-state";

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
          <button className="nav-item nav-item-active" type="button">桌宠</button>
          <button className="nav-item" disabled type="button">创建宠物</button>
          <button className="nav-item" disabled type="button">我的宠物</button>
          <button className="nav-item" disabled type="button">设置</button>
        </nav>
        <span className="phase-badge">阶段 2 · 桌面壳</span>
      </aside>

      <section className="workspace">
        <header className="workspace-header">
          <div>
            <p className="eyebrow">EPET DESKTOP</p>
            <h1>桌宠运行控制</h1>
            <p>当前使用内置离线宠物验证窗口、托盘和恢复能力。</p>
          </div>
          <div className="status-pill">
            <span className={state.visible ? "status-dot status-online" : "status-dot"} />
            {state.visible ? "桌宠已显示" : "桌宠已隐藏"}
          </div>
        </header>

        {error || autostartError ? (
          <div className="error-banner" role="alert">{error ?? autostartError}</div>
        ) : null}

        <div className="dashboard-grid">
          <article className="pet-card">
            <div className="pet-preview" aria-hidden="true">
              <img src={petSpriteUrl} alt="" />
            </div>
            <div>
              <p className="eyebrow">内置测试宠物</p>
              <h2>橘子</h2>
              <p>无需网络。用于确认桌宠窗口在云端不可用时仍可运行。</p>
            </div>
            <button
              className="primary-button"
              onClick={() => void actions.setVisible(!state.visible)}
              type="button"
            >
              {state.visible ? "隐藏桌宠" : "显示到桌面"}
            </button>
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
              <div><dt>宠物 ID</dt><dd>{state.activePetId}</dd></div>
              <div><dt>显示器</dt><dd>{state.monitorId ?? "等待首次定位"}</dd></div>
              <div><dt>坐标</dt><dd>{state.x === null ? "自动" : `${Math.round(state.x)}, ${Math.round(state.y ?? 0)}`}</dd></div>
              <div><dt>状态版本</dt><dd>v{state.runtimeVersion}</dd></div>
            </dl>
          </article>
        </div>
      </section>
    </main>
  );
}
