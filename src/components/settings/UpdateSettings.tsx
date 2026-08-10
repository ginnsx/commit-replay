import type { AvailableUpdate } from "../../lib/types";
import type { DownloadProgress, UpdateStatus } from "../../lib/update";

function formatTime(value?: string): string {
  if (!value) return "尚未检查";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}

function progressLabel(progress?: DownloadProgress): string {
  if (!progress) return "准备下载…";
  if (!progress.totalBytes) return `已下载 ${Math.round(progress.downloadedBytes / 1024)} KB`;
  return `${Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100))}%`;
}

export function UpdateSettings({
  appVersion,
  status,
  lastCheckedAt,
  availableUpdate,
  error,
  progress,
  migrating,
  supported,
  onCheck,
  onInstall,
  onLater,
}: {
  appVersion: string;
  status: UpdateStatus;
  lastCheckedAt?: string;
  availableUpdate?: AvailableUpdate;
  error?: string;
  progress?: DownloadProgress;
  migrating: boolean;
  supported: boolean;
  onCheck: () => void;
  onInstall: () => void;
  onLater: () => void;
}) {
  const checking = status === "checking";
  const downloading = status === "downloading";

  return (
    <div className="update-settings">
      <div className="update-card">
        <div>
          <div className="update-card-label">当前版本</div>
          <div className="update-version">v{appVersion}</div>
        </div>
        <button
          type="button"
          className="btn btn-ghost"
          disabled={!supported || checking || downloading}
          onClick={onCheck}
        >
          {checking ? "检查中…" : "检查更新"}
        </button>
      </div>

      <div className="update-meta">上次成功检查：{formatTime(lastCheckedAt)}</div>

      {!supported ? <div className="update-notice">更新仅可在安装后的正式版中使用。</div> : null}
      {status === "latest" ? <div className="update-notice ok">当前已是最新版本。</div> : null}
      {status === "failed" ? (
        <div className="update-notice error">检查失败：{error ?? "请稍后重试"}</div>
      ) : null}

      {availableUpdate ? (
        <div className="update-card update-card--available">
          <div className="update-card-label">发现新版本</div>
          <div className="update-version">v{availableUpdate.version}</div>
          {availableUpdate.date ? (
            <div className="update-meta">发布时间：{formatTime(availableUpdate.date)}</div>
          ) : null}
          {availableUpdate.notes ? (
            <div className="update-notes">{availableUpdate.notes}</div>
          ) : null}
          {migrating ? <div className="update-notice">迁移执行中，完成后才能安装更新。</div> : null}
          {downloading ? (
            <div className="update-download-progress">
              正在下载并安装：{progressLabel(progress)}
            </div>
          ) : (
            <div className="update-actions">
              <button type="button" className="btn btn-ghost" onClick={onLater}>
                稍后
              </button>
              <button
                type="button"
                className="btn btn-primary"
                disabled={migrating || checking}
                onClick={onInstall}
              >
                立即更新
              </button>
            </div>
          )}
        </div>
      ) : null}
    </div>
  );
}
