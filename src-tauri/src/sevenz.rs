// sevenz.rs — 7-Zip engine wrapper: listing, extract, create, test + progress events
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct TaskHandle {
    pub child: Child,
    pub cancelled: AtomicBool,
}

#[derive(Clone, Default)]
pub struct Tasks(pub Arc<Mutex<HashMap<u32, TaskHandle>>>);

impl Tasks {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveInfo {
    pub path: String,
    pub name: String,
    pub format: String,
    pub physical_size: u64,
    pub entries: u64,
    pub encrypted: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub packed_size: u64,
    pub modified: String,
    pub crc: String,
    pub attributes: String,
    pub encrypted: bool,
    pub is_dir: bool,
    pub method: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub info: ArchiveInfo,
    pub entries: Vec<Entry>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskEvent {
    pub task_id: u32,
    pub phase: String, // progress | done | error
    pub percent: Option<u32>,
    pub message: Option<String>,
    pub file: Option<String>,
}

pub fn seven_zip(app: &AppHandle) -> Result<PathBuf, String> {
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("7z")
        .join("7z.exe");
    if dev.exists() {
        return Ok(dev);
    }
    if let Ok(res) = app
        .path()
        .resolve("resources/7z/7z.exe", tauri::path::BaseDirectory::Resource)
    {
        if res.exists() {
            return Ok(res);
        }
    }
    Err("7z.exe bulunamadi".into())
}

fn base_cmd(app: &AppHandle) -> Result<Command, String> {
    let exe = seven_zip(app)?;
    let mut c = Command::new(exe);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);
    c.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(c)
}

fn parse_size(s: &str) -> u64 {
    s.trim().parse().unwrap_or(0)
}

fn parse_percent(l: &str) -> Option<u32> {
    let idx = l.find('%')?;
    let mut digits = String::new();
    for ch in l[..idx].trim_end().chars().rev() {
        if ch.is_ascii_digit() {
            digits.insert(0, ch);
        } else {
            break;
        }
    }
    digits.parse::<u32>().ok().filter(|p| *p <= 100)
}

fn parse_file(l: &str) -> Option<String> {
    let t = l.trim();
    if let Some(rest) = t.strip_prefix("Extracting") {
        let n = rest.trim();
        if !n.is_empty() && !n.starts_with('%') {
            return Some(n.to_string());
        }
    }
    let idx = l.find('%')?;
    let after = l[idx + 1..].trim();
    if after.is_empty() {
        return None;
    }
    let mut parts = after.splitn(3, ' ');
    let _count = parts.next();
    let sign = parts.next().unwrap_or("");
    if !matches!(sign, "-" | "+" | "U") {
        return None;
    }
    let name = parts.next().unwrap_or("").trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn emit(app: &AppHandle, ev: TaskEvent) {
    let _ = app.emit("zf://task", ev);
}

fn format_task_error(error_lines: &[String], code: Option<i32>) -> String {
    if !error_lines.is_empty() {
        let tail: Vec<&str> = error_lines
            .iter()
            .rev()
            .take(3)
            .map(|s| s.as_str())
            .collect();
        let joined: Vec<&str> = tail.into_iter().rev().collect();
        return joined.join("\n");
    }
    match code {
        Some(c) if c != 0 => format!("7z hata kodu: {c}"),
        _ => "İşlem başarısız oldu".to_string(),
    }
}

// ---------- list ----------

pub fn list_archive(app: &AppHandle, path: &str, password: Option<&str>) -> Result<Listing, String> {
    let mut c = base_cmd(app)?;
    c.arg("l")
        .arg("-slt")
        .arg("-sccUTF-8");
    if let Some(p) = password {
        if !p.is_empty() {
            c.arg(format!("-p{p}"));
        }
    }
    c.arg("--").arg(path);
    let out = c.output().map_err(|e| format!("7z çalıştırılamadı: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let blob = format!("{stdout}\n{stderr}");
    if !out.status.success() {
        let low = blob.to_lowercase();
        if low.contains("wrong password") || low.contains("cannot open encrypted archive") {
            return Err("ENCRYPTED".into());
        }
        let msg = blob
            .lines()
            .rev()
            .find(|l| {
                let t = l.trim();
                !t.is_empty() && !t.starts_with("7-Zip") && !t.starts_with("Scanning")
            })
            .unwrap_or("Arşiv açılamadı")
            .trim()
            .to_string();
        return Err(format!("Arşiv açılamadı: {msg}"));
    }
    parse_slt(path, &stdout)
}

fn parse_slt(path: &str, out: &str) -> Result<Listing, String> {
    let mut groups: Vec<HashMap<String, String>> = Vec::new();
    let mut cur: HashMap<String, String> = HashMap::new();
    let flush = |cur: &mut HashMap<String, String>, groups: &mut Vec<HashMap<String, String>>| {
        if !cur.is_empty() {
            groups.push(std::mem::take(cur));
        }
    };
    for line in out.lines() {
        if line.trim().is_empty() || (line.starts_with("---") && line.trim_end().chars().all(|ch| ch == '-')) {
            flush(&mut cur, &mut groups);
            continue;
        }
        if let Some((k, v)) = line.split_once(" = ") {
            cur.insert(k.trim().to_string(), v.to_string());
        }
    }
    flush(&mut cur, &mut groups);
    if groups.is_empty() {
        return Err("Arşiv içeriği okunamadı".into());
    }
    let meta_idx = groups
        .iter()
        .position(|g| g.contains_key("Type"))
        .unwrap_or(0);
    let meta = &groups[meta_idx];
    let name = PathBuf::from(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let entries: Vec<Entry> = groups
        .iter()
        .skip(meta_idx + 1)
        .filter_map(|g| {
            let name = g.get("Path")?.clone();
            let attrs = g.get("Attributes").cloned().unwrap_or_default();
            let is_dir = attrs.starts_with('D')
                || g.get("Folder").map(|v| v == "+").unwrap_or(false);
            let encrypted = g
                .get("Encrypted")
                .map(|v| v == "+" || v == "1")
                .unwrap_or(false);
            Some(Entry {
                name,
                size: parse_size(g.get("Size").map(String::as_str).unwrap_or("0")),
                packed_size: parse_size(g.get("Packed Size").map(String::as_str).unwrap_or("0")),
                modified: g.get("Modified").cloned().unwrap_or_default(),
                crc: g.get("CRC").cloned().unwrap_or_default(),
                attributes: attrs,
                encrypted,
                is_dir,
                method: g.get("Method").cloned().unwrap_or_default(),
            })
        })
        .collect();
    let info = ArchiveInfo {
        path: path.to_string(),
        name,
        format: meta.get("Type").cloned().unwrap_or_default(),
        physical_size: parse_size(meta.get("Physical Size").map(String::as_str).unwrap_or("0")),
        entries: entries.len() as u64,
        encrypted: entries.iter().any(|e| e.encrypted)
            || meta
                .get("Encrypted")
                .map(|v| v == "+")
                .unwrap_or(false),
    };
    Ok(Listing { info, entries })
}

// ---------- task runner ----------

pub fn spawn_task(
    app: &AppHandle,
    tasks: Arc<Mutex<HashMap<u32, TaskHandle>>>,
    task_id: u32,
    mut cmd: Command,
) -> Result<(), String> {
    let mut child = cmd.spawn().map_err(|e| format!("7z başlatılamadı: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    tasks.lock().unwrap().insert(
        task_id,
        TaskHandle {
            child,
            cancelled: AtomicBool::new(false),
        },
    );
    let app2 = app.clone();
    std::thread::spawn(move || {
        let errs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let errs2 = errs.clone();
        let stderr_thread = stderr.map(|e| {
            std::thread::spawn(move || {
                for line in BufReader::new(e).lines().map_while(Result::ok) {
                    if !line.trim().is_empty() {
                        errs2.lock().unwrap().push(line);
                    }
                }
            })
        });
        let mut percent: u32 = 0;
        let mut last_file: Option<String> = None;
        let mut error_lines: Vec<String> = Vec::new();
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        if let Some(out) = stdout {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                let l: String = line
                    .chars()
                    .filter(|c| *c != '\r' && *c != '\u{8}')
                    .collect();
                if l.contains("ERROR") || l.contains("Can not open") || l.contains("Cannot open") {
                    error_lines.push(l.clone());
                }
                if let Some(p) = parse_percent(&l) {
                    percent = p;
                }
                if let Some(f) = parse_file(&l) {
                    last_file = Some(f);
                }
                if last_emit.elapsed() >= Duration::from_millis(120) || percent >= 100 {
                    last_emit = Instant::now();
                    emit(
                        &app2,
                        TaskEvent {
                            task_id,
                            phase: "progress".into(),
                            percent: Some(percent),
                            message: None,
                            file: last_file.clone(),
                        },
                    );
                }
            }
        }
        if let Some(t) = stderr_thread {
            let _ = t.join();
        }
        let handle = tasks.lock().unwrap().remove(&task_id);
        let mut ok = false;
        let mut code = None;
        let mut cancelled = false;
        if let Some(mut h) = handle {
            cancelled = h.cancelled.load(Ordering::SeqCst);
            if let Ok(st) = h.child.wait() {
                ok = st.success();
                code = st.code();
            }
        }
        for l in errs.lock().unwrap().iter() {
            error_lines.push(l.clone());
        }
        if ok && !cancelled {
            emit(
                &app2,
                TaskEvent {
                    task_id,
                    phase: "done".into(),
                    percent: Some(100),
                    message: None,
                    file: last_file,
                },
            );
        } else {
            let msg = if cancelled {
                "İptal edildi".to_string()
            } else {
                format_task_error(&error_lines, code)
            };
            emit(
                &app2,
                TaskEvent {
                    task_id,
                    phase: "error".into(),
                    percent: Some(percent),
                    message: Some(msg),
                    file: last_file,
                },
            );
        }
    });
    Ok(())
}

// ---------- operations ----------

pub fn extract_archive(
    app: &AppHandle,
    tasks: Arc<Mutex<HashMap<u32, TaskHandle>>>,
    task_id: u32,
    path: &str,
    dest: &str,
    password: Option<&str>,
    overwrite: Option<&str>,
    entries: Option<Vec<String>>,
) -> Result<(), String> {
    let mut c = base_cmd(app)?;
    c.arg("x").arg("-y").arg("-sccUTF-8").arg("-bsp1");
    let ow = match overwrite {
        Some("skip") => "-aos",
        Some("rename") => "-aou",
        _ => "-aoa",
    };
    c.arg(ow);
    if let Some(p) = password {
        if !p.is_empty() {
            c.arg(format!("-p{p}"));
        }
    }
    c.arg(format!("-o{dest}"));
    c.arg("--").arg(path);
    if let Some(list) = entries {
        for e in list {
            c.arg(e);
        }
    }
    spawn_task(app, tasks, task_id, c)
}

pub fn test_archive(
    app: &AppHandle,
    tasks: Arc<Mutex<HashMap<u32, TaskHandle>>>,
    task_id: u32,
    path: &str,
    password: Option<&str>,
) -> Result<(), String> {
    let mut c = base_cmd(app)?;
    c.arg("t").arg("-y").arg("-sccUTF-8").arg("-bsp1");
    if let Some(p) = password {
        if !p.is_empty() {
            c.arg(format!("-p{p}"));
        }
    }
    c.arg("--").arg(path);
    spawn_task(app, tasks, task_id, c)
}

fn guess_format(output: &str) -> &'static str {
    let low = output.to_lowercase();
    if low.ends_with(".7z") {
        "7z"
    } else if low.ends_with(".tar") {
        "tar"
    } else {
        "zip"
    }
}

fn relativize(sources: &[String]) -> (Option<PathBuf>, Vec<String>) {
    let paths: Vec<PathBuf> = sources.iter().map(PathBuf::from).collect();
    let mut base: Option<PathBuf> = None;
    for p in &paths {
        let parent = match p.parent() {
            Some(par) => par.to_path_buf(),
            None => return (None, sources.to_vec()),
        };
        base = Some(match base {
            None => parent,
            Some(b) => {
                let mut cur = b;
                while !parent.starts_with(&cur) {
                    match cur.parent() {
                        Some(x) => cur = x.to_path_buf(),
                        None => return (None, sources.to_vec()),
                    }
                }
                cur
            }
        });
    }
    let base = match base {
        Some(b) if !b.as_os_str().is_empty() => b,
        _ => return (None, sources.to_vec()),
    };
    let args = paths
        .iter()
        .map(|p| {
            p.strip_prefix(&base)
                .map(|r| r.to_string_lossy().to_string())
                .unwrap_or_else(|_| p.to_string_lossy().to_string())
        })
        .collect();
    (Some(base), args)
}

pub fn create_archive(
    app: &AppHandle,
    tasks: Arc<Mutex<HashMap<u32, TaskHandle>>>,
    task_id: u32,
    output: &str,
    sources: Vec<String>,
    format: Option<&str>,
    level: Option<u32>,
    password: Option<&str>,
    split: Option<&str>,
) -> Result<(), String> {
    if sources.is_empty() {
        return Err("Kaynak seçilmedi".into());
    }
    let mut c = base_cmd(app)?;
    c.arg("a").arg("-y").arg("-sccUTF-8").arg("-bsp1");
    let fmt = format.filter(|f| !f.is_empty()).unwrap_or_else(|| guess_format(output));
    c.arg(format!("-t{fmt}"));
    if let Some(l) = level {
        if l > 0 {
            c.arg(format!("-mx{l}"));
        }
    }
    if let Some(p) = password {
        if !p.is_empty() {
            c.arg(format!("-p{p}"));
            if fmt == "7z" {
                c.arg("-mhe=on");
            }
            if fmt == "zip" {
                c.arg("-mem=AES256");
            }
        }
    }
    if let Some(s) = split {
        if !s.is_empty() {
            c.arg(format!("-v{s}"));
        }
    }
    c.arg("--").arg(output);
    let (cwd, args) = relativize(&sources);
    if let Some(dir) = cwd {
        c.current_dir(dir);
    }
    for a in args {
        c.arg(a);
    }
    spawn_task(app, tasks, task_id, c)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
7-Zip 26.04 (x64) : Copyright (c) 1999-2026 Igor Pavlov : 2026-10-05

Scanning the drive for archives:
1 file, 308 bytes (1 KiB)

Listing archive: C:\tmp\sample.zip

--
Path = C:\tmp\sample.zip
Type = zip
Physical Size = 308

----------
Path = hello.txt
Folder = -
Size = 16
Packed Size = 16
Modified = 2026-10-18 01:42:05.7451164
Attributes = A
Encrypted = -
CRC = ABD03619
Method = Store

Path = sub
Folder = +
Size = 0
Packed Size = 0
Modified = 2026-10-18 01:42:05.0000000
Attributes = D
Encrypted = -
CRC = 
Method = Store

Path = sub\note.md
Folder = -
Size = 14
Packed Size = 14
Modified = 2026-10-18 01:42:05.7481667
Attributes = A
Encrypted = -
CRC = AC45FB1A
Method = Store
"#;

    #[test]
    fn parses_slt_listing() {
        let l = parse_slt("C:\\tmp\\sample.zip", SAMPLE).unwrap();
        assert_eq!(l.info.format, "zip");
        assert_eq!(l.info.physical_size, 308);
        assert_eq!(l.entries.len(), 3);
        assert!(!l.entries[0].is_dir);
        assert_eq!(l.entries[0].size, 16);
        assert_eq!(l.entries[0].name, "hello.txt");
        assert!(l.entries[1].is_dir);
        assert!(!l.entries[2].is_dir);
        assert_eq!(l.entries[2].name, "sub\\note.md");
        assert_eq!(l.entries[2].size, 14);
    }
}
