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
          onToast("未获得系统通知权限，设置未更改。");
          return;
        }
      } catch {
        onToast("当前系统不支持请求通知权限。");
        return;
      }
    }
    await patch({ taskNotificationsEnabled: enabled });
  }
  async function checkUpdates() {
    setChecking(true);
    try {
      const result = await settingsClient.checkForUpdates();
      onToast(result.configured ? "更新服务已配置，将开始检查。" : "当前发行包未配置官方更新服务。");
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
        <h1>设置</h1>
        <button type="button" data-active={activeSection === "appearance"} onClick={() => showSection("appearance")}><ColorRegular fontSize={22} />外观</button>
        <button type="button" data-active={activeSection === "archive"} onClick={() => showSection("archive")}><ArchiveRegular fontSize={22} />压缩与解压</button>
        <button type="button" data-active={activeSection === "notifications"} onClick={() => showSection("notifications")}><AlertRegular fontSize={22} />通知</button>
        <button type="button" data-active={activeSection === "system"} onClick={() => showSection("system")}><ShieldCheckmarkRegular fontSize={22} />系统与隐私</button>
        <button type="button" data-active={activeSection === "about"} onClick={() => showSection("about")}><InfoRegular fontSize={22} />关于</button>
        <button type="button" className="qzip-settings__back" onClick={onBack}><ArrowLeftRegular fontSize={20} /> 返回首页</button>
      </aside>

      <main className="qzip-settings__content">
        <header className="qzip-settings__header">
          <h2>偏好设置</h2>
          <Button variant="tertiary" icon={<ArrowResetRegular fontSize={19} />} onClick={() => void settingsClient.reset().then(onChanged).then(() => onToast("设置已恢复默认值。"))}>恢复默认设置</Button>
        </header>

        <Card className="qzip-settings-panel">
          <SettingsSection id="appearance" icon={<ColorRegular fontSize={23} />} title="外观">
            <Row title="主题模式"><SegmentedControl options={[{ value: "light", label: "浅色" }, { value: "dark", label: "暗夜" }, { value: "system", label: "跟随系统" }]} value={settings.themeMode} onValueChange={(value) => void patch({ themeMode: value as AppSettings["themeMode"] })} ariaLabel="主题模式" /></Row>
            <Row title="强调色"><SegmentedControl options={[{ value: "mint", label: "薄荷" }, { value: "ocean", label: "海洋" }, { value: "lavender", label: "薰衣草" }, { value: "amber", label: "琥珀" }, { value: "coral", label: "珊瑚" }, { value: "cyan-slate", label: "青灰" }]} value={settings.accentTheme} onValueChange={(value) => void patch({ accentTheme: value as AppSettings["accentTheme"] })} ariaLabel="强调色" /></Row>
            <Row title="界面缩放"><SegmentedControl options={[{ value: "scale90", label: "90%" }, { value: "scale100", label: "100%" }, { value: "scale110", label: "110%" }, { value: "scale125", label: "125%" }]} value={settings.uiScale} onValueChange={(value) => void patch({ uiScale: value as AppSettings["uiScale"] })} ariaLabel="界面缩放" /></Row>
            <Row title="列表密度"><SegmentedControl options={[{ value: "comfortable", label: "舒适" }, { value: "compact", label: "紧凑" }]} value={settings.listDensity} onValueChange={(value) => void patch({ listDensity: value as AppSettings["listDensity"] })} ariaLabel="列表密度" /></Row>
            <Toggle checked={settings.reduceMotion} onChange={(reduceMotion) => void patch({ reduceMotion })} label="减少动效" hint="降低界面动画和过渡效果。" />
          </SettingsSection>

          <SettingsSection id="archive" icon={<ArchiveRegular fontSize={23} />} title="压缩与解压">
            <Row title="默认压缩格式"><SegmentedControl options={formatOptions} value={settings.defaultFormat} onValueChange={(value) => void patch({ defaultFormat: value as AppSettings["defaultFormat"] })} ariaLabel="默认压缩格式" /></Row>
            <Row title="压缩等级"><SegmentedControl options={[{ value: "fast", label: "快速" }, { value: "balanced", label: "均衡" }, { value: "small", label: "更小" }]} value={settings.compressionProfile} onValueChange={(value) => void patch({ compressionProfile: value as AppSettings["compressionProfile"] })} ariaLabel="默认压缩等级" /></Row>
            <Row title="冲突文件处理"><SegmentedControl options={[{ value: "rename", label: "自动重命名" }, { value: "overwrite", label: "覆盖" }, { value: "skip", label: "跳过" }]} value={settings.conflictPolicy} onValueChange={(value) => void patch({ conflictPolicy: value as AppSettings["conflictPolicy"] })} ariaLabel="冲突文件处理" /></Row>
            <Toggle checked={settings.testAfterCreate} onChange={(testAfterCreate) => void patch({ testAfterCreate })} label="创建完成后测试压缩包" />
            <Toggle checked={settings.extractToNamedFolder} onChange={(extractToNamedFolder) => void patch({ extractToNamedFolder })} label="解压到同名文件夹" />
            <Toggle checked={settings.avoidDuplicateRootFolder} onChange={(avoidDuplicateRootFolder) => void patch({ avoidDuplicateRootFolder })} label="避免重复根目录" />
            <Toggle checked={settings.openFolderAfterExtract} onChange={(openFolderAfterExtract) => void patch({ openFolderAfterExtract })} label="解压完成后打开文件夹" />
          </SettingsSection>

          <SettingsSection id="notifications" icon={<AlertRegular fontSize={23} />} title="通知">
            <Toggle checked={settings.taskNotificationsEnabled} onChange={(enabled) => void updateNotifications(enabled)} label="任务完成通知" hint="启用时会请求操作系统通知权限。" />
            <Toggle checked={settings.notifyOnSuccess} onChange={(notifyOnSuccess) => void patch({ notifyOnSuccess })} disabled={!settings.taskNotificationsEnabled} label="成功时通知" />
            <Toggle checked={settings.notifyOnFailure} onChange={(notifyOnFailure) => void patch({ notifyOnFailure })} disabled={!settings.taskNotificationsEnabled} label="失败时通知" />
          </SettingsSection>

          <SettingsSection id="system" icon={<ShieldCheckmarkRegular fontSize={23} />} title="系统、更新与隐私">
            <div className="qzip-setting-status"><span>文件关联</span><strong>{status?.fileAssociationsDeclared ? "安装包已声明常见压缩格式" : "当前平台未声明"}</strong><Button variant="tertiary" icon={<OpenRegular fontSize={18} />} onClick={() => void settingsClient.openDefaultApps().catch((reason) => onToast(String(reason)))}>默认应用设置</Button></div>
            <div className="qzip-setting-status"><span>Windows 11 右键菜单</span><strong>{status?.modernContextMenuRegistered ? "已注册" : "未注册或当前安装包不包含系统集成"}</strong></div>
            <Toggle checked={settings.checkUpdatesOnStartup} onChange={(checkUpdatesOnStartup) => void patch({ checkUpdatesOnStartup })} label="启动时检查更新" disabled={!status?.updaterConfigured} hint={status?.updaterConfigured ? "仅官方签名发行包可用。" : "当前发行包尚未配置官方更新服务。"} />
            <div className="qzip-setting-status"><span>更新服务</span><strong>{status?.updaterConfigured ? "已配置" : "未配置"}</strong><Button variant="secondary" loading={checking} disabled={!status?.updaterConfigured} onClick={() => void checkUpdates()}>检查更新</Button></div>
            <Toggle checked={false} onChange={() => undefined} disabled label="使用情况遥测" hint="QZip 不收集遥测数据，此项永久关闭。" />
          </SettingsSection>

          <SettingsSection id="about" icon={<InfoRegular fontSize={23} />} title="关于">
            <div className="qzip-about">
              <span>QZip {status?.appVersion ?? "1.0.0-rc.2"}</span>
              <span>本地优先的压缩与解压工具</span>
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

function Row({ title, children }: { title: string; children: React.ReactNode }) {
  return <div className="qzip-setting-row"><strong>{title}</strong>{children}</div>;
}
