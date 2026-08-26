import { useEffect, useState } from "react";
import {
  AlertRegular,
  AppsSettingsRegular,
  ArchiveRegular,
  ArrowLeftRegular,
  ArrowResetRegular,
  ArrowSyncRegular,
  ChatHelpRegular,
  ChevronRightRegular,
  CodeRegular,
  ColorRegular,
  DocumentTextRegular,
  InfoRegular,
  OpenRegular,
  ShieldCheckmarkRegular
} from "@fluentui/react-icons";
import { Button, Card, SegmentedControl } from "@qzip/ui";
import type { AppSettings, AppSettingsPatch, IntegrationStatus } from "../../contracts/settings";
import { settingsClient } from "../../lib/settingsClient";
import { useI18n } from "../../lib/i18n";
import appIcon from "../../../src-tauri/icons/128x128@2x.png";

interface SettingsPageProps {
  settings: AppSettings;
  onBack: () => void;
  onChanged: (settings: AppSettings) => void;
  onToast: (message: string) => void;
}

type SettingsSectionId = "appearance" | "archive" | "notifications" | "system" | "about";

const formatOptions = [
  { value: "sevenZip", label: "7Z" },
  { value: "zip", label: "ZIP" },
  { value: "tar", label: "TAR" },
  { value: "tarGz", label: "TAR.GZ" },
  { value: "tarXz", label: "TAR.XZ" }
];

function Toggle({ checked, onChange, label, hint, disabled = false }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  hint?: string;
  disabled?: boolean;
}) {
  return (
    <label className="qzip-setting-toggle" data-disabled={disabled}>
      <span><strong>{label}</strong>{hint ? <small>{hint}</small> : null}</span>
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} />
    </label>
  );
}

export function SettingsPage({ settings, onBack, onChanged, onToast }: SettingsPageProps) {
  const { text } = useI18n();
  const [status, setStatus] = useState<IntegrationStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [activeSection, setActiveSection] = useState<SettingsSectionId>("appearance");

  useEffect(() => {
    void settingsClient.integration().then(setStatus).catch(() => setStatus(null));
  }, []);

  async function patch(next: AppSettingsPatch) {
    try {
      onChanged(await settingsClient.update(next));
    } catch (reason) {
      onToast(String(reason));
    }
  }

  async function updateNotifications(enabled: boolean) {
    if (enabled && settingsClient.isTauri) {
      try {
        const { isPermissionGranted, requestPermission } = await import("@tauri-apps/plugin-notification");
        if (!(await isPermissionGranted()) && (await requestPermission()) !== "granted") {
          onToast(text("未获得系统通知权限，设置未更改。", "Notification permission was not granted. The setting was not changed."));
          return;
        }
      } catch {
        onToast(text("当前系统不支持请求通知权限。", "This system does not support notification permission requests."));
        return;
      }
    }
    await patch({ taskNotificationsEnabled: enabled });
  }

  async function checkUpdates() {
    setChecking(true);
    try {
      const result = await settingsClient.checkForUpdates();
      onToast(result.configured
        ? text("更新服务已配置，将开始检查。", "The update service is configured. Checking for updates.")
        : text("当前发行包未配置官方更新服务。", "The official update service is not configured for this build."));
    } finally {
      setChecking(false);
    }
  }

  const sectionMeta: Record<SettingsSectionId, { title: string; subtitle: string }> = {
    appearance: { title: text("外观", "Appearance"), subtitle: text("调整轻压的显示方式与界面体验", "Adjust how QZip looks and feels") },
    archive: { title: text("压缩与解压", "Compression & extraction"), subtitle: text("设置常用格式和默认处理方式", "Set common formats and default behaviors") },
    notifications: { title: text("通知", "Notifications"), subtitle: text("选择任务完成时的提醒方式", "Choose how task completion is reported") },
    system: { title: text("系统与更新", "System & updates"), subtitle: text("管理系统集成与应用更新", "Manage system integration and application updates") },
    about: { title: text("关于轻压", "About QZip"), subtitle: "" }
  };
  const meta = sectionMeta[activeSection];
  const canReset = activeSection === "appearance" || activeSection === "archive" || activeSection === "notifications";

  return (
    <section className="qzip-settings">
      <aside className="qzip-settings__nav">
        <h1>{text("设置", "Settings")}</h1>
        <button type="button" data-active={activeSection === "appearance"} onClick={() => setActiveSection("appearance")}><ColorRegular fontSize={22} />{text("外观", "Appearance")}</button>
        <button type="button" data-active={activeSection === "archive"} onClick={() => setActiveSection("archive")}><ArchiveRegular fontSize={22} />{text("压缩与解压", "Compression & extraction")}</button>
        <button type="button" data-active={activeSection === "notifications"} onClick={() => setActiveSection("notifications")}><AlertRegular fontSize={22} />{text("通知", "Notifications")}</button>
        <button type="button" data-active={activeSection === "system"} onClick={() => setActiveSection("system")}><ShieldCheckmarkRegular fontSize={22} />{text("系统与更新", "System & updates")}</button>
        <button type="button" data-active={activeSection === "about"} onClick={() => setActiveSection("about")}><InfoRegular fontSize={22} />{text("关于", "About")}</button>
        <button type="button" className="qzip-settings__back" onClick={onBack}><ArrowLeftRegular fontSize={20} /> {text("返回首页", "Back to home")}</button>
      </aside>

      <main className="qzip-settings__content">
        <header className="qzip-settings__header">
          <div><h2>{meta.title}</h2>{meta.subtitle ? <p>{meta.subtitle}</p> : null}</div>
          {canReset ? <Button variant="tertiary" icon={<ArrowResetRegular fontSize={19} />} onClick={() => void settingsClient.reset().then(onChanged).then(() => onToast(text("设置已恢复默认值。", "Settings have been restored to defaults.")))}>{text("恢复默认设置", "Restore defaults")}</Button> : null}
        </header>

        {activeSection === "appearance" ? <AppearanceSettings settings={settings} patch={patch} /> : null}
        {activeSection === "archive" ? <ArchiveSettings settings={settings} patch={patch} /> : null}
        {activeSection === "notifications" ? <NotificationSettings settings={settings} patch={patch} updateNotifications={updateNotifications} /> : null}
        {activeSection === "system" ? <SystemSettings settings={settings} status={status} checking={checking} patch={patch} checkUpdates={checkUpdates} onToast={onToast} /> : null}
        {activeSection === "about" ? <AboutSettings status={status} checking={checking} checkUpdates={checkUpdates} /> : null}
      </main>
    </section>
  );
}

