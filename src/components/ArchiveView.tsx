import { useMemo, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  FileArchive,
  FlaskConical,
  FolderInput,
  Lock,
  Search,
} from "lucide-react";
import type { ArchiveEntry, Listing } from "../types";
import { fmtBytes, fmtDate } from "../lib/format";
import { fileVisual } from "../lib/fileicons";

const MAX_ROWS = 3000;

type SortKey = "name" | "size" | "modified";

export default function ArchiveView(props: {
  t: (k: string, vars?: Record<string, string | number>) => string;
  listing: Listing;
  selected: Set<string>;
  onToggle: (name: string) => void;
  onSetSelection: (names: string[]) => void;
  onExtract: () => void;
  onTest: () => void;
  busy: boolean;
}) {
  const { t, listing, selected, onToggle, onSetSelection, onExtract, onTest, busy } = props;
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: "name", dir: 1 });

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const arr = q ? listing.entries.filter((e) => e.name.toLowerCase().includes(q)) : listing.entries;
    return [...arr].sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      let c = 0;
      if (sort.key === "size") c = a.size - b.size;
      else if (sort.key === "modified") c = a.modified.localeCompare(b.modified);
      else c = a.name.localeCompare(b.name, undefined, { numeric: true, sensitivity: "base" });
      return c * sort.dir;
    });
  }, [listing, query, sort]);

  const shown: ArchiveEntry[] = filtered.length > MAX_ROWS ? filtered.slice(0, MAX_ROWS) : filtered;
  const allChecked = shown.length > 0 && shown.every((e) => selected.has(e.name));

  const selectedTotal = useMemo(
    () => listing.entries.filter((e) => selected.has(e.name) && !e.isDir).reduce((s, e) => s + e.size, 0),
    [listing, selected],
  );

  const clickSort = (key: SortKey) => {
    setSort((s) => (s.key === key ? { key, dir: s.dir === 1 ? -1 : 1 } : { key, dir: 1 }));
  };

  const toggleAll = () => {
    if (allChecked) {
      const shownSet = new Set(shown.map((e) => e.name));
      onSetSelection([...selected].filter((n) => !shownSet.has(n)));
    } else {
      onSetSelection([...new Set([...selected, ...shown.map((e) => e.name)])]);
    }
  };

  const sortIcon = (key: SortKey) =>
    sort.key === key ? (
      sort.dir === 1 ? (
        <ArrowUp size={11} className="text-indigo-500" />
      ) : (
        <ArrowDown size={11} className="text-indigo-500" />
      )
    ) : null;

  const info = listing.info;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="card flex h-14 shrink-0 items-center gap-3 px-4">
        <FileArchive size={18} className="shrink-0 text-indigo-500" />
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-medium">{info.name}</span>
            <span className="badge">{info.format}</span>
            {info.encrypted && <Lock size={12} className="shrink-0 text-amber-500" />}
          </div>
          <div className="text-xs text-zinc-400">
            {fmtBytes(info.physicalSize)} · {info.entries} {t("items")}
          </div>
        </div>
        <div className="flex-1" />
        <div className="relative">
          <Search size={14} className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400" />
          <input
            className="input w-52 pl-8"
            placeholder={t("searchPlaceholder")}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </div>
        <button className="btn btn-soft" disabled={busy} onClick={onTest}>
          <FlaskConical size={15} />
          {t("test")}
        </button>
        <button className="btn btn-primary" disabled={busy} onClick={onExtract}>
          <FolderInput size={15} />
          {t("extract")}
        </button>
      </div>

      <div className="card mt-3 min-h-0 flex-1 overflow-auto">
        <table className="w-full border-separate border-spacing-0 text-sm">
          <thead className="sticky top-0 z-10">
            <tr className="bg-white/95 text-left text-xs text-zinc-500 backdrop-blur dark:bg-zinc-900/95">
              <th className="w-9 border-b border-zinc-200 px-3 py-2 dark:border-zinc-800">
                <input
                  type="checkbox"
                  className="accent-indigo-600"
                  checked={allChecked}
                  onChange={toggleAll}
                  title={t("selectAll")}
                />
              </th>
              <th className="border-b border-zinc-200 px-2 py-2 dark:border-zinc-800">
                <button className="inline-flex items-center gap-1 hover:text-zinc-900 dark:hover:text-zinc-100" onClick={() => clickSort("name")}>
                  {t("colName")}
                  {sortIcon("name")}
                </button>
              </th>
              <th className="w-24 border-b border-zinc-200 px-2 py-2 text-right dark:border-zinc-800">
                <button className="inline-flex items-center gap-1 hover:text-zinc-900 dark:hover:text-zinc-100" onClick={() => clickSort("size")}>
                  {t("colSize")}
                  {sortIcon("size")}
                </button>
              </th>
              <th className="w-28 border-b border-zinc-200 px-2 py-2 text-right dark:border-zinc-800">{t("colPacked")}</th>
              <th className="w-36 border-b border-zinc-200 px-2 py-2 dark:border-zinc-800">
                <button className="inline-flex items-center gap-1 hover:text-zinc-900 dark:hover:text-zinc-100" onClick={() => clickSort("modified")}>
                  {t("colModified")}
                  {sortIcon("modified")}
                </button>
              </th>
            </tr>
          </thead>
          <tbody>
            {shown.map((e) => {
              const { Icon, cls } = fileVisual(e.name, e.isDir);
              const isSel = selected.has(e.name);
              return (
                <tr
                  key={e.name}
                  onClick={() => onToggle(e.name)}
                  className={`cursor-default ${isSel ? "bg-indigo-500/10" : "hover:bg-zinc-100/70 dark:hover:bg-zinc-800/40"}`}
                >
                  <td className="border-b border-zinc-100 px-3 py-1.5 dark:border-zinc-800/60">
                    <input type="checkbox" className="accent-indigo-600" checked={isSel} readOnly />
                  </td>
                  <td className="max-w-0 border-b border-zinc-100 px-2 py-1.5 dark:border-zinc-800/60" style={{ width: "50%" }}>
                    <div className="flex min-w-0 items-center gap-2">
                      <Icon size={15} className={`shrink-0 ${cls}`} />
                      <span className="truncate">{e.name}</span>
                      {e.encrypted && <Lock size={11} className="shrink-0 text-amber-500" />}
                    </div>
                  </td>
                  <td className="border-b border-zinc-100 px-2 py-1.5 text-right tabular-nums dark:border-zinc-800/60">
                    {e.isDir ? "" : fmtBytes(e.size)}
                  </td>
                  <td className="border-b border-zinc-100 px-2 py-1.5 text-right tabular-nums text-zinc-400 dark:border-zinc-800/60">
                    {e.isDir || !e.packedSize ? "" : fmtBytes(e.packedSize)}
                  </td>
                  <td className="border-b border-zinc-100 px-2 py-1.5 text-xs text-zinc-400 dark:border-zinc-800/60">{fmtDate(e.modified)}</td>
                </tr>
              );
            })}
            {shown.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-10 text-center text-sm text-zinc-400">
                  {listing.entries.length === 0 ? t("noEntries") : t("noMatch")}
                </td>
              </tr>
            )}
            {filtered.length > MAX_ROWS && (
              <tr>
                <td colSpan={5} className="px-4 py-2 text-center text-xs text-zinc-400">
                  {t("truncated", { n: MAX_ROWS })}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="mt-2 flex h-7 shrink-0 items-center gap-2 text-xs text-zinc-400">
        <span>
          {selected.size} {t("selectedCount")}
          {selected.size > 0 ? ` · ${t("totalSize")} ${fmtBytes(selectedTotal)}` : ""}
        </span>
        {selected.size > 0 && (
          <button className="text-indigo-500 hover:underline" onClick={() => onSetSelection([])}>
            {t("clear")}
          </button>
        )}
      </div>
    </div>
  );
}
