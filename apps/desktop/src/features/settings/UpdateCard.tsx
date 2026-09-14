import Markdown from "react-markdown";
import { ArrowDownloadRegular, ArrowSyncRegular, CheckmarkCircleRegular, OpenRegular } from "@fluentui/react-icons";
import { Button } from "@qzip/ui";
import { useI18n } from "../../lib/i18n";
import type { AppUpdates } from "./useAppUpdates";
import appIcon from "../../../src-tauri/icons/128x128@2x.png";

export function UpdateCard({ updates, currentVersion }: { updates: AppUpdates; currentVersion: string }) {
  const { text, locale } = useI18n();
  const { result, phase, progress, error, downloaded } = updates;
  const available = result?.status === "update_available";
  const checking = phase === "checking";
  const transferring = phase === "downloading" || phase === "verifying";
  const installing = phase === "installing";
  const busy = checking || transferring || installing;
  const total = progress?.total || result?.downloadSize || 0;
  const percent = total && progress ? Math.min(100, Math.round(progress.downloaded / total * 100)) : 0;
  const date = result?.publishedAt ? new Date(result.publishedAt) : null;
  const size = (bytes: number) => `${(bytes / 1024 / 1024).toLocaleString(locale, { maximumFractionDigits: 1 })} MB`;
  const title = checking ? text("正在检查更新", "Checking for updates") : available ? text("新版本，已就绪", "A new version is here") : result?.status === "up_to_date" ? text("已是最新版本", "You're up to date") : text("让轻压保持最新", "Keep QZip up to date");
  const errorMessages: Record<string, string> = {
    UPDATE_NETWORK: "Could not connect to GitHub. Check your connection and retry.",
    UPDATE_HTTP: "GitHub is unavailable or rate-limited. Please try again later.",
    UPDATE_CHECKSUM_MISMATCH: "Verification failed. Please download the installer again.",
    UPDATE_CHECKSUM_INVALID: "The release checksum is invalid. Download has been stopped.",
    UPDATE_SIZE: "The download is incomplete or has an unexpected size. Please retry.",
    UPDATE_FILE: "Could not save or read the installer. Check disk space and permissions.",
    UPDATE_TASKS_ACTIVE: "Wait for active archive tasks to finish before installing.",
    UPDATE_INSTALL: "Could not open the installer. Please retry.",
    UPDATE_ASSET_MISSING: "This release has no Windows x64 installer yet.",
    UPDATE_CHECKSUM_MISSING: "This release has no checksum file for verification.",
    UPDATE_NOT_READY: "Download and verify the installer first."
  };
  return <section className="qzip-update-card" aria-label={text("应用更新", "Application updates")}>
    <div className="qzip-update-card__hero">
      <img src={appIcon} alt="" aria-hidden="true" />
      <div>
        <span className="qzip-update-card__eyebrow">{text("来自 GitHub 的官方发布", "Official releases from GitHub")}</span>
        <h3 aria-live="polite">{title}</h3>
        <div className="qzip-update-card__versions"><span>{text("当前", "Current")} {currentVersion}</span>{available ? <><span aria-hidden="true">→</span><strong>v{result.latestVersion}</strong><span className="qzip-status-pill" data-tone="success">{text("稳定版", "Stable")}</span></> : null}</div>
      </div>
    </div>
    {result && (available || result.releaseNotes?.trim()) ? <>
      <div className="qzip-update-card__meta">
        {date && !Number.isNaN(date.getTime()) ? <time dateTime={result.publishedAt}>{date.toLocaleDateString(locale, { year: "numeric", month: "short", day: "numeric" })}</time> : null}
        <span>Windows x64</span>{total > 0 ? <span>{size(total)}</span> : null}
      </div>
      <div className="qzip-release-notes-heading"><h4>{text("本次更新", "What's new")}</h4><button type="button" onClick={() => void updates.openRelease(result.releaseUrl)}>{text("查看原文", "View on GitHub")}<OpenRegular fontSize={15} /></button></div>
      <div className="qzip-release-notes" tabIndex={0} role="region" aria-label={text("版本更新记录", "Release notes")}>
        {result.releaseNotes?.trim() ? <Markdown skipHtml components={{
          img: () => null,
          a: ({ href, children }) => href?.startsWith("https://") ? <a href={href} target="_blank" rel="noreferrer" onClick={(event) => { event.preventDefault(); void updates.openRelease(href); }}>{children}</a> : <span>{children}</span>
        }}>{result.releaseNotes}</Markdown> : <p>{text("作者尚未提供本版本的更新记录。", "The author has not provided release notes for this version.")}</p>}
      </div>
    </> : <p className="qzip-update-card__intro">{result?.status === "up_to_date" ? text("你正在使用最新稳定版，感谢使用轻压。", "You're running the latest stable release. Thanks for using QZip.") : result?.status === "unavailable" ? text("请在轻压桌面应用中检查并下载更新。", "Check and download updates in the QZip desktop app.") : text("在这里查看更新记录，直接下载新版本。", "Read release notes and download new versions right here.")}</p>}
    {error ? <p className="qzip-update-card__error" role="alert">{text(error.message, errorMessages[error.code] ?? "The update could not be completed. Please try again.")}</p> : null}
    <footer className="qzip-update-card__footer">
      {transferring ? <div className="qzip-update-download" aria-live="polite">
        <div><strong>{phase === "verifying" ? text("正在校验安装包…", "Verifying installer…") : progress ? text("正在下载", "Downloading") : text("正在连接 GitHub…", "Connecting to GitHub…")}</strong><span>{progress ? `${size(progress.downloaded)} / ${size(total)}` : ""}</span></div>
        <progress aria-label={text("更新下载进度", "Update download progress")} max={100} value={progress ? percent : undefined} />
      </div> : <p className="qzip-update-card__footnote" role="status">{downloaded ? <><CheckmarkCircleRegular fontSize={18} />{text("下载完成，SHA-256 校验通过", "Download complete. SHA-256 verified.")}</> : updates.cancelled ? text("下载已取消，可以重新下载。", "Download cancelled. You can start again.") : available && !result.downloadAvailable ? text("此版本暂不支持应用内下载，请前往 GitHub 发布页。", "In-app download is unavailable for this release. Visit GitHub.") : available ? text("通过安全连接下载，完成后自动校验。", "Downloaded over HTTPS and verified before installation.") : text("仅检查稳定版本", "Stable releases only")}</p>}
      <div className="qzip-update-card__actions">
        {transferring ? <Button variant="secondary" disabled={updates.cancelling} onClick={() => void updates.cancel()}>{updates.cancelling ? text("正在取消…", "Cancelling…") : text("取消下载", "Cancel download")}</Button> : <>
          <Button variant={available ? "tertiary" : "primary"} loading={checking} disabled={busy} icon={<ArrowSyncRegular fontSize={18} />} onClick={() => void updates.check()}>{text("检查更新", "Check for updates")}</Button>
          {available && result.downloadAvailable ? downloaded ? <Button loading={installing} onClick={() => void updates.install()}>{text("安装更新", "Install update")}</Button> : <Button disabled={busy} icon={<ArrowDownloadRegular fontSize={19} />} onClick={() => void updates.download()}>{error || updates.cancelled ? text("重新下载", "Retry download") : text("下载更新", "Download update")}</Button> : available ? <Button onClick={() => void updates.openRelease(result.releaseUrl)}>{text("前往 GitHub", "Open GitHub")}</Button> : null}
        </>}
      </div>
      {downloaded ? <small>{text("点击安装将关闭轻压，并打开安装向导。", "Installing closes QZip and opens the setup wizard.")}</small> : null}
    </footer>
  </section>;
}
