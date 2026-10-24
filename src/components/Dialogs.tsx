import { useEffect, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { Eye, EyeOff, FilePlus, FolderPlus, X } from "lucide-react";
import Modal from "./Modal";
import type { CreateOpts } from "../lib/api";
import { baseName, defaultOutputName, dirName, joinPath } from "../lib/format";
import type { Listing } from "../types";

type T = (k: string, vars?: Record<string, string | number>) => string;

export function ExtractDialog(props: {
  open: boolean;
  t: T;
  listing: Listing | null;
  selected: Set<string>;
  initialDest?: string | null;
  onClose: () => void;
  onStart: (dest: string, overwrite: "overwrite" | "skip" | "rename", entries: string[]) => void;
}) {
  const { open, t, listing, selected, initialDest, onClose, onStart } = props;
  const [dest, setDest] = useState("");
  const [ow, setOw] = useState<"overwrite" | "skip" | "rename">("overwrite");

  useEffect(() => {
    if (open && listing) {
      setDest(initialDest?.trim() || dirName(listing.info.path));
      setOw("overwrite");
    }
  }, [open, listing, initialDest]);

  if (!listing) return null;
  const count = selected.size > 0 ? selected.size : listing.info.entries;

  const browse = async () => {
    const res = await openDialog({ directory: true, defaultPath: dest, title: t("destFolder") });
    if (typeof res === "string") setDest(res);
  };

  return (
    <Modal
      open={open}
      title={t("extractTitle")}
      onClose={onClose}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose}>
            {t("cancel")}
          </button>
          <button className="btn btn-primary" disabled={!dest.trim()} onClick={() => onStart(dest.trim(), ow, [...selected])}>
            {t("start")}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        <div>
          <div className="mb-1.5 text-xs font-medium text-zinc-500">{t("destFolder")}</div>
          <div className="flex gap-2">
            <input className="input" value={dest} onChange={(e) => setDest(e.target.value)} spellCheck={false} />
            <button className="btn btn-soft shrink-0" onClick={browse}>
              {t("browse")}
            </button>
          </div>
        </div>
        <div>
          <div className="mb-1.5 text-xs font-medium text-zinc-500">{t("existingFiles")}</div>
          <div className="seg">
            {(
              [
                ["overwrite", t("owOverwrite")],
                ["skip", t("owSkip")],
                ["rename", t("owRename")],
              ] as const
            ).map(([key, label]) => (
              <button key={key} className={`seg-item ${ow === key ? "seg-item-active" : ""}`} onClick={() => setOw(key)}>
                {label}
              </button>
            ))}
          </div>
        </div>
        <div className="text-xs text-zinc-400">
          {count} {t("itemCount")}
        </div>
      </div>
    </Modal>
  );
}

