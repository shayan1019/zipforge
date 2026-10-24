mod sevenz;

use sevenz::Tasks;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

const ARCHIVE_EXTS: &[&str] = &[
    "zip", "7z", "rar", "tar", "gz", "tgz", "bz2", "tbz2", "xz", "txz", "zst", "tzst", "iso",
    "cab", "jar", "lzma", "lz", "apk", "war", "ear", "001",
];

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupArgs {
    open: Option<String>,
    add: Vec<String>,
    extract: Option<String>,
    extract_sub: Option<String>,
}

/// Startup arguments forwarded by a second process while this instance is alive.
/// Queued until the frontend announces it is listening, so nothing is lost if
/// Explorer fires a multi-selection while the webview is still booting.
#[derive(Default)]
struct PendingArgs {
    queue: Mutex<Vec<StartupArgs>>,
    live: std::sync::atomic::AtomicBool,
}

fn is_archive_path(p: &str) -> bool {
    std::path::Path::new(p)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| ARCHIVE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Parses command line arguments used by the Explorer context-menu entries.
///
/// Supported forms:
/// - `zipforge.exe <archive>`           open an archive
/// - `zipforge.exe --add <path>...`     pre-fill the Create dialog with the given sources
/// - `zipforge.exe --extract <archive>` open an archive and pre-fill "extract here"
/// - `zipforge.exe --extract-sub <arc>` open an archive and pre-fill "extract to subfolder"
fn parse_args<I: IntoIterator<Item = String>>(args: I) -> StartupArgs {
    let mut out = StartupArgs {
        open: None,
        add: Vec::new(),
        extract: None,
        extract_sub: None,
    };
    let mut it = args.into_iter().peekable();
    while let Some(s) = it.next() {
        match s.as_str() {
            "--add" => {
                while let Some(next) = it.peek() {
                    if next.starts_with("--") {
                        break;
                    }
                    out.add.push(it.next().unwrap_or_default());
                }
            }
            "--extract" => {
                if let Some(v) = it.next() {
                    out.extract = Some(v);
                }
            }
            "--extract-sub" => {
                if let Some(v) = it.next() {
                    out.extract_sub = Some(v);
                }
            }
            _ if s.starts_with('-') => {}
            _ => {
                if is_archive_path(&s) {
                    if out.open.is_none() {
                        out.open = Some(s);
                    } else {
                        out.add.push(s);
                    }
                } else {
                    out.add.push(s);
                }
            }
        }
    }
    out
}

fn parse_startup_args() -> StartupArgs {
    parse_args(
        std::env::args_os()
            .skip(1)
            .map(|a| a.to_string_lossy().into_owned()),
    )
}

fn has_payload(a: &StartupArgs) -> bool {
    !a.add.is_empty() || a.open.is_some() || a.extract.is_some() || a.extract_sub.is_some()
}

/// Argv coming from a second process (Explorer "Player" multi-selection) is
/// forwarded to the running instance instead of opening another window.
fn forward_args(app: &tauri::AppHandle, argv: Vec<String>) {
    let args = parse_args(argv.into_iter().skip(1));
    if !has_payload(&args) {
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.set_focus();
        }
        return;
    }
    if let Some(state) = app.try_state::<PendingArgs>() {
        if !state.live.load(std::sync::atomic::Ordering::SeqCst) {
            state.queue.lock().unwrap().push(args.clone());
        }
    }
    let _ = app.emit("zf://startup", args);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// The frontend calls this once its `zf://startup` listener is registered.
/// From that point on forwarded args arrive as events, so the queue is not
/// needed (and would otherwise grow for the lifetime of the process).
#[tauri::command]
fn take_pending_args(state: State<'_, PendingArgs>) -> Vec<StartupArgs> {
    state.live.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut q = state.queue.lock().unwrap();
    std::mem::take(&mut *q)
}

#[tauri::command]
fn startup_args() -> StartupArgs {
    parse_startup_args()
}

#[tauri::command]
fn app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
async fn list_archive(
    app: tauri::AppHandle,
    path: String,
    password: Option<String>,
) -> Result<sevenz::Listing, String> {
    sevenz::list_archive(&app, &path, password.as_deref())
}

#[tauri::command]
async fn extract_archive(
    app: tauri::AppHandle,
    tasks: State<'_, Tasks>,
    task_id: u32,
    path: String,
    dest: String,
    password: Option<String>,
    overwrite: Option<String>,
    entries: Option<Vec<String>>,
) -> Result<(), String> {
    sevenz::extract_archive(
        &app,
        tasks.0.clone(),
        task_id,
        &path,
        &dest,
        password.as_deref(),
        overwrite.as_deref(),
        entries,
    )
}

#[tauri::command]
async fn create_archive(
    app: tauri::AppHandle,
    tasks: State<'_, Tasks>,
    task_id: u32,
    output: String,
    sources: Vec<String>,
    format: Option<String>,
    level: Option<u32>,
    password: Option<String>,
    split: Option<String>,
) -> Result<(), String> {
    sevenz::create_archive(
        &app,
        tasks.0.clone(),
        task_id,
        &output,
        sources,
        format.as_deref(),
        level,
        password.as_deref(),
        split.as_deref(),
    )
}

#[tauri::command]
async fn test_archive(
    app: tauri::AppHandle,
    tasks: State<'_, Tasks>,
    task_id: u32,
    path: String,
    password: Option<String>,
) -> Result<(), String> {
    sevenz::test_archive(&app, tasks.0.clone(), task_id, &path, password.as_deref())
}

#[tauri::command]
async fn cancel_task(tasks: State<'_, Tasks>, task_id: u32) -> Result<(), String> {
    let mut map = tasks.0.lock().unwrap();
    if let Some(h) = map.get_mut(&task_id) {
        h.cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
        for child in h.children.iter_mut() {
            let _ = child.kill();
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // Explorer's "Player" selection model starts one process per selected item.
    // Funnel those launches into the already running window instead of spawning
    // one app per file.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            forward_args(app, argv);
        }));
    }

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Tasks::new())
        .manage(PendingArgs::default())
        .invoke_handler(tauri::generate_handler![
            list_archive,
            extract_archive,
            create_archive,
            test_archive,
            cancel_task,
            startup_args,
            take_pending_args,
            app_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
