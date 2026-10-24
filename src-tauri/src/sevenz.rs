// sevenz.rs — 7-Zip engine wrapper: listing, extract, create, test + progress events
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read};
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
    pub children: Vec<Child>,
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

/// Bundled 7-Zip console binary: `7z.exe` on Windows, `7zz` on Linux/macOS.
pub const SEVEN_ZIP_BIN: &str = if cfg!(windows) { "7z.exe" } else { "7zz" };

/// Returns a path to a runnable 7-Zip binary.
///
/// On Unix the bundled copy is often not executable: installers such as `.deb`
/// drop resources as root-owned 0644 and macOS app bundles keep whatever bit
/// the zip carried. `chmod` fails there, so fall back to a private copy under
/// the user cache directory and mark that one 0755.
#[cfg(unix)]
fn runnable(src: &std::path::Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::metadata(src).map_err(|e| e.to_string())?.permissions().mode();
    if mode & 0o111 == 0o111 {
        return Ok(src.to_path_buf());
    }
    if std::fs::set_permissions(src, std::fs::Permissions::from_mode(mode | 0o111)).is_ok() {
        return Ok(src.to_path_buf());
    }

    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .ok_or_else(|| "Önbellek dizini bulunamadı".to_string())?
        .join("zipforge");
    std::fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let dst = cache.join(SEVEN_ZIP_BIN);
    let same = matches!(
        (std::fs::metadata(src), std::fs::metadata(&dst)),
        (Ok(a), Ok(b)) if a.len() == b.len()
    );
    if !same {
        std::fs::copy(src, &dst).map_err(|e| format!("Arşiv motoru kopyalanamadı: {e}"))?;
        std::fs::set_permissions(&dst, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| e.to_string())?;
    }
    Ok(dst)
}

#[cfg(not(unix))]
fn runnable(src: &std::path::Path) -> Result<PathBuf, String> {
    Ok(src.to_path_buf())
}

pub fn seven_zip(app: &AppHandle) -> Result<PathBuf, String> {
    // Dev checkout first (any per-platform folder), then the bundled resource.
    let dev_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join("7z");
    for sub in ["", "win", "linux-x64", "mac-universal"] {
        let p = if sub.is_empty() { dev_root.join(SEVEN_ZIP_BIN) } else { dev_root.join(sub).join(SEVEN_ZIP_BIN) };
        if p.exists() {
            return runnable(&p);
        }
    }
    if let Ok(res) = app.path().resolve(
        format!("resources/7z/{SEVEN_ZIP_BIN}"),
        tauri::path::BaseDirectory::Resource,
    ) {
        if res.exists() {
            return runnable(&res);
        }
    }
    Err(format!(
        "Arşiv motoru bulunamadı ({SEVEN_ZIP_BIN}). Uygulamayı yeniden kurun."
    ))
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

// ---------- compressed tar (tar.gz / tgz / tar.bz2 / tar.xz / tar.zst) ----------

const TAR_SUFFIXES: [&str; 8] = [
    ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz", ".tar.zst", ".tzst",
];

pub fn compressed_tar(path: &str) -> bool {
    let n = path.to_ascii_lowercase();
    TAR_SUFFIXES.iter().any(|s| n.ends_with(s))
}

fn tar_container_label(path: &str) -> &'static str {
    let n = path.to_ascii_lowercase();
    if n.ends_with(".tgz") || n.ends_with(".tar.gz") {
        "tar.gz"
    } else if n.ends_with(".tbz2") || n.ends_with(".tar.bz2") {
        "tar.bz2"
    } else if n.ends_with(".txz") || n.ends_with(".tar.xz") {
        "tar.xz"
    } else {
        "tar.zst"
    }
}

/// Outer decompressor: streams the inner .tar payload to stdout.
fn decompress_cmd(app: &AppHandle, path: &str) -> Result<Command, String> {
    let mut c = base_cmd(app)?;
    c.arg("x").arg("-so").arg("-y").arg("--").arg(path);
    Ok(c)
}

