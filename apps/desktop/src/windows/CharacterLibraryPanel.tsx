import { useState } from "react";

import { useCharacterLibrary } from "../shared/character-library";
import { useCharacter } from "../shared/use-character";

function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

interface CharacterLibraryPanelProps {
  activeCharacterId: string;
  onActivate(characterId: string): Promise<void>;
}

export function CharacterLibraryPanel({
  activeCharacterId,
  onActivate,
}: CharacterLibraryPanelProps) {
  const library = useCharacterLibrary();
  const [url, setUrl] = useState("");
  const [sha256, setSha256] = useState("");
  const installed = library.characters.filter((character) => !character.builtIn);
  const canInstall =
    url.startsWith("https://") && /^[0-9a-f]{64}$/.test(sha256) && !library.busy;

  const install = async () => {
    const result = await library.installFromUrl(url.trim(), sha256.trim());
    if (result) {
      setUrl("");
      setSha256("");
    }
  };

  return (
    <section className="library-page" aria-labelledby="library-title">
      <article className="character-library-card install-card">
        <div>
          <p className="eyebrow">安全安装</p>
          <h2 id="library-title">从 HTTPS 安装 .epet</h2>
          <p className="card-description">
            下载先写入临时目录，通过包外 SHA-256、manifest、逐文件哈希、大小和路径校验后才原子写入角色库。
          </p>
        </div>
        <label className="field-stack">
          <span>下载地址</span>
          <input
            id="package-download-url"
            disabled={library.busy}
            onChange={(event) => setUrl(event.target.value)}
            placeholder="https://…/character.epet"
            spellCheck={false}
            type="url"
            value={url}
          />
        </label>
        <label className="field-stack">
          <span>期望 SHA-256（64 位小写十六进制）</span>
          <input
            disabled={library.busy}
            maxLength={64}
            onChange={(event) => setSha256(event.target.value.trim())}
            placeholder="由生成服务或发布清单提供"
            spellCheck={false}
            value={sha256}
          />
        </label>
        <button
          className="primary-button"
          disabled={!canInstall}
          onClick={() => void install()}
          type="button"
        >
          {library.busy ? "处理中…" : "下载并安装"}
        </button>
        {library.error ? <div className="error-banner" role="alert">{library.error}</div> : null}
      </article>

      <div className="library-heading">
        <div>
          <p className="eyebrow">本地索引</p>
          <h2>我的角色</h2>
        </div>
        <button
          className="secondary-button compact-button"
          disabled={library.busy}
          onClick={() => void library.reload()}
          type="button"
        >
          刷新
        </button>
      </div>

      {installed.length === 0 ? (
        <article className="empty-library">
          <strong>还没有安装生成角色</strong>
          <p>安装成功后会在这里显示当前版本和保留的旧版本。</p>
        </article>
      ) : (
        <div className="installed-character-list">
          {installed.map((character) => (
            <article className="installed-character-card" key={character.id}>
              <header>
                <InstalledThumbnail
                  characterId={character.id}
                  fallback={character.name.slice(0, 1)}
                  localAvailable={character.localAvailable}
                />
                <div>
                  <h3>{character.name}</h3>
                  <p>{character.id}</p>
                </div>
                <span className={`version-pill ${character.localAvailable ? "" : "version-missing"}`}>
                  {character.localAvailable ? `当前 v${character.currentVersion}` : "本地资源丢失"}
                </span>
              </header>
              <div className="version-list">
                {character.versions.map((version) => (
                  <div className="version-row" key={version.packageSha256}>
                    <div>
                      <strong>v{version.packageVersion}</strong>
                      <small>
                        {formatBytes(version.packageSize)} · {version.packageSha256.slice(0, 12)}…
                      </small>
                    </div>
                    <div className="version-actions">
                      {version.current ? (
                        <span>当前版本</span>
                      ) : (
                        <>
                          <button
                            disabled={library.busy}
                            onClick={() =>
                              void library.activateVersion(character.id, version.packageSha256)
                            }
                            type="button"
                          >
                            回滚到此版本
                          </button>
                          <button
                            className="danger-link"
                            disabled={library.busy}
                            onClick={() => {
                              if (window.confirm(`删除 ${character.name} v${version.packageVersion}？`)) {
                                void library.deleteVersion(character.id, version.packageSha256);
                              }
                            }}
                            type="button"
                          >
                            删除旧版本
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                ))}
              </div>
              <footer>
                <span>新包安装为更新；现有版本不会被覆盖。</span>
                <div className="version-actions">
                  <button
                    disabled={library.busy}
                    onClick={() => {
                      const customName = window.prompt("新的角色名称", character.name)?.trim();
                      if (customName && customName !== character.name) {
                        void library.renameCharacter(character.id, customName);
                      }
                    }}
                    type="button"
                  >
                    重命名
                  </button>
                  {!character.localAvailable ? (
                    <button
                      onClick={() => {
                        document.getElementById("package-download-url")?.focus();
                        window.scrollTo({ top: 0, behavior: "smooth" });
                      }}
                      type="button"
                    >
                      重新下载
                    </button>
                  ) : null}
                  <button
                    disabled={
                      library.busy ||
                      activeCharacterId === character.id ||
                      !character.localAvailable
                    }
                    onClick={() => void onActivate(character.id)}
                    type="button"
                  >
                    {activeCharacterId === character.id ? "使用中" : "使用到桌面"}
                  </button>
                  <button
                    className="danger-link"
                    disabled={library.busy}
                    onClick={() => {
                      const activeNote =
                        activeCharacterId === character.id
                          ? " 当前桌宠会先停止并切回内置角色。"
                          : "";
                      if (
                        window.confirm(
                          `删除角色“${character.name}”及其全部版本？${activeNote}`,
                        )
                      ) {
                        void library.deleteCharacter(character.id);
                      }
                    }}
                    type="button"
                  >
                    删除角色
                  </button>
                </div>
              </footer>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

function InstalledThumbnail({
  characterId,
  fallback,
  localAvailable,
}: {
  characterId: string;
  fallback: string;
  localAvailable: boolean;
}) {
  if (!localAvailable) {
    return <div className="package-avatar" aria-hidden="true">{fallback}</div>;
  }
  return <AvailableThumbnail characterId={characterId} fallback={fallback} />;
}

function AvailableThumbnail({
  characterId,
  fallback,
}: {
  characterId: string;
  fallback: string;
}) {
  const { character } = useCharacter(characterId);
  return character ? (
    <img className="package-thumbnail" alt="" src={character.assetUrl} />
  ) : (
    <div className="package-avatar" aria-hidden="true">{fallback}</div>
  );
}
