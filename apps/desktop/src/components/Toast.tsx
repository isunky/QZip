import { Info, X } from "lucide-react";
import { Button } from "@qzip/ui";

interface ToastProps {
  message: string;
  onClose: () => void;
}

export function Toast({ message, onClose }: ToastProps) {
  return (
    <div className="qzip-toast" role="status">
      <Info size={18} aria-hidden="true" />
      <span>{message}</span>
      <Button
        variant="icon"
        aria-label="关闭提示"
        title="关闭"
        icon={<X size={16} />}
        onClick={onClose}
      />
    </div>
  );
}
