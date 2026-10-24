import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  FileArchive,
  FlaskConical,
  FolderInput,
  Lock,
  Search,
} from "lucide-react";
import type { Listing } from "../types";
import { fmtBytes, fmtDate } from "../lib/format";
import { fileVisual } from "../lib/fileicons";

/** Fixed row height (px) so the virtual window never has to measure the DOM. */
const ROW_H = 33;
/** Rows rendered above/below the viewport to keep fast scrolling smooth. */
const OVERSCAN = 8;

type SortKey = "name" | "size" | "modified";

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

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
  const [scrollTop, setScrollTop] = useState(0);
  const [viewH, setViewH] = useState(600);

  const bodyRef = useRef<HTMLDivElement | null>(null);
  const rafRef = useRef(0);

  // Filtering + sorting over 100k entries is the expensive part; keep it in
  // one memo so scrolling never recomputes it.
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    const arr = q ? listing.entries.filter((e) => e.name.toLowerCase().includes(q)) : listing.entries;
    return [...arr].sort((a, b) => {
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
      let c = 0;
      if (sort.key === "size") c = a.size - b.size;
      else if (sort.key === "modified") c = a.modified.localeCompare(b.modified);
      else c = collator.compare(a.name, b.name);
      return c * sort.dir;
    });
  }, [listing, query, sort]);

  const total = filtered.length;

  useLayoutEffect(() => {
    const el = bodyRef.current;
    if (!el) return;
    const measure = () => setViewH(el.clientHeight);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      ro.disconnect();
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
    };
  }, []);

  // Reset the scroll position when the content changes underneath us.
  useEffect(() => {
    const el = bodyRef.current;
    if (!el) return;
    el.scrollTop = 0;
    setScrollTop(0);
  }, [listing, query, sort]);

  useEffect(() => {
    const el = bodyRef.current;
    if (!el) return;
    const max = Math.max(0, total * ROW_H - el.clientHeight);
    if (el.scrollTop > max) {
      el.scrollTop = max;
      setScrollTop(max);
    }
  }, [total]);

  const onScroll = () => {
    if (rafRef.current) return;
    rafRef.current = requestAnimationFrame(() => {
      rafRef.current = 0;
      setScrollTop(bodyRef.current?.scrollTop ?? 0);
    });
  };

  const start = Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN);
  const end = Math.min(total, Math.ceil((scrollTop + viewH) / ROW_H) + OVERSCAN);
  const visible = useMemo(() => filtered.slice(start, end), [filtered, start, end]);

  // O(1) lookups instead of scanning every entry on each selection change.
  const sizeByName = useMemo(() => {
    const m = new Map<string, number>();
    for (const e of listing.entries) if (!e.isDir) m.set(e.name, e.size);
    return m;
  }, [listing]);

  const selectedTotal = useMemo(() => {
    let sum = 0;
    for (const n of selected) sum += sizeByName.get(n) ?? 0;
    return sum;
  }, [selected, sizeByName]);

  const allChecked = useMemo(
    () => total > 0 && filtered.every((e) => selected.has(e.name)),
    [filtered, total, selected],
  );

  const clickSort = (key: SortKey) => {
    setSort((s) => (s.key === key ? { key, dir: s.dir === 1 ? -1 : 1 } : { key, dir: 1 }));
  };

  const toggleAll = () => {
    if (allChecked) {
      const shownSet = new Set(filtered.map((e) => e.name));
      onSetSelection([...selected].filter((n) => !shownSet.has(n)));
    } else {
      onSetSelection([...new Set([...selected, ...filtered.map((e) => e.name)])]);
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
  const padTop = start * ROW_H;
  const padBottom = Math.max(0, (total - end) * ROW_H);

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
            {query.trim() ? ` · ${total} / ${info.entries}` : ""}
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

      <div ref={bodyRef} onScroll={onScroll} className="card mt-3 min-h-0 flex-1 overflow-auto">
        <table className="w-full table-fixed border-separate border-spacing-0 text-sm">
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
            {padTop > 0 && (
              <tr aria-hidden style={{ height: padTop }}>
                <td colSpan={5} style={{ padding: 0, border: 0 }} />
              </tr>
            )}
            {visible.map((e) => {
              const { Icon, cls } = fileVisual(e.name, e.isDir);
              const isSel = selected.has(e.name);
              return (
                <tr
                  key={e.name}
                  onClick={() => onToggle(e.name)}
                  style={{ height: ROW_H }}
                  className={`cursor-default ${isSel ? "bg-indigo-500/10" : "hover:bg-zinc-100/70 dark:hover:bg-zinc-800/40"}`}
                >
                  <td className="border-b border-zinc-100 px-3 py-1.5 dark:border-zinc-800/60">
                    <input type="checkbox" className="accent-indigo-600" checked={isSel} readOnly />
                  </td>
                  <td className="border-b border-zinc-100 px-2 py-1.5 dark:border-zinc-800/60">
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
            {total === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-10 text-center text-sm text-zinc-400">
                  {listing.entries.length === 0 ? t("noEntries") : t("noMatch")}
                </td>
              </tr>
            )}
            {padBottom > 0 && (
              <tr aria-hidden style={{ height: padBottom }}>
                <td colSpan={5} style={{ padding: 0, border: 0 }} />
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
