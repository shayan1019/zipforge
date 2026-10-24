import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle,
  Boxes,
  CheckCircle2,
  FolderOpen,
  Loader2,
  Moon,
  Plus,
  Sun,
  X,
} from "lucide-react";
import ArchiveView from "./components/ArchiveView";
import { CreateDialog, ExtractDialog, PasswordDialog } from "./components/Dialogs";
import EmptyState from "./components/EmptyState";
import {
  appVersion,
  cancelTask,
  createArchive,
  extractArchive,
  listArchive,
  onStartupArgs,
  onTask,
  startupArgs,
  takePendingArgs,
  testArchive,
  type CreateOpts,
  type StartupArgs,
} from "./lib/api";
import { dirName, joinPath, stemName, baseName } from "./lib/format";
import { useI18n } from "./lib/i18n";
import type { Listing, TaskEvent } from "./types";

const ARCH_EXTS = [
  "zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "xz", "zst", "iso", "cab",
  "jar", "lzma", "lz", "apk", "war", "ear", "001",
];

interface TaskState {
  id: number;
  kind: "extract" | "create" | "test";
  label: string;
  percent: number;
  file?: string;
  dest?: string;
}

interface Toast {
  ok: boolean;
  msg: string;
  action?: { label: string; run: () => void };
}