function AppearanceSettings({ settings, patch }: { settings: AppSettings; patch: (next: AppSettingsPatch) => Promise<void> }) {
  const { text } = useI18n();
  return <Card className="qzip-settings-panel"><SettingsSection icon={<ColorRegular fontSize={23} />} title={text("界面显示", "Display")}>
    <Row title={text("界面语言", "Language")}><SegmentedControl options={[{ value: "zh-CN", label: "简体中文" }, { value: "en-US", label: "English" }, { value: "system", label: text("跟随系统", "System") }]} value={settings.language} onValueChange={(value) => void patch({ language: value as AppSettings["language"] })} ariaLabel={text("界面语言", "Language")} /></Row>
    <Row title={text("主题模式", "Theme mode")}><SegmentedControl options={[{ value: "light", label: text("浅色", "Light") }, { value: "dark", label: text("暗夜", "Dark") }, { value: "system", label: text("跟随系统", "System") }]} value={settings.themeMode} onValueChange={(value) => void patch({ themeMode: value as AppSettings["themeMode"] })} ariaLabel={text("主题模式", "Theme mode")} /></Row>
    <Row title={text("强调色", "Accent color")}><SegmentedControl options={[{ value: "mint", label: text("薄荷", "Mint") }, { value: "ocean", label: text("海洋", "Ocean") }, { value: "lavender", label: text("薰衣草", "Lavender") }, { value: "amber", label: text("琥珀", "Amber") }, { value: "coral", label: text("珊瑚", "Coral") }, { value: "cyan-slate", label: text("青灰", "Cyan slate") }]} value={settings.accentTheme} onValueChange={(value) => void patch({ accentTheme: value as AppSettings["accentTheme"] })} ariaLabel={text("强调色", "Accent color")} /></Row>
    <Row title={text("界面缩放", "Interface scale")}><SegmentedControl options={[{ value: "scale90", label: "90%" }, { value: "scale100", label: "100%" }, { value: "scale110", label: "110%" }, { value: "scale125", label: "125%" }]} value={settings.uiScale} onValueChange={(value) => void patch({ uiScale: value as AppSettings["uiScale"] })} ariaLabel={text("界面缩放", "Interface scale")} /></Row>
    <Row title={text("列表密度", "List density")}><SegmentedControl options={[{ value: "comfortable", label: text("舒适", "Comfortable") }, { value: "compact", label: text("紧凑", "Compact") }]} value={settings.listDensity} onValueChange={(value) => void patch({ listDensity: value as AppSettings["listDensity"] })} ariaLabel={text("列表密度", "List density")} /></Row>
    <Toggle checked={settings.reduceMotion} onChange={(reduceMotion) => void patch({ reduceMotion })} label={text("减少动效", "Reduce motion")} hint={text("降低界面动画和过渡效果。", "Reduce interface animations and transitions.")} />
  </SettingsSection></Card>;
}

