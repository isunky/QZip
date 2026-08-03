import {
  Check,
  Monitor,
  Moon,
  Palette,
  Sun,
  X
} from "lucide-react";
import {
  Button,
  SegmentedControl,
  accentThemes,
  type AccentTheme,
  type ThemeMode
} from "@qzip/ui";
import { useI18n } from "../lib/i18n";

interface ThemePopoverProps {
  mode: ThemeMode;
  accent: AccentTheme;
  onModeChange: (mode: ThemeMode) => void;
  onAccentChange: (accent: AccentTheme) => void;
  onClose: () => void;
}

export function ThemePopover({
  mode,
  accent,
  onModeChange,
  onAccentChange,
  onClose
}: ThemePopoverProps) {
  const { text } = useI18n();
  const accentLabels: Record<AccentTheme, string> = {
    mint: text("薄荷绿", "Mint"),
    ocean: text("海洋蓝", "Ocean"),
    lavender: text("紫藤", "Lavender"),
    amber: text("琥珀橙", "Amber"),
    coral: text("珊瑚红", "Coral"),
    "cyan-slate": text("青灰", "Cyan slate")
  };
  return (
    <aside className="qzip-popover qzip-popover--theme" aria-label={text("外观设置", "Appearance settings")}>
      <div className="qzip-popover__header">
        <span className="qzip-popover__title">
          <Palette size={19} />
          {text("外观", "Appearance")}
        </span>
        <Button
          variant="icon"
          aria-label={text("关闭外观设置", "Close appearance settings")}
          title={text("关闭", "Close")}
          icon={<X size={18} />}
          onClick={onClose}
        />
      </div>
      <div className="qzip-popover__section">
        <span className="qzip-popover__label">{text("显示模式", "Display mode")}</span>
        <SegmentedControl
          ariaLabel={text("显示模式", "Display mode")}
          value={mode}
          onValueChange={onModeChange}
          options={[
            { value: "light", label: text("浅色", "Light") },
            { value: "dark", label: text("暗夜", "Dark") },
            { value: "system", label: text("系统", "System") }
          ]}
        />
        <div className="qzip-mode-icons" aria-hidden="true">
          <Sun size={15} />
          <Moon size={15} />
          <Monitor size={15} />
        </div>
      </div>
      <div className="qzip-popover__section">
        <span className="qzip-popover__label">{text("主题色", "Accent color")}</span>
        <div className="qzip-theme-grid" role="radiogroup" aria-label={text("主题色", "Accent color")}>
          {accentThemes.map((item) => (
            <button
              key={item}
              type="button"
              role="radio"
              aria-checked={accent === item}
              className="qzip-theme-swatch"
              data-theme={item}
              onClick={() => onAccentChange(item)}
            >
              <span className="qzip-theme-swatch__dot">
                {accent === item ? <Check size={14} /> : null}
              </span>
              <span>{accentLabels[item]}</span>
            </button>
          ))}
        </div>
      </div>
    </aside>
  );
}
