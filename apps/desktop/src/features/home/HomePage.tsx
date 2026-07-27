import {
  Archive,
  FileArchive,
  FolderOpen,
  ShieldCheck
} from "lucide-react";
import { Button, Card } from "@qzip/ui";
import archiveHero from "../../assets/archive-hero.png";

interface HomePageProps {
  onUnavailable: () => void;
}

export function HomePage({ onUnavailable }: HomePageProps) {
  function onDragOver(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.currentTarget.dataset.dragging = "true";
  }

  function clearDragState(event: React.DragEvent<HTMLElement>) {
    event.currentTarget.dataset.dragging = "false";
  }

  function onDrop(event: React.DragEvent<HTMLElement>) {
    event.preventDefault();
    event.currentTarget.dataset.dragging = "false";
    onUnavailable();
  }

  return (
    <Card
      elevated
      className="qzip-home-card"
      onDragOver={onDragOver}
      onDragEnter={onDragOver}
      onDragLeave={clearDragState}
      onDrop={onDrop}
    >
      <div className="qzip-home-card__hero">
        <img
          src={archiveHero}
          alt="装有文件的绿色压缩包插画"
          className="qzip-home-card__illustration"
        />
      </div>
      <h1>将文件拖到这里</h1>
      <p className="qzip-home-card__subtitle">快速压缩、解压与浏览压缩包</p>
      <div className="qzip-home-card__actions">
        <Button
          className="qzip-home-card__action"
          icon={<FileArchive size={28} />}
          onClick={onUnavailable}
        >
          压缩文件
        </Button>
        <Button
          className="qzip-home-card__action"
          variant="secondary"
          icon={<FolderOpen size={29} />}
          onClick={onUnavailable}
        >
          打开压缩包
        </Button>
      </div>
      <p className="qzip-home-card__format-hint">
        <ShieldCheck size={21} aria-hidden="true" />
        支持 7Z、ZIP、RAR、TAR 等常用格式
      </p>
      <span className="qzip-home-card__drop-hint" aria-hidden="true">
        <Archive size={20} />
        松开以选择处理方式
      </span>
    </Card>
  );
}