function ArchiveSettings({ settings, patch }: { settings: AppSettings; patch: (next: AppSettingsPatch) => Promise<void> }) {
  const { text } = useI18n();
  return <Card className="qzip-settings-panel"><SettingsSection icon={<ArchiveRegular fontSize={23} />} title={text("默认行为", "Default behavior")}>
    <Row title={text("默认压缩格式", "Default archive format")}><SegmentedControl options={formatOptions} value={settings.defaultFormat} onValueChange={(value) => void patch({ defaultFormat: value as AppSettings["defaultFormat"] })} ariaLabel={text("默认压缩格式", "Default archive format")} /></Row>
    <Row title={text("压缩等级", "Compression level")}><SegmentedControl options={[{ value: "fast", label: text("快速", "Fast") }, { value: "balanced", label: text("均衡", "Balanced") }, { value: "small", label: text("更小", "Smaller") }]} value={settings.compressionProfile} onValueChange={(value) => void patch({ compressionProfile: value as AppSettings["compressionProfile"] })} ariaLabel={text("默认压缩等级", "Compression level")} /></Row>
    <Row title={text("冲突文件处理", "File conflicts")}><SegmentedControl options={[{ value: "rename", label: text("自动重命名", "Auto rename") }, { value: "overwrite", label: text("覆盖", "Overwrite") }, { value: "skip", label: text("跳过", "Skip") }]} value={settings.conflictPolicy} onValueChange={(value) => void patch({ conflictPolicy: value as AppSettings["conflictPolicy"] })} ariaLabel={text("冲突文件处理", "File conflicts")} /></Row>
    <Toggle checked={settings.testAfterCreate} onChange={(testAfterCreate) => void patch({ testAfterCreate })} label={text("创建完成后测试压缩包", "Test archive after creation")} />
    <Toggle checked={settings.extractToNamedFolder} onChange={(extractToNamedFolder) => void patch({ extractToNamedFolder })} label={text("解压到同名文件夹", "Extract to a folder with the same name")} />
    <Toggle checked={settings.avoidDuplicateRootFolder} onChange={(avoidDuplicateRootFolder) => void patch({ avoidDuplicateRootFolder })} label={text("避免重复根目录", "Avoid duplicate root folders")} />
    <Toggle checked={settings.openFolderAfterExtract} onChange={(openFolderAfterExtract) => void patch({ openFolderAfterExtract })} label={text("解压完成后打开文件夹", "Open folder after extraction")} />
  </SettingsSection></Card>;
}

function NotificationSettings({ settings, patch, updateNotifications }: { settings: AppSettings; patch: (next: AppSettingsPatch) => Promise<void>; updateNotifications: (enabled: boolean) => Promise<void> }) {
  const { text } = useI18n();
  return <Card className="qzip-settings-panel"><SettingsSection icon={<AlertRegular fontSize={23} />} title={text("任务提醒", "Task alerts")}>
    <Toggle checked={settings.taskNotificationsEnabled} onChange={(enabled) => void updateNotifications(enabled)} label={text("任务完成通知", "Task notifications")} hint={text("启用时会请求操作系统通知权限。", "Enabling this requests operating system notification permission.")} />
    <Toggle checked={settings.notifyOnSuccess} onChange={(notifyOnSuccess) => void patch({ notifyOnSuccess })} disabled={!settings.taskNotificationsEnabled} label={text("成功时通知", "Notify on success")} />
    <Toggle checked={settings.notifyOnFailure} onChange={(notifyOnFailure) => void patch({ notifyOnFailure })} disabled={!settings.taskNotificationsEnabled} label={text("失败时通知", "Notify on failure")} />
  </SettingsSection></Card>;
}

