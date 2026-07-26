import { useWorkshopState } from "../shared/workshop";

export function PrivacyPanel() {
  const workshop = useWorkshopState();
  return (
    <section className="privacy-page">
      <article className="character-library-card">
        <div>
          <p className="eyebrow">本地数据边界</p>
          <h2>设置与隐私</h2>
          <p className="card-description">
            原照片只在 WebView 中只读解码；Epet 持久化的是重新绘制、压缩并去除元数据后的副本。
          </p>
        </div>
        <dl className="privacy-inventory">
          <div>
            <dt>本地草稿</dt>
            <dd>{workshop.loading ? "正在读取…" : `${workshop.snapshot.drafts.length} 个`}</dd>
          </div>
          <div>
            <dt>云端照片与派生资源</dt>
            <dd>未上传；阶段 5 服务尚未配置</dd>
          </div>
          <div>
            <dt>输入活动</dt>
            <dd>只读取距上次输入的时长，不记录按键或鼠标内容</dd>
          </div>
          <div>
            <dt>角色包下载地址</dt>
            <dd>签名 URL 和查询令牌不写入角色索引</dd>
          </div>
        </dl>
      </article>

      <article className="character-library-card">
        <div>
          <p className="eyebrow">草稿清理</p>
          <h2>删除清理后的照片副本</h2>
          <p className="card-description">
            删除是不可逆操作。每个草稿单独确认，不会影响已安装角色或其他草稿。
          </p>
        </div>
        {workshop.error ? <div className="error-banner">{workshop.error}</div> : null}
        {workshop.snapshot.drafts.length ? (
          <div className="privacy-draft-list">
            {workshop.snapshot.drafts.map((draft) => (
              <div key={draft.id}>
                <span>
                  <strong>{draft.subjectKind === "pet_cat" ? "猫咪草稿" : "人物草稿"}</strong>
                  <small>{draft.photos.length} 张清理后照片 · {draft.id}</small>
                </span>
                <button
                  className="danger-link"
                  disabled={workshop.busy}
                  onClick={() => {
                    if (window.confirm("永久删除此草稿和全部清理后照片？")) {
                      void workshop.deleteDraft(draft.id);
                    }
                  }}
                  type="button"
                >
                  删除本地数据
                </button>
              </div>
            ))}
          </div>
        ) : (
          <div className="empty-library">没有需要清理的本地草稿。</div>
        )}
      </article>
    </section>
  );
}
