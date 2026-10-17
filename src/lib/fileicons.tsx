import type { LucideIcon } from "lucide-react";
import {
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileText,
  FileVideo,
  Folder,
} from "lucide-react";

const RULES: Array<[string[], LucideIcon, string]> = [
  [
    ["zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "xz", "zst", "iso", "cab", "lz4", "br", "jar"],
    FileArchive,
    "text-amber-500",
  ],
  [["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "ico", "tif", "tiff", "heic", "avif"], FileImage, "text-emerald-500"],
  [["mp4", "mkv", "avi", "mov", "wmv", "webm", "m4v", "flv", "mpg", "mpeg"], FileVideo, "text-violet-500"],
  [["mp3", "wav", "flac", "aac", "ogg", "m4a", "wma", "opus", "mid"], FileAudio, "text-pink-500"],
  [
    [
      "js", "ts", "tsx", "jsx", "rs", "py", "c", "cpp", "h", "hpp", "cs", "java", "go", "rb", "php",
      "html", "css", "scss", "json", "xml", "yml", "yaml", "toml", "sh", "ps1", "bat", "lua", "sql",
    ],
    FileCode,
    "text-sky-500",
  ],
  [
    ["txt", "md", "log", "ini", "cfg", "conf", "csv", "rtf", "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx"],
    FileText,
    "text-zinc-400",
  ],
];

export function fileVisual(name: string, isDir: boolean): { Icon: LucideIcon; cls: string } {
  if (isDir) return { Icon: Folder, cls: "text-amber-400" };
  const ext = name.split(".").pop()?.toLowerCase() ?? "";
  for (const [exts, Icon, cls] of RULES) {
    if (exts.includes(ext)) return { Icon, cls };
  }
  return { Icon: File, cls: "text-zinc-400" };
}