function SystemSettings({ settings, status, checking, patch, checkUpdates, onToast }: { settings: AppSettings; status: IntegrationStatus | null; checking: boolean; patch: (next: AppSettingsPatch) => Promise<void>; checkUpdates: () => Promise<void>; onToast: (message: string) => void }) {
  const { text } = useI18n();
  return <div className="qzip-settings-card-stack">
    <Card className="qzip-settings-feature-card">
      <SettingsCardHeader icon={<AppsSettingsRegular fontSize={22} />} title={text("系统集成", "System integration")} />
      <StatusRow label={text("文件关联", "File associations")} status={status?.fileAssociationsDeclared ? text("已配置", "Configured") : text("未配置", "Not configured")} tone={status?.fileAssociationsDeclared ? "success" : "neutral"}>
        <Button variant="tertiary" icon={<OpenRegular fontSize={18} />} onClick={() => void settingsClient.openDefaultApps().catch((reason) => onToast(String(reason)))}>{text("默认应用设置", "Default apps")}</Button>
      </StatusRow>
      <StatusRow label={`Windows 11 ${text("右键菜单", "context menu")}`} status={!status ? text("正在检查", "Checking") : status.modernContextMenuRegistered ? text("已注册", "Registered") : status.modernContextMenuAvailable ? text("未注册", "Not registered") : text("当前版本不可用", "Unavailable in this build")} tone={status?.modernContextMenuRegistered ? "success" : "neutral"} />
    </Card>
    <Card className="qzip-settings-feature-card">
      <SettingsCardHeader icon={<ArrowSyncRegular fontSize={22} />} title={text("应用更新", "Application updates")} />
      <div className="qzip-update-version"><span>{text("当前版本", "Current version")}</span><strong>{status?.appVersion ?? "1.1.2"}</strong><span className="qzip-status-pill" data-tone={status?.updaterConfigured ? "success" : "neutral"}>{status?.updaterConfigured ? text("更新服务可用", "Updater ready") : text("手动更新", "Manual updates")}</span></div>
      <Toggle checked={settings.checkUpdatesOnStartup} onChange={(checkUpdatesOnStartup) => void patch({ checkUpdatesOnStartup })} label={text("启动时检查更新", "Check for updates at startup")} disabled={!status?.updaterConfigured} hint={status?.updaterConfigured ? text("有新版本时会提醒你。", "You will be notified when a new version is available.") : text("当前版本请通过 GitHub 获取更新。", "Get updates for this build through GitHub.")} />
      <div className="qzip-update-action"><Button variant="secondary" icon={<ArrowSyncRegular fontSize={18} />} loading={checking} disabled={!status?.updaterConfigured} onClick={() => void checkUpdates()}>{text("检查更新", "Check for updates")}</Button></div>
    </Card>
  </div>;
}

function AboutSettings({ status, checking, checkUpdates }: { status: IntegrationStatus | null; checking: boolean; checkUpdates: () => Promise<void> }) {
  const { text } = useI18n();
  const version = status?.appVersion ?? "1.1.2";
  return <div className="qzip-about-page">
    <Card className="qzip-about-hero">
      <img src={appIcon} alt={text("轻压应用图标", "QZip application icon")} />
      <h3>{text("轻压 · QZip", "QZip")}</h3>
      <span className="qzip-about-version">{text(`版本 ${version}`, `Version ${version}`)}</span>
      <p>{text("轻巧、漂亮、专注本地的压缩与解压工具", "A lightweight, beautiful, local-first compression tool")}</p>
    </Card>
    <Card className="qzip-about-links" aria-label={text("关于轻压的链接", "About QZip links")}>
      <AboutLink href="https://github.com/isunky/QZip" icon={<CodeRegular fontSize={21} />} label={text("GitHub 项目", "GitHub project")} />
      <button type="button" onClick={() => void checkUpdates()} disabled={checking}><ArrowSyncRegular fontSize={21} /><span>{text("检查更新", "Check for updates")}</span><ChevronRightRegular fontSize={18} /></button>
      <AboutLink href="https://github.com/isunky/QZip/blob/main/LICENSE" icon={<DocumentTextRegular fontSize={21} />} label={text("开源许可", "Open-source license")} />
      <AboutLink href="https://github.com/isunky/QZip/issues" icon={<ChatHelpRegular fontSize={21} />} label={text("问题反馈", "Report an issue")} />
    </Card>
    <p className="qzip-about-footer">{text("免费 · 无广告 · 本地优先", "Free · Ad-free · Local-first")}</p>
  </div>;
}

function SettingsSection({ icon, title, children }: { icon: React.ReactNode; title: string; children: React.ReactNode }) {
  return <section className="qzip-settings-section"><header>{icon}<h3>{title}</h3></header>{children}</section>;
}

function SettingsCardHeader({ icon, title }: { icon: React.ReactNode; title: string }) {
  return <header className="qzip-settings-feature-card__header">{icon}<h3>{title}</h3></header>;
}

function StatusRow({ label, status, tone, children }: { label: string; status: string; tone: "success" | "neutral"; children?: React.ReactNode }) {
  return <div className="qzip-setting-status"><strong>{label}</strong><span className="qzip-status-pill" data-tone={tone}>{status}</span>{children ? <div>{children}</div> : null}</div>;
}

function AboutLink({ href, icon, label }: { href: string; icon: React.ReactNode; label: string }) {
  return <a href={href} target="_blank" rel="noreferrer">{icon}<span>{label}</span><ChevronRightRegular fontSize={18} /></a>;
}

function Row({ title, children }: { title: string; children: React.ReactNode }) {
  return <div className="qzip-setting-row"><strong>{title}</strong>{children}</div>;
}
