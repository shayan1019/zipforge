import { Boxes, FolderOpen, Plus } from "lucide-react";

const CHIPS = ["zip", "7z", "rar", "tar", "gz", "xz", "bz2", "zst", "iso", "cab", "jar"];

export default function EmptyState(props: {
  t: (k: string) => string;
  onOpen: () => void;
  onCreate: () => void;
}) {
  const { t, onOpen, onCreate } = props;
  return (
    <div className="flex h-full flex-col items-center justify-center gap-6">
      <div className="flex h-20 w-20 items-center justify-center rounded-3xl bg-gradient-to-br from-indigo-500 via-violet-500 to-fuchsia-500 shadow-lg shadow-indigo-500/25">
        <Boxes className="text-white" size={40} strokeWidth={1.8} />
      </div>
      <div className="text-center">
        <h1 className="text-2xl font-semibold tracking-tight">zipforge</h1>
        <p className="mt-1 text-sm font-medium text-zinc-600 dark:text-zinc-300">{t("dropTitle")}</p>
        <p className="mt-0.5 text-xs text-zinc-400">{t("dropHint")}</p>
      </div>
      <div className="flex gap-2">
        <button className="btn btn-primary" onClick={onOpen}>
          <FolderOpen size={16} />
          {t("open")}
        </button>
        <button className="btn btn-soft" onClick={onCreate}>
          <Plus size={16} />
          {t("create")}
        </button>
      </div>
      <div className="flex max-w-md flex-wrap justify-center gap-1.5">
        {CHIPS.map((c) => (
          <span key={c} className="badge">
            {c}
          </span>
        ))}
      </div>
    </div>
  );
}
