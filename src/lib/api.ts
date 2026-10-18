import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Listing, TaskEvent } from "../types";

export interface ExtractOpts {
  password?: string;
  overwrite?: "overwrite" | "skip" | "rename";
  entries?: string[];
}

export interface CreateOpts {
  format?: "zip" | "7z" | "tar" | "tar.gz";
  level?: number;
  password?: string;
  split?: string;
}

export function listArchive(path: string, password?: string): Promise<Listing> {
  return invoke<Listing>("list_archive", { path, password: password ?? null });
}

export function extractArchive(taskId: number, path: string, dest: string, opts: ExtractOpts = {}): Promise<void> {
  return invoke<void>("extract_archive", {
    taskId,
    path,
    dest,
    password: opts.password ?? null,
    overwrite: opts.overwrite ?? null,
    entries: opts.entries ?? null,
  });
}

export function createArchive(taskId: number, output: string, sources: string[], opts: CreateOpts = {}): Promise<void> {
  return invoke<void>("create_archive", {
    taskId,
    output,
    sources,
    format: opts.format ?? null,
    level: opts.level ?? null,
    password: opts.password ?? null,
    split: opts.split ?? null,
  });
}

export function testArchive(taskId: number, path: string, password?: string): Promise<void> {
  return invoke<void>("test_archive", { taskId, path, password: password ?? null });
}

export function cancelTask(taskId: number): Promise<void> {
  return invoke<void>("cancel_task", { taskId });
}

export function onTask(cb: (e: TaskEvent) => void): Promise<() => void> {
  return listen<TaskEvent>("zf://task", (e) => cb(e.payload));
}

export interface StartupArgs {
  open: string | null;
  add: string[];
  extract: string | null;
  extractSub: string | null;
}

export function startupArgs(): Promise<StartupArgs> {
  return invoke<StartupArgs>("startup_args");
}

export function appVersion(): Promise<string> {
  return invoke<string>("app_version");
}
