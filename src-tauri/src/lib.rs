mod sevenz;

use sevenz::Tasks;
use tauri::State;

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
fn parse_startup_args() -> StartupArgs {
    let mut out = StartupArgs {
        open: None,
        add: Vec::new(),
        extract: None,
        extract_sub: None,
    };
    let mut it = std::env::args_os().skip(1).peekable();
    while let Some(arg) = it.next() {
        let s = arg.to_string_lossy().into_owned();
        match s.as_str() {
            "--add" => {
                while let Some(next) = it.peek() {
                    let n = next.to_string_lossy().into_owned();
                    if n.starts_with("--") {
                        break;
                    }
                    out.add.push(n);
                    it.next();
                }
            }
            "--extract" => {
                if let Some(v) = it.next() {
                    out.extract = Some(v.to_string_lossy().into_owned());
                }
            }
            "--extract-sub" => {
                if let Some(v) = it.next() {
                    out.extract_sub = Some(v.to_string_lossy().into_owned());
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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Tasks::new())
        .invoke_handler(tauri::generate_handler![
            list_archive,
            extract_archive,
            create_archive,
            test_archive,
            cancel_task,
            startup_args,
            app_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
