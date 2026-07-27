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

interface ThemePopoverProps {
  mode: ThemeMode;
  accent: AccentTheme;
  onModeChange: (mode: ThemeMode) => void;
  onAccentChange: (accent: AccentTheme) => void;
  onClose: () => void;
}

const accentLabels: Record<AccentTheme, string> = {
  mint: "薄荷绿",
  ocean: "海洋蓝",
  lavender: "紫藤",
  amber: "琥珀橙",
  coral: "珊瑚红",
  "cyan-slate": "青灰"
};

export function ThemePopover({
  mode,
  accent,
  onModeChange,
  onAccentChange,
  onClose
}: ThemePopoverProps) {
  return (
    <aside className="qzip-popover qzip-popover--theme" aria-label="外观设置">
      <div className="qzip-popover__header">
        <span className="qzip-popover__title">
          <Palette size={19} />
          外观
        </span>
        <Button
          variant="icon"
          aria-label="关闭外观设置"
          title="关闭"
          icon={<X size={18} />}
          onClick={onClose}
        />
      </div>
      <div className="qzip-popover__section">
        <span className="qzip-popover__label">显示模式</span>
        <SegmentedControl
          ariaLabel="显示模式"
          value={mode}
          onValueChange={onModeChange}
          options={[
            { value: "light", label: "浅色" },
            { value: "dark", label: "暗夜" },
            { value: "system", label: "系统" }
          ]}
        />
        <div className="qzip-mode-icons" aria-hidden="true">
          <Sun size={15} />
          <Moon size={15} />
          <Monitor size={15} />
        </div>
      </div>
      <div className="qzip-popover__section">
        <span className="qzip-popover__label">主题色</span>
        <div className="qzip-theme-grid" role="radiogroup" aria-label="主题色">
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
