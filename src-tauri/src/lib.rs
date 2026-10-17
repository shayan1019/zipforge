mod sevenz;

use sevenz::Tasks;
use tauri::State;

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
        let _ = h.child.kill();
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
            cancel_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
