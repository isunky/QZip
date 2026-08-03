import { Info, X } from "lucide-react";
import { Button } from "@qzip/ui";
import { useI18n } from "../lib/i18n";

interface ToastProps {
  message: string;
  onClose: () => void;
}

export function Toast({ message, onClose }: ToastProps) {
  const { text } = useI18n();
  return (
    <div className="qzip-toast" role="status">
      <Info size={18} aria-hidden="true" />
      <span>{message}</span>
      <Button
        variant="icon"
        aria-label={text("关闭提示", "Dismiss notification")}
        title={text("关闭", "Close")}
        icon={<X size={16} />}
        onClick={onClose}
      />
    </div>
  );
}
