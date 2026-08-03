import type { ArchiveFormat } from "../../contracts/archive";
import { localize, type AppLocale } from "../../lib/i18n";

export function splitOutputPath(path: string) {
  const index = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return index < 0
    ? { directory: "", name: path }
    : { directory: path.slice(0, index), name: path.slice(index + 1) };
}

export function joinOutputPath(directory: string, name: string) {
  if (!directory) return name;
  const separator = directory.includes("\\") ? "\\" : "/";
  return `${directory.replace(/[\\/]+$/, "")}${separator}${name}`;
}

function createExtension(format: ArchiveFormat) {
  switch (format) {
    case "sevenZip": return "7z";
    case "zip": return "zip";
    case "tar": return "tar";
    case "tarGz": return "tar.gz";
    case "tarXz": return "tar.xz";
    default: return "7z";
  }
}

export function suggestCreateOutputLocally(inputs: string[], format: ArchiveFormat, locale: AppLocale = "zh-CN") {
  const first = inputs[0]?.replace(/[\\/]+$/, "");
  if (!first) return null;
  const split = splitOutputPath(first);
  const leaf = split.name || localize(locale, "新建压缩包", "New archive");
  const lastDot = leaf.lastIndexOf(".");
  const stem = inputs.length > 1
    ? localize(locale, "压缩文件", "Archive")
    : lastDot > 0
      ? leaf.slice(0, lastDot)
      : leaf;
  return joinOutputPath(split.directory, `${stem}.${createExtension(format)}`);
}
