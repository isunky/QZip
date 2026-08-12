import { useEffect, useState } from "react";
import {
  AlertRegular,
  ArchiveRegular,
  ArrowLeftRegular,
  ArrowResetRegular,
  ChevronRightRegular,
  ColorRegular,
  InfoRegular,
  OpenRegular,
  ShieldCheckmarkRegular
} from "@fluentui/react-icons";
import { Button, Card, SegmentedControl } from "@qzip/ui";
import type { AppSettings, AppSettingsPatch, IntegrationStatus } from "../../contracts/settings";
import { settingsClient } from "../../lib/settingsClient";
import { useI18n } from "../../lib/i18n";

interface SettingsPageProps {
  settings: AppSettings;
  onBack: () => void;
  onChanged: (settings: AppSettings) => void;
  onToast: (message: string) => void;
}

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
  const { brandName, text } = useI18n();
  const [status, setStatus] = useState<IntegrationStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [activeSection, setActiveSection] = useState("appearance");

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
  function showSection(section: string) {
    setActiveSection(section);
    document.getElementById(`qzip-settings-${section}`)?.scrollIntoView({ behavior: settings.reduceMotion ? "auto" : "smooth", block: "start" });
  }

  return (
    <section className="qzip-settings">
      <aside className="qzip-settings__nav">
        <h1>{text("设置", "Settings")}</h1>
        <button type="button" data-active={activeSection === "appearance"} onClick={() => showSection("appearance")}><ColorRegular fontSize={22} />{text("外观", "Appearance")}</button>
        <button type="button" data-active={activeSection === "archive"} onClick={() => showSection("archive")}><ArchiveRegular fontSize={22} />{text("压缩与解压", "Compression & extraction")}</button>
        <button type="button" data-active={activeSection === "notifications"} onClick={() => showSection("notifications")}><AlertRegular fontSize={22} />{text("通知", "Notifications")}</button>
        <button type="button" data-active={activeSection === "system"} onClick={() => showSection("system")}><ShieldCheckmarkRegular fontSize={22} />{text("系统与隐私", "System & privacy")}</button>
        <button type="button" data-active={activeSection === "about"} onClick={() => showSection("about")}><InfoRegular fontSize={22} />{text("关于", "About")}</button>
        <button type="button" className="qzip-settings__back" onClick={onBack}><ArrowLeftRegular fontSize={20} /> {text("返回首页", "Back to home")}</button>
      </aside>

      <main className="qzip-settings__content">
        <header className="qzip-settings__header">
          <h2>{text("偏好设置", "Preferences")}</h2>
          <Button variant="tertiary" icon={<ArrowResetRegular fontSize={19} />} onClick={() => void settingsClient.reset().then(onChanged).then(() => onToast(text("设置已恢复默认值。", "Settings have been restored to defaults.")))}>{text("恢复默认设置", "Restore defaults")}</Button>
        </header>

        <Card className="qzip-settings-panel">
          <SettingsSection id="appearance" icon={<ColorRegular fontSize={23} />} title={text("外观", "Appearance")}>
            <div className="qzip-appearance-layout">
              <section className="qzip-appearance-group">
                <header className="qzip-appearance-group__header">
                  <div><strong>{text("界面风格", "Interface style")}</strong><span>{text("选择语言、明暗模式与界面主色", "Choose the language, theme and interface color")}</span></div>
                  <span className="qzip-appearance-group__mark" aria-hidden="true"><i /><i /><i /></span>
                </header>
                <div className="qzip-appearance-group__body">
                  <Row title={text("界面语言", "Language")} hint={text("应用菜单和提示文字", "Menus and interface text")}><SegmentedControl className="qzip-appearance-control" options={[{ value: "zh-CN", label: "简体中文" }, { value: "en-US", label: "English" }, { value: "system", label: text("跟随系统", "System") }]} value={settings.language} onValueChange={(value) => void patch({ language: value as AppSettings["language"] })} ariaLabel={text("界面语言", "Language")} /></Row>
                  <Row title={text("主题模式", "Theme mode")} hint={text("调整窗口明暗外观", "Adjust the window appearance")}><SegmentedControl className="qzip-appearance-control" options={[{ value: "light", label: text("浅色", "Light") }, { value: "dark", label: text("暗夜", "Dark") }, { value: "system", label: text("跟随系统", "System") }]} value={settings.themeMode} onValueChange={(value) => void patch({ themeMode: value as AppSettings["themeMode"] })} ariaLabel={text("主题模式", "Theme mode")} /></Row>
                  <Row title={text("强调色", "Accent color")} hint={text("用于按钮与选中状态", "Used for buttons and selections")}><SegmentedControl className="qzip-appearance-control qzip-appearance-accent" options={[{ value: "mint", label: text("薄荷", "Mint") }, { value: "ocean", label: text("海洋", "Ocean") }, { value: "lavender", label: text("薰衣草", "Lavender") }, { value: "amber", label: text("琥珀", "Amber") }, { value: "coral", label: text("珊瑚", "Coral") }, { value: "cyan-slate", label: text("青灰", "Cyan slate") }]} value={settings.accentTheme} onValueChange={(value) => void patch({ accentTheme: value as AppSettings["accentTheme"] })} ariaLabel={text("强调色", "Accent color")} /></Row>
                </div>
              </section>

              <section className="qzip-appearance-group">
                <header className="qzip-appearance-group__header">
                  <div><strong>{text("显示体验", "Display experience")}</strong><span>{text("根据屏幕和使用习惯调整内容呈现", "Tune content for your screen and workflow")}</span></div>
                  <span className="qzip-appearance-scale-preview" aria-hidden="true"><i>90</i><i>100</i><i>125</i></span>
                </header>
                <div className="qzip-appearance-group__body">
                  <Row title={text("界面缩放", "Interface scale")} hint={text("放大或缩小界面内容", "Resize interface content")}><SegmentedControl className="qzip-appearance-control" options={[{ value: "scale90", label: "90%" }, { value: "scale100", label: "100%" }, { value: "scale110", label: "110%" }, { value: "scale125", label: "125%" }]} value={settings.uiScale} onValueChange={(value) => void patch({ uiScale: value as AppSettings["uiScale"] })} ariaLabel={text("界面缩放", "Interface scale")} /></Row>
                  <Row title={text("列表密度", "List density")} hint={text("控制任务和文件列表间距", "Control spacing in task and file lists")}><SegmentedControl className="qzip-appearance-control" options={[{ value: "comfortable", label: text("舒适", "Comfortable") }, { value: "compact", label: text("紧凑", "Compact") }]} value={settings.listDensity} onValueChange={(value) => void patch({ listDensity: value as AppSettings["listDensity"] })} ariaLabel={text("列表密度", "List density")} /></Row>
                  <Toggle checked={settings.reduceMotion} onChange={(reduceMotion) => void patch({ reduceMotion })} label={text("减少动效", "Reduce motion")} hint={text("降低界面动画和过渡效果。", "Reduce interface animations and transitions.")} />
                </div>
              </section>
            </div>
          </SettingsSection>

          <SettingsSection id="archive" icon={<ArchiveRegular fontSize={23} />} title={text("压缩与解压", "Compression & extraction")}>
            <Row title={text("默认压缩格式", "Default archive format")}><SegmentedControl options={formatOptions} value={settings.defaultFormat} onValueChange={(value) => void patch({ defaultFormat: value as AppSettings["defaultFormat"] })} ariaLabel={text("默认压缩格式", "Default archive format")} /></Row>
            <Row title={text("压缩等级", "Compression level")}><SegmentedControl options={[{ value: "fast", label: text("快速", "Fast") }, { value: "balanced", label: text("均衡", "Balanced") }, { value: "small", label: text("更小", "Smaller") }]} value={settings.compressionProfile} onValueChange={(value) => void patch({ compressionProfile: value as AppSettings["compressionProfile"] })} ariaLabel={text("默认压缩等级", "Default compression level")} /></Row>
            <Row title={text("冲突文件处理", "File conflicts")}><SegmentedControl options={[{ value: "rename", label: text("自动重命名", "Auto rename") }, { value: "overwrite", label: text("覆盖", "Overwrite") }, { value: "skip", label: text("跳过", "Skip") }]} value={settings.conflictPolicy} onValueChange={(value) => void patch({ conflictPolicy: value as AppSettings["conflictPolicy"] })} ariaLabel={text("冲突文件处理", "File conflicts")} /></Row>
            <Toggle checked={settings.testAfterCreate} onChange={(testAfterCreate) => void patch({ testAfterCreate })} label={text("创建完成后测试压缩包", "Test archive after creation")} />
            <Toggle checked={settings.extractToNamedFolder} onChange={(extractToNamedFolder) => void patch({ extractToNamedFolder })} label={text("解压到同名文件夹", "Extract to a folder with the same name")} />
            <Toggle checked={settings.avoidDuplicateRootFolder} onChange={(avoidDuplicateRootFolder) => void patch({ avoidDuplicateRootFolder })} label={text("避免重复根目录", "Avoid duplicate root folders")} />
            <Toggle checked={settings.openFolderAfterExtract} onChange={(openFolderAfterExtract) => void patch({ openFolderAfterExtract })} label={text("解压完成后打开文件夹", "Open folder after extraction")} />
          </SettingsSection>

          <SettingsSection id="notifications" icon={<AlertRegular fontSize={23} />} title={text("通知", "Notifications")}>
            <Toggle checked={settings.taskNotificationsEnabled} onChange={(enabled) => void updateNotifications(enabled)} label={text("任务完成通知", "Task notifications")} hint={text("启用时会请求操作系统通知权限。", "Enabling this requests operating system notification permission.")} />
            <Toggle checked={settings.notifyOnSuccess} onChange={(notifyOnSuccess) => void patch({ notifyOnSuccess })} disabled={!settings.taskNotificationsEnabled} label={text("成功时通知", "Notify on success")} />
            <Toggle checked={settings.notifyOnFailure} onChange={(notifyOnFailure) => void patch({ notifyOnFailure })} disabled={!settings.taskNotificationsEnabled} label={text("失败时通知", "Notify on failure")} />
          </SettingsSection>

          <SettingsSection id="system" icon={<ShieldCheckmarkRegular fontSize={23} />} title={text("系统、更新与隐私", "System, updates & privacy")}>
            <div className="qzip-setting-status"><span>{text("文件关联", "File associations")}</span><strong>{status?.fileAssociationsDeclared ? text("安装时已选择文件关联", "File associations were selected during installation") : text("尚未选择文件关联", "No file associations selected")}</strong><Button variant="tertiary" icon={<OpenRegular fontSize={18} />} onClick={() => void settingsClient.openDefaultApps().catch((reason) => onToast(String(reason)))}>{text("默认应用设置", "Default apps")}</Button></div>
            <div className="qzip-setting-status"><span>Windows 11 {text("右键菜单", "context menu")}</span><strong>{status?.modernContextMenuRegistered ? text("已注册", "Registered") : text("未注册或当前安装包不包含系统集成", "Not registered or unavailable in this build")}</strong></div>
            <Toggle checked={settings.checkUpdatesOnStartup} onChange={(checkUpdatesOnStartup) => void patch({ checkUpdatesOnStartup })} label={text("启动时检查更新", "Check for updates at startup")} disabled={!status?.updaterConfigured} hint={status?.updaterConfigured ? text("仅官方签名发行包可用。", "Available only in officially signed releases.") : text("当前发行包尚未配置官方更新服务。", "The official update service is not configured for this build.")} />
            <div className="qzip-setting-status"><span>{text("更新服务", "Update service")}</span><strong>{status?.updaterConfigured ? text("已配置", "Configured") : text("未配置", "Not configured")}</strong><Button variant="secondary" loading={checking} disabled={!status?.updaterConfigured} onClick={() => void checkUpdates()}>{text("检查更新", "Check for updates")}</Button></div>
            <Toggle checked={false} onChange={() => undefined} disabled label={text("使用情况遥测", "Usage telemetry")} hint={text("轻压不收集遥测数据，此项永久关闭。", "QZip does not collect telemetry. This setting is permanently off.")} />
          </SettingsSection>

          <SettingsSection id="about" icon={<InfoRegular fontSize={23} />} title={text("关于", "About")}>
            <div className="qzip-about">
              <span>{brandName} {status?.appVersion ?? "1.0.0-rc.2"}</span>
              <span>{text("本地优先的压缩与解压工具", "A local-first compression and extraction tool")}</span>
              <a href="https://github.com/isunky/QZip" target="_blank" rel="noreferrer">GitHub <ChevronRightRegular fontSize={17} /></a>
            </div>
          </SettingsSection>
        </Card>
      </main>
    </section>
  );
}

function SettingsSection({ id, icon, title, children }: {
  id: string;
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section id={`qzip-settings-${id}`} className="qzip-settings-section">
      <header>{icon}<h3>{title}</h3></header>
      {children}
    </section>
  );
}

function Row({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) {
  return <div className="qzip-setting-row"><span className="qzip-setting-row__label"><strong>{title}</strong>{hint ? <small>{hint}</small> : null}</span>{children}</div>;
}