export default function App() {
  const { lang, setLang, t } = useI18n();
  const [dark, setDark] = useState<boolean>(() => {
    const saved = localStorage.getItem("zf.theme");
    return saved ? saved === "dark" : window.matchMedia("(prefers-color-scheme: dark)").matches;
  });
  const [listing, setListing] = useState<Listing | null>(null);
  const [password, setPassword] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [task, setTask] = useState<TaskState | null>(null);
  const [toast, setToast] = useState<Toast | null>(null);
  const [extractOpen, setExtractOpen] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [pwFor, setPwFor] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [version, setVersion] = useState("");
  const [initSources, setInitSources] = useState<string[] | null>(null);
  const [extractDest, setExtractDest] = useState<string | null>(null);
  const [opening, setOpening] = useState<string | null>(null);
  const [openMs, setOpenMs] = useState(0);

  const taskRef = useRef<TaskState | null>(null);
  const nextId = useRef(Math.floor(Math.random() * 900000) + 100);
  const pendingExtract = useRef<string | null>(null);
  const openSeq = useRef(0);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem("zf.theme", dark ? "dark" : "light");
  }, [dark]);

  useEffect(() => {
    taskRef.current = task;
  }, [task]);

  useEffect(() => {
    if (!toast) return;
    const h = setTimeout(() => setToast(null), 7000);
    return () => clearTimeout(h);
  }, [toast]);

  useEffect(() => {
    const un = onTask((ev: TaskEvent) => {
      const cur = taskRef.current;
      if (!cur || cur.id !== ev.taskId) return;
      if (ev.phase === "progress") {
        setTask({ ...cur, percent: ev.percent ?? cur.percent, file: ev.file ?? cur.file });
        return;
      }
      setTask(null);
      if (ev.phase === "done") {
        const action = cur.dest
          ? { label: t("revealFolder"), run: () => void revealItemInDir(cur.dest as string) }
          : undefined;
        if (cur.kind === "extract") setToast({ ok: true, msg: t("extractDone"), action });
        else if (cur.kind === "create") setToast({ ok: true, msg: t("createDone"), action });
        else setToast({ ok: true, msg: t("testDone") });
      } else {
        setToast({ ok: false, msg: ev.message || t("error") });
      }
    });
    return () => {
      void un.then((f) => f());
    };
  }, [t]);

  const openPath = useCallback(async (p: string, pw?: string) => {
    const seq = ++openSeq.current;
    setOpening(p);
    setOpenMs(0);
    try {
      const l = await listArchive(p, pw);
      if (seq !== openSeq.current) return;
      setListing(l);
      setPassword(pw ?? null);
      setSelected(new Set());
      setPwFor(null);
      const pend = pendingExtract.current;
      if (pend) {
        pendingExtract.current = null;
        setExtractDest(pend);
        setExtractOpen(true);
      }
    } catch (e) {
      if (seq !== openSeq.current) return;
      const s = String(e);
      if (!pw && s.includes("ENCRYPTED")) {
        setPwFor(p);
      } else {
        pendingExtract.current = null;
        setToast({ ok: false, msg: s });
      }
    } finally {
      if (seq === openSeq.current) setOpening(null);
    }
  }, []);

  // Elapsed time next to the spinner: a 60k-entry archive needs a few seconds
  // before the first row exists, and silence reads as a freeze.
  useEffect(() => {
    if (!opening) return;
    const t0 = performance.now();
    const h = setInterval(() => setOpenMs(performance.now() - t0), 250);
    return () => clearInterval(h);
  }, [opening]);

  // Explorer context-menu / command line entry points. A multi-selection uses
  // the shell's "Player" model, so every selected item arrives as its own
  // process; the Rust side funnels them into this instance and we merge them
  // into a single Create dialog.
  const handleStartup = useCallback(
    (a: StartupArgs) => {
      if (a.add.length) {
        setInitSources((prev) => [...new Set([...(prev ?? []), ...a.add])]);
        setCreateOpen(true);
      }
      const target = a.extract ?? a.extractSub;
      if (target) {
        pendingExtract.current = a.extractSub ? joinPath(dirName(target), stemName(target)) : dirName(target);
        void openPath(target);
      } else if (a.open) {
        void openPath(a.open);
      }
    },
    [openPath],
  );

  useEffect(() => {
    let alive = true;
    const un = onStartupArgs((a) => {
      if (alive) handleStartup(a);
    });
    void (async () => {
      try {
        const v = await appVersion();
        if (alive) setVersion(v);
      } catch {
        /* ignore */
      }
      // Drain the forwarded queue after the listener is live so launches that
      // happened while the webview was still booting are not lost.
      try {
        const own = await startupArgs();
        if (alive) handleStartup(own);
      } catch {
        /* ignore */
      }
      try {
        const pending = await takePendingArgs();
        if (alive) pending.forEach(handleStartup);
      } catch {
        /* ignore */
      }
    })();
    return () => {
      alive = false;
      void un.then((f) => f());
    };
  }, [handleStartup]);

  const pickOpen = useCallback(async () => {
    try {
      const res = await openDialog({
        multiple: false,
        title: t("open"),
        filters: [
          { name: t("archivesFilter"), extensions: ARCH_EXTS },
          { name: t("allFiles"), extensions: ["*"] },
        ],
      });
      if (typeof res === "string") void openPath(res);
    } catch (e) {
      setToast({ ok: false, msg: String(e) });
    }
  }, [t, openPath]);

  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === "enter" || p.type === "over") setDragOver(true);
      else if (p.type === "drop") {
        setDragOver(false);
        const path = p.paths?.[0];
        if (path) void openPath(path);
      } else {
        setDragOver(false);
      }
    });
    return () => {
      void un.then((f) => f());
    };
  }, [openPath]);

  const toggle = useCallback((name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });
  }, []);

  const startExtract = useCallback(
    async (dest: string, overwrite: "overwrite" | "skip" | "rename", entries: string[]) => {
      if (!listing) return;
      setExtractOpen(false);
      setExtractDest(null);
      const id = nextId.current++;
      setTask({ id, kind: "extract", label: t("extracting"), percent: 0, dest });
      try {
        await extractArchive(id, listing.info.path, dest, {
          password: password ?? undefined,
          overwrite,
          entries: entries.length ? entries : undefined,
        });
      } catch (e) {
        setTask(null);
        setToast({ ok: false, msg: String(e) });
      }
    },
    [listing, password, t],
  );

  const startCreate = useCallback(
    async (output: string, sources: string[], opts: CreateOpts) => {
      setCreateOpen(false);
      setInitSources(null);
      const id = nextId.current++;
      setTask({ id, kind: "create", label: t("creating"), percent: 0, dest: output });
      try {
        await createArchive(id, output, sources, opts);
      } catch (e) {
        setTask(null);
        setToast({ ok: false, msg: String(e) });
      }
    },
    [t],
  );

  const runTest = useCallback(async () => {
    if (!listing) return;
    const id = nextId.current++;
    setTask({ id, kind: "test", label: t("testing"), percent: 0 });
    try {
      await testArchive(id, listing.info.path, password ?? undefined);
    } catch (e) {
      setTask(null);
      setToast({ ok: false, msg: String(e) });
    }
  }, [listing, password, t]);

  return (
    <div className="flex h-screen flex-col bg-zinc-50 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
      <header className="flex h-14 shrink-0 items-center gap-3 border-b border-zinc-200 px-5 dark:border-zinc-800">
        <div className="flex items-center gap-2.5">
          <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-gradient-to-br from-indigo-500 via-violet-500 to-fuchsia-500 shadow-sm">
            <Boxes className="text-white" size={17} strokeWidth={2} />
          </div>
          <div className="leading-tight">
            <div className="text-sm font-semibold tracking-tight">zipforge</div>
            <div className="text-[10px] text-zinc-400">{t("appTagline")}</div>
          </div>
        </div>
        <div className="flex-1" />
        <button className="btn btn-soft" onClick={pickOpen} disabled={!!task}>
          <FolderOpen size={15} />
          {t("open")}
        </button>
        <button className="btn btn-primary" onClick={() => setCreateOpen(true)} disabled={!!task}>
          <Plus size={15} />
          {t("create")}
        </button>
        <div className="mx-1 h-6 w-px bg-zinc-200 dark:bg-zinc-800" />
        <button className="btn-icon" title={t("language")} onClick={() => setLang(lang === "tr" ? "en" : "tr")}>
          <span className="text-[11px] font-bold">{lang === "tr" ? "TR" : "EN"}</span>
        </button>
        <button className="btn-icon" title={t("theme")} onClick={() => setDark((d) => !d)}>
          {dark ? <Sun size={15} /> : <Moon size={15} />}
        </button>
      </header>

      <main className="relative min-h-0 flex-1 px-5 pb-2 pt-4">
        {listing ? (
          <ArchiveView
            t={t}
            listing={listing}
            selected={selected}
            onToggle={toggle}
            onSetSelection={(names) => setSelected(new Set(names))}
            onExtract={() => setExtractOpen(true)}
            onTest={runTest}
            busy={!!task}
          />
        ) : (
          <EmptyState t={t} onOpen={pickOpen} onCreate={() => setCreateOpen(true)} />
        )}
        {opening && (
          <div className="absolute inset-0 z-30 flex items-center justify-center bg-zinc-50/80 backdrop-blur-sm dark:bg-zinc-950/80">
            <div className="flex items-center gap-3 rounded-xl border border-zinc-200 bg-white px-4 py-3 shadow-lg dark:border-zinc-700 dark:bg-zinc-900">
              <Loader2 size={18} className="animate-spin text-indigo-500" />
              <div className="min-w-0">
                <div className="text-sm font-medium">{t("opening")}</div>
                <div className="max-w-xs truncate text-xs text-zinc-400">
                  {baseName(opening)} · {(openMs / 1000).toFixed(1)}s
                </div>
              </div>
            </div>
          </div>
        )}
      </main>

      <footer className="flex h-10 shrink-0 items-center gap-3 border-t border-zinc-200 px-4 text-xs dark:border-zinc-800">
        {task ? (
          <>
            <span className="max-w-[38%] truncate font-medium text-zinc-600 dark:text-zinc-300">
              {task.label}
              {task.file ? <span className="text-zinc-400"> — {task.file}</span> : null}
            </span>
            <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-800">
              <div
                className="h-full rounded-full bg-indigo-500 transition-[width] duration-150"
                style={{ width: `${task.percent}%` }}
              />
            </div>
            <span className="w-10 text-right tabular-nums text-zinc-500">{task.percent}%</span>
            <button className="btn btn-danger-soft h-7 px-2" onClick={() => void cancelTask(task.id)}>
              {t("cancel")}
            </button>
          </>
        ) : (
          <span className="text-zinc-400">
            {t("ready")}
            {version ? ` · v${version}` : ""}
          </span>
        )}
      </footer>

      <ExtractDialog
        open={extractOpen}
        t={t}
        listing={listing}
        selected={selected}
        initialDest={extractDest}
        onClose={() => {
          setExtractOpen(false);
          setExtractDest(null);
        }}
        onStart={startExtract}
      />
      <CreateDialog
        open={createOpen}
        t={t}
        initialSources={initSources ?? undefined}
        onClose={() => {
          setCreateOpen(false);
          setInitSources(null);
        }}
        onStart={startCreate}
      />
      <PasswordDialog
        open={!!pwFor}
        t={t}
        onClose={() => {
          setPwFor(null);
          pendingExtract.current = null;
        }}
        onSubmit={(pw) => void openPath(pwFor as string, pw)}
      />

      {dragOver && (
        <div className="pointer-events-none fixed inset-3 z-40 flex items-center justify-center rounded-2xl border-2 border-dashed border-indigo-400 bg-indigo-500/10 backdrop-blur-sm">
          <div className="rounded-xl bg-white/90 px-4 py-2 text-sm font-medium text-indigo-600 shadow dark:bg-zinc-900/90 dark:text-indigo-300">
            {t("dropTitle")}
          </div>
        </div>
      )}

      {toast && (
        <div className="fixed bottom-14 right-4 z-[60] flex max-w-md items-start gap-2.5 rounded-xl border border-zinc-200 bg-white p-3 shadow-xl dark:border-zinc-700 dark:bg-zinc-900">
          {toast.ok ? (
            <CheckCircle2 size={17} className="mt-px shrink-0 text-emerald-500" />
          ) : (
            <AlertTriangle size={17} className="mt-px shrink-0 text-red-500" />
          )}
          <div className="min-w-0 whitespace-pre-wrap break-words text-sm">{toast.msg}</div>
          {toast.action && (
            <button
              className="btn btn-soft h-7 shrink-0 px-2 text-xs"
              onClick={() => {
                toast.action?.run();
                setToast(null);
              }}
            >
              {toast.action.label}
            </button>
          )}
          <button className="btn-icon h-6 w-6 shrink-0" onClick={() => setToast(null)}>
            <X size={13} />
          </button>
        </div>
      )}
    </div>
  );
}
