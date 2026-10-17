export interface ArchiveInfo {
  path: string;
  name: string;
  format: string;
  physicalSize: number;
  entries: number;
  encrypted: boolean;
}

export interface ArchiveEntry {
  name: string;
  size: number;
  packedSize: number;
  modified: string;
  crc: string;
  attributes: string;
  encrypted: boolean;
  isDir: boolean;
  method: string;
}

export interface Listing {
  info: ArchiveInfo;
  entries: ArchiveEntry[];
}

export interface TaskEvent {
  taskId: number;
  phase: "progress" | "done" | "error";
  percent?: number;
  message?: string;
  file?: string;
}