fn first_error_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|l| l.to_lowercase().contains("error"))
        .map(str::to_string)
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

/// Extracts the file name from a 7-Zip work line. 7-Zip prints the work line in
/// two pieces separated by a `\r`:
///   "  0%\r    \r+ data\f0.bin"
/// so a token can be either `PERCENT [FILES]` or `SIGN NAME`:
///   " 81% 37"
///   "+ data\f0.bin"
///   "- folder\other.bin"
fn parse_file(l: &str) -> Option<String> {
    let t = l.trim();
    if let Some(rest) = t.strip_prefix("Extracting") {
        let n = rest.trim();
        if !n.is_empty() && !n.starts_with('%') && !n.starts_with("archive:") {
            return Some(n.to_string());
        }
    }
    // bare marker line: "+ name", "- name", "U name"
    let mut head = t.chars();
    if let Some('+' | '-' | 'U') = head.next() {
        let rest = head.as_str();
        if let Some(name) = rest.strip_prefix(' ') {
            let name = name.trim();
            if !name.is_empty() && !name.contains('%') {
                return Some(name.to_string());
            }
        }
    }
    let idx = t.rfind('%')?;
    let after = t[idx + 1..].trim_start();
    // skip the optional "N" file counter, then the gap before the marker
    let after = after
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim_start();
    let mut chars = after.chars();
    match chars.next() {
        Some('+' | '-' | 'U') => {
            let name = chars.as_str().trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        }
        _ => None,
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
    if compressed_tar(path) {
        return list_compressed_tar(app, path);
    }
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

#[derive(Default)]
struct Draft {
    path: Option<String>,
    size: u64,
    packed_size: u64,
    modified: String,
    crc: String,
    attributes: String,
    encrypted: bool,
    is_dir: bool,
    method: String,
    has_type: bool,
    type_name: String,
    physical_size: u64,
    meta_encrypted: bool,
}

impl Draft {
    fn flush(&mut self, meta: &mut Option<ArchiveMeta>, entries: &mut Vec<Entry>) {
        // The archive header is the block carrying `Type`. It cannot be found by
        // looking at `Path`: when the archive is read from stdin (`7z l -si`,
        // used for tar.gz/tgz) the header has no name at all, so `Path` is
        // either empty or missing.
        if meta.is_none() && self.has_type {
            *meta = Some(ArchiveMeta {
                format: std::mem::take(&mut self.type_name),
                physical_size: self.physical_size,
                encrypted: self.meta_encrypted,
            });
        } else if let Some(name) = self.path.take() {
            if !name.is_empty() {
                entries.push(Entry {
                    name,
                    size: self.size,
                    packed_size: self.packed_size,
                    modified: std::mem::take(&mut self.modified),
                    crc: std::mem::take(&mut self.crc),
                    attributes: std::mem::take(&mut self.attributes),
                    encrypted: self.encrypted,
                    is_dir: self.is_dir,
                    method: std::mem::take(&mut self.method),
                });
            }
        }
        *self = Draft::default();
    }
}

struct ArchiveMeta {
    format: String,
    physical_size: u64,
    encrypted: bool,
}

/// Parses `7z l -slt` output in a single pass. Each entry used to cost a
/// HashMap plus ~12 key strings; large archives (50k+ entries) spent most of
/// their listing time in the allocator.
fn parse_slt(path: &str, out: &str) -> Result<Listing, String> {
    let mut meta: Option<ArchiveMeta> = None;
    let mut entries: Vec<Entry> = Vec::new();
    let mut d = Draft::default();

    for raw in out.lines() {
        let line = raw.trim_end();
        if line.is_empty()
            || (line.starts_with("---") && line.chars().all(|c| c == '-'))
        {
            d.flush(&mut meta, &mut entries);
            continue;
        }
        // Split on the first '=' instead of " = ": a header read from stdin has
        // an empty `Path`, and 7-Zip writes that as "Path =" with nothing after
        // the separator, so a " = " split would silently drop the field.
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let v = v.trim();
        match k.trim() {
            "Path" => d.path = Some(v.to_string()),
            "Size" => d.size = parse_size(v),
            "Packed Size" => d.packed_size = parse_size(v),
            "Modified" => d.modified = v.to_string(),
            "CRC" => d.crc = v.to_string(),
            "Attributes" => {
                d.attributes = v.to_string();
                d.is_dir = d.attributes.starts_with('D');
            }
            "Folder" => {
                if v == "+" {
                    d.is_dir = true;
                }
            }
            "Encrypted" => {
                let on = v == "+" || v == "1";
                d.encrypted = on;
                d.meta_encrypted = on;
            }
            "Method" => d.method = v.to_string(),
            "Type" => {
                d.has_type = true;
                d.type_name = v.to_string();
            }
            "Physical Size" => d.physical_size = parse_size(v),
            _ => {}
        }
    }
    d.flush(&mut meta, &mut entries);

    let meta = meta.ok_or_else(|| "Arşiv içeriği okunamadı".to_string())?;
    let name = PathBuf::from(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    let info = ArchiveInfo {
        path: path.to_string(),
        name,
        format: meta.format,
        physical_size: meta.physical_size,
        entries: entries.len() as u64,
        encrypted: meta.encrypted || entries.iter().any(|e| e.encrypted),
    };
    Ok(Listing { info, entries })
}

/// Lists a compressed tar (tar.gz/tgz/...) by streaming the outer layer through
/// `7z x -so` into `7z l -si -ttar -slt`, so the real file entries are shown.
fn list_compressed_tar(app: &AppHandle, path: &str) -> Result<Listing, String> {
    let errs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut outer = decompress_cmd(app, path)?;
    let mut outer_child = outer
        .spawn()
        .map_err(|e| format!("7z çalıştırılamadı: {e}"))?;
    let outer_out = outer_child
        .stdout
        .take()
        .ok_or_else(|| "7z çıktısı alınamadı".to_string())?;
    let outer_err_thread = outer_child
        .stderr
        .take()
        .map(|e| spawn_stderr_reader(e, errs.clone()));

    let mut inner = base_cmd(app)?;
    inner
        .arg("l")
        .arg("-si")
        .arg("-ttar")
        .arg("-slt")
        .arg("-sccUTF-8");
    inner.stdin(Stdio::from(outer_out));
    let res = inner.spawn().and_then(|ch| ch.wait_with_output());

    let _ = outer_child.kill();
    let _ = outer_child.wait();
    if let Some(t) = outer_err_thread {
        let _ = t.join();
    }

    let out = res.map_err(|e| format!("7z çalıştırılamadı: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        let inner_err = String::from_utf8_lossy(&out.stderr).to_string();
        let collected = errs.lock().unwrap().join("\n");
        let msg = first_error_line(&collected)
            .or_else(|| first_error_line(&inner_err))
            .or_else(|| first_error_line(&stdout))
            .unwrap_or_else(|| "Arşiv açılamadı".to_string());
        return Err(format!("Arşiv açılamadı: {msg}"));
    }

    let mut listing = parse_slt(path, &stdout)?;
    listing.info.format = tar_container_label(path).to_string();
    if let Ok(md) = std::fs::metadata(path) {
        listing.info.physical_size = md.len();
    }
    listing.entries.retain(|e| {
        let t = e.name.trim_matches(|c: char| c == '\\' || c == '/' || c == ' ');
        !t.is_empty() && t != "."
    });
    listing.info.entries = listing.entries.len() as u64;
    Ok(listing)
}

// ---------- task runner ----------

enum StepKind {
    Single(Command),
    Piped(Command, Command),
}

struct Step {
    kind: StepKind,
    lo: u32,
    hi: u32,
}

impl Step {
    fn single(cmd: Command) -> Self {
        Self {
            kind: StepKind::Single(cmd),
            lo: 0,
            hi: 100,
        }
    }

    fn covered(cmd: Command, lo: u32, hi: u32) -> Self {
        Self {
            kind: StepKind::Single(cmd),
            lo,
            hi,
        }
    }

    fn piped(outer: Command, inner: Command) -> Self {
        Self {
            kind: StepKind::Piped(outer, inner),
            lo: 0,
            hi: 100,
        }
    }
}

fn is_cancelled(tasks: &Arc<Mutex<HashMap<u32, TaskHandle>>>, task_id: u32) -> bool {
    tasks
        .lock()
        .unwrap()
        .get(&task_id)
        .map(|h| h.cancelled.load(Ordering::SeqCst))
        .unwrap_or(false)
}

fn register_children(tasks: &Arc<Mutex<HashMap<u32, TaskHandle>>>, task_id: u32, children: Vec<Child>) {
    if let Some(h) = tasks.lock().unwrap().get_mut(&task_id) {
        h.children = children;
    }
}

fn take_children(tasks: &Arc<Mutex<HashMap<u32, TaskHandle>>>, task_id: u32) -> Vec<Child> {
    if let Some(h) = tasks.lock().unwrap().get_mut(&task_id) {
        std::mem::take(&mut h.children)
    } else {
        Vec::new()
    }
}

fn spawn_stderr_reader<R: Read + Send + 'static>(
    e: R,
    errs: Arc<Mutex<Vec<String>>>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for line in BufReader::new(e).lines().map_while(Result::ok) {
            if !line.trim().is_empty() {
                errs.lock().unwrap().push(line);
            }
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn read_progress(
    app: &AppHandle,
    task_id: u32,
    out: impl Read,
    lo: u32,
    hi: u32,
    percent: &mut u32,
    last_file: &mut Option<String>,
    error_lines: &mut Vec<String>,
) {
    let mut last_emit = Instant::now() - Duration::from_secs(1);
    let mut emit_if_due = |app: &AppHandle, percent: &u32, file: &Option<String>, force: bool| {
        if force || last_emit.elapsed() >= Duration::from_millis(120) || *percent >= 100 {
            last_emit = Instant::now();
            emit(
                app,
                TaskEvent {
                    task_id,
                    phase: "progress".into(),
                    percent: Some(*percent),
                    message: None,
                    file: file.clone(),
                },
            );
        }
    };

    // 7-Zip paints its console updates with `\r` (same line, overwritten) and
    // only ends a line with `\n` when a file is finished. Reading with
    // `BufRead::lines` therefore hides every intermediate update: while one big
    // file is being compressed the reader blocks until that file completes,
    // which is why the window sat at 0 % for the whole operation. Split on both
    // characters so each repaint becomes a progress event.
    let mut reader = BufReader::new(out);
    let mut chunk = [0u8; 8192];
    let mut pending: Vec<u8> = Vec::with_capacity(8192);
    // Bytes below this index were already inspected and contained no separator.
    let mut scanned = 0usize;

    loop {
        let n = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        pending.extend_from_slice(&chunk[..n]);

        let mut start = 0usize;
        let mut i = scanned;
        while i < pending.len() {
            if pending[i] == b'\r' || pending[i] == b'\n' {
                if i > start {
                    handle_progress_token(&pending[start..i], lo, hi, percent, last_file, error_lines);
                    emit_if_due(app, percent, last_file, false);
                }
                start = i + 1;
            }
            i += 1;
        }
        pending.drain(..start);
        scanned = pending.len();

        // A pathological line without separators must not grow without bound.
        if pending.len() > 1 << 20 {
            handle_progress_token(&pending, lo, hi, percent, last_file, error_lines);
            pending.clear();
            scanned = 0;
        }
    }

    // whatever is left has no trailing separator
    if !pending.is_empty() {
        handle_progress_token(&pending, lo, hi, percent, last_file, error_lines);
    }
    emit_if_due(app, percent, last_file, true);
}

/// Interprets one `\r`/`\n`-delimited chunk of 7-Zip console output.
fn handle_progress_token(
    token: &[u8],
    lo: u32,
    hi: u32,
    percent: &mut u32,
    last_file: &mut Option<String>,
    error_lines: &mut Vec<String>,
) {
    let text = String::from_utf8_lossy(token);
    let l: String = text.chars().filter(|c| *c != '\u{8}').collect();
    let l = l.trim();
    if l.is_empty() {
        return;
    }
    if l.contains("ERROR") || l.contains("Can not open") || l.contains("Cannot open") {
        error_lines.push(l.to_string());
    }
    if let Some(p) = parse_percent(l) {
        *percent = lo + (p.min(100) * (hi - lo)) / 100;
    }
    if let Some(f) = parse_file(l) {
        *last_file = Some(f);
    }
}

pub fn spawn_task(
    app: &AppHandle,
    tasks: Arc<Mutex<HashMap<u32, TaskHandle>>>,
    task_id: u32,
    cmd: Command,
) -> Result<(), String> {
    spawn_steps(app, tasks, task_id, vec![Step::single(cmd)], None)
}

pub fn spawn_piped_task(
    app: &AppHandle,
    tasks: Arc<Mutex<HashMap<u32, TaskHandle>>>,
    task_id: u32,
    outer: Command,
    inner: Command,
) -> Result<(), String> {
    spawn_steps(app, tasks, task_id, vec![Step::piped(outer, inner)], None)
}

fn spawn_steps(
    app: &AppHandle,
    tasks: Arc<Mutex<HashMap<u32, TaskHandle>>>,
    task_id: u32,
    steps: Vec<Step>,
    cleanup: Option<PathBuf>,
) -> Result<(), String> {
    tasks.lock().unwrap().insert(
        task_id,
        TaskHandle {
            children: Vec::new(),
            cancelled: AtomicBool::new(false),
        },
    );
    let app2 = app.clone();
    std::thread::spawn(move || {
        let errs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let mut percent: u32 = 0;
        let mut last_file: Option<String> = None;
        let mut error_lines: Vec<String> = Vec::new();
        let mut code: Option<i32> = None;
        let mut ok = true;

        for step in steps {
            if is_cancelled(&tasks, task_id) {
                ok = false;
                break;
            }
            match step.kind {
                StepKind::Single(mut cmd) => {
                    let mut child = match cmd.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            error_lines.push(format!("7z başlatılamadı: {e}"));
                            ok = false;
                            break;
                        }
                    };
                    let out = child.stdout.take();
                    let err_thread = child
                        .stderr
                        .take()
                        .map(|e| spawn_stderr_reader(e, errs.clone()));
                    register_children(&tasks, task_id, vec![child]);
                    if is_cancelled(&tasks, task_id) {
                        for mut c in take_children(&tasks, task_id) {
                            let _ = c.kill();
                        }
                        ok = false;
                        break;
                    }
                    if let Some(out) = out {
                        read_progress(
                            &app2,
                            task_id,
                            out,
                            step.lo,
                            step.hi,
                            &mut percent,
                            &mut last_file,
                            &mut error_lines,
                        );
                    }
                    if let Some(t) = err_thread {
                        let _ = t.join();
                    }
                    if let Some(mut c) = take_children(&tasks, task_id).pop() {
                        if let Ok(st) = c.wait() {
                            code = st.code();
                            if !st.success() {
                                ok = false;
                            }
                        }
                    }
                    if is_cancelled(&tasks, task_id) {
                        ok = false;
                        break;
                    }
                }
                StepKind::Piped(mut outer, mut inner_cmd) => {
                    let mut outer_child = match outer.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            error_lines.push(format!("7z başlatılamadı: {e}"));
                            ok = false;
                            break;
                        }
                    };
                    let outer_out = match outer_child.stdout.take() {
                        Some(o) => o,
                        None => {
                            error_lines.push("7z çıktısı alınamadı".to_string());
                            let _ = outer_child.kill();
                            ok = false;
                            break;
                        }
                    };
                    let outer_err_thread = outer_child
                        .stderr
                        .take()
                        .map(|e| spawn_stderr_reader(e, errs.clone()));
                    inner_cmd.stdin(Stdio::from(outer_out));
                    let mut inner_child = match inner_cmd.spawn() {
                        Ok(c) => c,
                        Err(e) => {
                            error_lines.push(format!("7z başlatılamadı: {e}"));
                            let _ = outer_child.kill();
                            ok = false;
                            break;
                        }
                    };
                    let in_out = inner_child.stdout.take();
                    let inner_err_thread = inner_child
                        .stderr
                        .take()
                        .map(|e| spawn_stderr_reader(e, errs.clone()));
                    register_children(&tasks, task_id, vec![inner_child, outer_child]);
                    if is_cancelled(&tasks, task_id) {
                        for mut c in take_children(&tasks, task_id) {
                            let _ = c.kill();
                        }
                        ok = false;
                        break;
                    }
                    if let Some(out) = in_out {
                        read_progress(
                            &app2,
                            task_id,
                            out,
                            step.lo,
                            step.hi,
                            &mut percent,
                            &mut last_file,
                            &mut error_lines,
                        );
                    }
                    if let Some(t) = outer_err_thread {
                        let _ = t.join();
                    }
                    if let Some(t) = inner_err_thread {
                        let _ = t.join();
                    }
                    let mut children = take_children(&tasks, task_id);
                    if children.len() == 2 {
                        let mut inner_child = children.remove(0);
                        let mut outer_child = children.remove(0);
                        if let Ok(st) = inner_child.wait() {
                            code = st.code();
                            if !st.success() {
                                ok = false;
                            }
                        }
                        let _ = outer_child.kill();
                        let _ = outer_child.wait();
                    }
                    if is_cancelled(&tasks, task_id) {
                        ok = false;
                        break;
                    }
                }
            }
        }

        let was_cancelled = is_cancelled(&tasks, task_id);
        tasks.lock().unwrap().remove(&task_id);
        if let Some(dir) = cleanup {
            let _ = std::fs::remove_dir_all(&dir);
        }
        for l in errs.lock().unwrap().iter() {
            error_lines.push(l.clone());
        }
        if ok && !was_cancelled {
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
            let msg = if was_cancelled {
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
    let ow = match overwrite {
        Some("skip") => "-aos",
        Some("rename") => "-aou",
        _ => "-aoa",
    };
    if compressed_tar(path) {
        let outer = decompress_cmd(app, path)?;
        let mut inner = base_cmd(app)?;
        inner
            .arg("x")
            .arg("-y")
            .arg("-sccUTF-8")
            .arg("-bsp1")
            .arg("-bb1")
            .arg(ow)
            .arg(format!("-o{dest}"))
            .arg("-si")
            .arg("-ttar");
        if let Some(list) = entries {
            for e in list {
                inner.arg(e);
            }
        }
        return spawn_piped_task(app, tasks, task_id, outer, inner);
    }
    let mut c = base_cmd(app)?;
    // `-bb1` makes 7-Zip name every finished file; without it the console only
    // repaints the percentage line and the status bar stays anonymous.
    c.arg("x").arg("-y").arg("-sccUTF-8").arg("-bsp1").arg("-bb1");
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
    if compressed_tar(path) {
        let outer = decompress_cmd(app, path)?;
        let mut inner = base_cmd(app)?;
        inner
            .arg("t")
            .arg("-y")
            .arg("-sccUTF-8")
            .arg("-bsp1")
            .arg("-bb1")
            .arg("-si")
            .arg("-ttar");
        return spawn_piped_task(app, tasks, task_id, outer, inner);
    }
    let mut c = base_cmd(app)?;
    c.arg("t").arg("-y").arg("-sccUTF-8").arg("-bsp1").arg("-bb1");
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
    if low.ends_with(".tar.gz") || low.ends_with(".tgz") {
        "tar.gz"
    } else if low.ends_with(".7z") {
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
    // `-bb1` is what turns the single repainted percentage line into a stream
    // of `PERCENT [N] + name` lines, so the window can show live progress.
    c.arg("a").arg("-y").arg("-sccUTF-8").arg("-bsp1").arg("-bb1");
    let fmt = format.filter(|f| !f.is_empty()).unwrap_or_else(|| guess_format(output));
    if fmt == "tar.gz" {
        return create_tar_gz(app, tasks, task_id, output, sources, level);
    }
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

/// Creates a tar.gz in two steps: 7z cannot produce combined archives in one
/// command, so we pack a temporary .tar next to the output and gzip it.
fn create_tar_gz(
    app: &AppHandle,
    tasks: Arc<Mutex<HashMap<u32, TaskHandle>>>,
    task_id: u32,
    output: &str,
    sources: Vec<String>,
    level: Option<u32>,
) -> Result<(), String> {
    let out_path = PathBuf::from(output);
    let parent = match out_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let stem = out_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "archive".to_string());
    let inner_name = if stem.to_ascii_lowercase().ends_with(".tar") {
        stem
    } else {
        format!("{stem}.tar")
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_dir = parent.join(format!(".zipforge-tmp-{stamp}"));
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("Geçici klasör oluşturulamadı: {e}"))?;
    let tmp_tar = tmp_dir.join(&inner_name);

    let mut s1 = base_cmd(app)?;
    s1.arg("a")
        .arg("-y")
        .arg("-sccUTF-8")
        .arg("-bsp1")
        .arg("-bb1")
        .arg("-ttar")
        .arg("--")
        .arg(&tmp_tar);
    let (cwd, args) = relativize(&sources);
    if let Some(dir) = cwd {
        s1.current_dir(dir);
    }
    for a in args {
        s1.arg(a);
    }

    let mut s2 = base_cmd(app)?;
    s2.arg("a")
        .arg("-y")
        .arg("-sccUTF-8")
        .arg("-bsp1")
        .arg("-bb1")
        .arg("-tgzip");
    if let Some(l) = level {
        if l > 0 {
            s2.arg(format!("-mx{l}"));
        }
    }
    s2.arg("--").arg(&out_path).arg(&tmp_tar);

    let steps = vec![
        Step::covered(s1, 0, 30),
        Step::covered(s2, 30, 100),
    ];
    let res = spawn_steps(app, tasks, task_id, steps, Some(tmp_dir.clone()));
    if res.is_err() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
    res
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

    /// Captured from `7z l -si -ttar -slt`, the pipeline used for tar.gz/tgz.
    /// The header has no file name (`Path =` is empty), so it must be detected
    /// by its `Type` field — looking at `Path` rejected the whole listing.
    const PIPED_TAR: &str = r#"
7-Zip 26.04 (x64) : Copyright (c) 1999-2026 Igor Pavlov : 2026-10-05


Listing archive: 

--
Path = 
Type = tar
Code Page = UTF-8
Characteristics = ASCII

----------
Path = folder1
Folder = +
Size = 0
Packed Size = 0
Modified = 2026-10-24 13:26:52
Mode = drwxrwxrwx
Characteristics = 5 GNU ASCII

Path = folder1/a.txt
Folder = -
Size = 12
Packed Size = 12
Modified = 2026-10-24 13:26:52
Mode = -rw-rw-rw-
Characteristics = A
"#;

    #[test]
    fn parses_piped_tar_listing() {
        let l = parse_slt("C:\\tmp\\test.tar.gz", PIPED_TAR).unwrap();
        assert_eq!(l.info.format, "tar");
        assert_eq!(l.entries.len(), 2);
        assert_eq!(l.entries[0].name, "folder1");
        assert!(l.entries[0].is_dir);
        assert_eq!(l.entries[1].name, "folder1/a.txt");
        assert_eq!(l.entries[1].size, 12);
        assert!(!l.entries[1].is_dir);
    }

    #[test]
    fn detects_compressed_tar() {
        assert!(compressed_tar("C:\\tmp\\a.tar.gz"));
        assert!(compressed_tar("C:\\tmp\\A.TAR.GZ"));
        assert!(compressed_tar("x.tgz"));
        assert!(compressed_tar("x.tar.bz2"));
        assert!(compressed_tar("x.TBZ2"));
        assert!(compressed_tar("x.tar.xz"));
        assert!(compressed_tar("x.tzst"));
        assert!(!compressed_tar("x.tar"));
        assert!(!compressed_tar("x.gz"));
        assert!(!compressed_tar("x.zip"));
        assert!(!compressed_tar("x.7z"));
    }

    #[test]
    fn labels_tar_containers() {
        assert_eq!(tar_container_label("a.tar.gz"), "tar.gz");
        assert_eq!(tar_container_label("A.TGZ"), "tar.gz");
        assert_eq!(tar_container_label("b.tar.bz2"), "tar.bz2");
        assert_eq!(tar_container_label("c.tar.xz"), "tar.xz");
        assert_eq!(tar_container_label("d.tar.zst"), "tar.zst");
    }

    #[test]
    fn parses_percent_from_work_lines() {
        assert_eq!(parse_percent("  0%"), Some(0));
        assert_eq!(parse_percent("  0%    + data\\f0.bin"), Some(0));
        assert_eq!(parse_percent(" 81% 37  + data\\f7.bin"), Some(81));
        assert_eq!(parse_percent("100%"), Some(100));
        assert_eq!(parse_percent("1 folder, 40 files"), None);
    }

    #[test]
    fn parses_file_from_work_lines() {
        // real 7-Zip splits the work line on `\r`: percent first, then the marker
        assert_eq!(parse_file("+ data\\f0.bin").as_deref(), Some("data\\f0.bin"));
        assert_eq!(
            parse_file("- folder\\other.bin").as_deref(),
            Some("folder\\other.bin")
        );
        // no counter yet: "  0%     + data\f0.bin"
        assert_eq!(
            parse_file("  0%     + data\\f0.bin").as_deref(),
            Some("data\\f0.bin")
        );
        // counter present: " 81% 37  + data\f7.bin"
        assert_eq!(
            parse_file(" 81% 37  + data\\f7.bin").as_deref(),
            Some("data\\f7.bin")
        );
        // deleting/updating entries
        assert_eq!(
            parse_file(" 45% 3   - folder\\other.bin").as_deref(),
            Some("folder\\other.bin")
        );
        assert_eq!(
            parse_file(" 45% 3   U folder\\other.bin").as_deref(),
            Some("folder\\other.bin")
        );
        // percentage only, nothing to name
        assert_eq!(parse_file("  0%"), None);
        assert_eq!(parse_file("  0% 1"), None);
        assert_eq!(parse_file(" 81% 39"), None);
        // extract style line
        assert_eq!(
            parse_file("Extracting  sub\\note.md").as_deref(),
            Some("sub\\note.md")
        );
    }

    #[test]
    fn folds_tokens_into_progress() {
        let mut percent = 0u32;
        let mut file = None;
        let mut errors: Vec<String> = Vec::new();
        for token in [
            b"  0%     + data\\f0.bin".as_slice(),
            b"  0% 1\r".as_slice(),
            b" 81% 37  + data\\f7.bin".as_slice(),
            b" 81% 38".as_slice(),
            b"ERROR: Can not open file".as_slice(),
        ] {
            handle_progress_token(token, 0, 100, &mut percent, &mut file, &mut errors);
        }
        assert_eq!(percent, 81);
        assert_eq!(file.as_deref(), Some("data\\f7.bin"));
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn maps_percent_into_step_range() {
        let mut percent = 0u32;
        let mut file = None;
        let mut errors: Vec<String> = Vec::new();
        handle_progress_token(b" 50% 1  + a.bin", 30, 100, &mut percent, &mut file, &mut errors);
        assert_eq!(percent, 65);
        handle_progress_token(b"100%", 30, 100, &mut percent, &mut file, &mut errors);
        assert_eq!(percent, 100);
    }
}
