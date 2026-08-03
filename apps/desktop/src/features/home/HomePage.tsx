import {
  ArchiveRegular,
  FolderOpenRegular,
  ShieldCheckmarkRegular
} from "@fluentui/react-icons";
import { Button, Card } from "@qzip/ui";
import archiveHero from "../../assets/archive-hero.png";
import { useI18n } from "../../lib/i18n";

interface HomePageProps {
  onCreate?: () => void;
  onOpenArchive?: () => void;
  onUnavailable?: () => void;
}

export function HomePage({ onCreate, onOpenArchive, onUnavailable }: HomePageProps) {
  const { text } = useI18n();
  const openCreate = onCreate ?? onUnavailable ?? (() => undefined);
  const openArchive = onOpenArchive ?? onUnavailable ?? (() => undefined);
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
    openArchive();
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
          alt={text("装有文件的绿色压缩包插画", "Green archive containing files")}
          className="qzip-home-card__illustration"
        />
      </div>
      <h1>{text("将文件拖到这里", "Drop files here")}</h1>
      <p className="qzip-home-card__subtitle">{text("快速压缩、解压与浏览压缩包", "Compress, extract, and browse archives quickly")}</p>
      <div className="qzip-home-card__actions">
        <Button
          className="qzip-home-card__action"
          icon={<ArchiveRegular fontSize={29} />}
          onClick={openCreate}
        >
          {text("压缩文件", "Compress files")}
        </Button>
        <Button
          className="qzip-home-card__action"
          variant="secondary"
          icon={<FolderOpenRegular fontSize={30} />}
          onClick={openArchive}
        >
          {text("打开压缩包", "Open archive")}
        </Button>
      </div>
      <p className="qzip-home-card__format-hint">
        <ShieldCheckmarkRegular fontSize={22} aria-hidden="true" />
        {text("支持 7Z、ZIP、RAR、TAR 等常用格式", "Supports 7Z, ZIP, RAR, TAR, and other common formats")}
      </p>
      <span className="qzip-home-card__drop-hint" aria-hidden="true">
        <ArchiveRegular fontSize={20} />
        {text("松开以选择处理方式", "Drop to choose an action")}
      </span>
    </Card>
  );
}