export function CreateDialog(props: {
  open: boolean;
  t: T;
  initialSources?: string[];
  /** Called once the incoming sources are merged, so the parent can drop them.
   *  Without this the same paths are merged again on every reopen and old
   *  selections keep piling up in a fresh dialog. */
  onSourcesConsumed?: () => void;
  onClose: () => void;
  onStart: (output: string, sources: string[], opts: CreateOpts) => void;
}) {
  const { open, t, initialSources, onSourcesConsumed, onClose, onStart } = props;
  const [sources, setSources] = useState<string[]>([]);
  const [format, setFormat] = useState<"zip" | "7z" | "tar" | "tar.gz">("zip");
  const [level, setLevel] = useState(5);
  const [password, setPassword] = useState("");
  const [showPw, setShowPw] = useState(false);
  const [split, setSplit] = useState("");
  const [output, setOutput] = useState("");

  useEffect(() => {
    if (!open) return;
    setSources([]);
    setOutput("");
    setPassword("");
    setShowPw(false);
    setSplit("");
    setFormat("zip");
    setLevel(5);
  }, [open]);

  // Sources can arrive in several waves: Explorer's multi-selection launches one
  // process per item and the Rust side forwards each argv to this window.
  useEffect(() => {
    if (!open) return;
    const init = initialSources ?? [];
    if (!init.length) return;
    setSources((prev) => [...new Set([...prev, ...init])]);
    setOutput((prev) => prev || joinPath(dirName(init[0]), defaultOutputName(init[0], "zip")));
    onSourcesConsumed?.();
  }, [open, initialSources, onSourcesConsumed]);

  const ext = format;

  const pickFiles = async () => {
    const res = await openDialog({ multiple: true, title: t("addFiles") });
    const list = typeof res === "string" ? [res] : Array.isArray(res) ? res : [];
    if (list.length) setSources((s) => [...new Set([...s, ...list])]);
  };

  const pickFolder = async () => {
    const res = await openDialog({ directory: true, title: t("addFolder") });
    if (typeof res === "string") setSources((s) => [...new Set([...s, res])]);
  };

  const pickOutput = async () => {
    const res = await saveDialog({
      defaultPath: output || defaultOutputName(sources[0], ext),
      filters: [{ name: format.toUpperCase(), extensions: [ext] }],
      title: t("output"),
    });
    if (typeof res === "string") setOutput(res);
  };

  const setFmt = (f: "zip" | "7z" | "tar" | "tar.gz") => {
    setFormat(f);
    const ne = f;
    setOutput((o) => (o ? o.replace(/\.(zip|7z|tar\.gz|tar)$/i, `.${ne}`) : o));
  };

  const canStart = sources.length > 0 && output.trim().length > 0;

  const start = () => {
    let out = output.trim();
    if (!out) return;
    if (!new RegExp(`\\.(zip|7z|tar\\.gz|tar)$`, "i").test(out)) out = `${out}.${ext}`;
    onStart(out, sources, {
      format,
      level,
      password: format === "tar.gz" ? undefined : password || undefined,
      split: format !== "tar" && format !== "tar.gz" && split ? split : undefined,
    });
  };

  return (
    <Modal
      open={open}
      title={t("createTitle")}
      onClose={onClose}
      width={600}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose}>
            {t("cancel")}
          </button>
          <button className="btn btn-primary" disabled={!canStart} onClick={start}>
            {t("create")}
          </button>
        </>
      }
    >
      <div className="space-y-4">
        <div>
          <div className="mb-1.5 flex items-center justify-between">
            <span className="text-xs font-medium text-zinc-500">{t("sources")}</span>
            <div className="flex gap-1.5">
              <button className="btn btn-soft h-7 px-2 text-xs" onClick={pickFiles}>
                <FilePlus size={13} />
                {t("addFiles")}
              </button>
              <button className="btn btn-soft h-7 px-2 text-xs" onClick={pickFolder}>
                <FolderPlus size={13} />
                {t("addFolder")}
              </button>
            </div>
          </div>
          <div className="max-h-32 overflow-auto rounded-lg border border-zinc-200 dark:border-zinc-800">
            {sources.length === 0 ? (
              <div className="px-3 py-3 text-xs text-zinc-400">{t("noSources")}</div>
            ) : (
              sources.map((s) => (
                <div key={s} className="flex items-center gap-2 px-3 py-1.5 text-sm [&:not(:last-child)]:border-b [&:not(:last-child)]:border-zinc-100 dark:[&:not(:last-child)]:border-zinc-800/60">
                  <span className="min-w-0 flex-1 truncate" title={s}>
                    {baseName(s)}
                  </span>
                  <button className="btn-icon h-6 w-6" onClick={() => setSources((cur) => cur.filter((x) => x !== s))}>
                    <X size={12} />
                  </button>
                </div>
              ))
            )}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <div className="mb-1.5 text-xs font-medium text-zinc-500">{t("format")}</div>
            <div className="seg">
              {(["zip", "7z", "tar", "tar.gz"] as const).map((f) => (
                <button key={f} className={`seg-item uppercase ${format === f ? "seg-item-active" : ""}`} onClick={() => setFmt(f)}>
                  {f}
                </button>
              ))}
            </div>
          </div>
          <div>
            <div className="mb-1.5 text-xs font-medium text-zinc-500">{t("level")}</div>
            <div className="seg">
              {(
                [
                  [1, t("lvlFast")],
                  [5, t("lvlNormal")],
                  [9, t("lvlMax")],
                ] as const
              ).map(([lv, label]) => (
                <button key={lv} className={`seg-item ${level === lv ? "seg-item-active" : ""}`} onClick={() => setLevel(lv)}>
                  {label}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div>
            <div className="mb-1.5 text-xs font-medium text-zinc-500">{t("passwordOptional")}</div>
            <div className="relative">
              <input
                className="input pr-9"
                type={showPw ? "text" : "password"}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                spellCheck={false}
              />
              <button
                className="absolute right-1 top-1/2 -translate-y-1/2 btn-icon h-7 w-7"
                onClick={() => setShowPw((v) => !v)}
                tabIndex={-1}
              >
                {showPw ? <EyeOff size={14} /> : <Eye size={14} />}
              </button>
            </div>
          </div>
          <div>
            <div className="mb-1.5 text-xs font-medium text-zinc-500">{t("split")}</div>
            <select className="input" value={split} onChange={(e) => setSplit(e.target.value)} disabled={format === "tar" || format === "tar.gz"}>
              <option value="">{t("splitNone")}</option>
              <option value="100m">100 MB</option>
              <option value="650m">650 MB</option>
              <option value="1g">1 GB</option>
            </select>
          </div>
        </div>

        <div>
          <div className="mb-1.5 text-xs font-medium text-zinc-500">{t("output")}</div>
          <div className="flex gap-2">
            <input className="input" value={output} onChange={(e) => setOutput(e.target.value)} spellCheck={false} />
            <button className="btn btn-soft shrink-0" onClick={pickOutput}>
              {t("browse")}
            </button>
          </div>
        </div>
      </div>
    </Modal>
  );
}

export function PasswordDialog(props: {
  open: boolean;
  t: T;
  onClose: () => void;
  onSubmit: (pw: string) => void;
}) {
  const { open, t, onClose, onSubmit } = props;
  const [pw, setPw] = useState("");

  useEffect(() => {
    if (open) setPw("");
  }, [open]);

  return (
    <Modal
      open={open}
      title={t("passwordNeeded")}
      onClose={onClose}
      width={420}
      footer={
        <>
          <button className="btn btn-ghost" onClick={onClose}>
            {t("cancel")}
          </button>
          <button className="btn btn-primary" disabled={!pw} onClick={() => onSubmit(pw)}>
            {t("start")}
          </button>
        </>
      }
    >
      <div className="space-y-3">
        <p className="text-xs text-zinc-400">{t("passwordHint")}</p>
        <input
          className="input"
          type="password"
          autoFocus
          value={pw}
          onChange={(e) => setPw(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && pw) onSubmit(pw);
          }}
        />
      </div>
    </Modal>
  );
}
