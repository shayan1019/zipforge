export function fmtBytes(n: number): string {
  if (!n || n < 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  let x = n;
  while (x >= 1024 && i < units.length - 1) {
    x /= 1024;
    i++;
  }
  return `${x >= 100 || i === 0 ? Math.round(x) : x.toFixed(1)} ${units[i]}`;
}

export function fmtDate(s: string): string {
  if (!s) return "—";
  return s.length >= 16 ? s.slice(0, 16) : s;
}

export function baseName(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts[parts.length - 1] || p;
}

export function dirName(p: string): string {
  const sep = p.includes("\\") ? "\\" : "/";
  const parts = p.replace(/[\\/]+$/, "").split(/[\\/]/);
  parts.pop();
  return parts.join(sep) || sep;
}

/** Joins a directory and a name using the separator that matches the platform path style. */
export function joinPath(dir: string, name: string): string {
  const sep = /^[A-Za-z]:[\\/]/.test(dir) || dir.startsWith("\\\\") ? "\\" : "/";
  return `${dir.replace(/[\\/]+$/, "")}${sep}${name}`;
}

export function defaultOutputName(source: string | undefined, ext: string): string {
  const base = source ? baseName(source).replace(/\.[^.]+$/, "") : "archive";
  return `${base || "archive"}.${ext}`;
}

export function stemName(p: string): string {
  return baseName(p)
    .replace(/\.(tar\.(gz|bz2|xz|zst)|tgz|tbz2|txz|tzst)$/i, "")
    .replace(/\.[^.]+$/, "");
}
