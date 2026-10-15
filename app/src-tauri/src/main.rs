#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sp404_core::config::Settings;
use sp404_core::runner::{self, Event, Summary};
use sp404_core::scan::{self, PackSummary, Plan};
use sp404_core::undo::{self, UndoReport};
use sp404_core::space;
use tauri::ipc::Channel;
use tauri::State;

#[derive(Default)]
struct AppState {
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

/// Held for the whole conversion; a second `convert` while one runs is refused.
struct RunGuard(Arc<AtomicBool>);

impl RunGuard {
    fn acquire(flag: &Arc<AtomicBool>) -> Option<RunGuard> {
        if flag.swap(true, Ordering::SeqCst) {
            None
        } else {
            Some(RunGuard(flag.clone()))
        }
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[tauri::command]
fn get_settings() -> Settings {
    Settings::load()
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    settings.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn inspect_packs(paths: Vec<PathBuf>) -> Result<Vec<PackSummary>, String> {
    paths.iter().map(|p| scan::inspect_pack(p).map_err(|e| e.to_string())).collect()
}

#[tauri::command]
fn plan_conversion(paths: Vec<PathBuf>, output_dir: Option<PathBuf>) -> Result<Plan, String> {
    scan::build_plan(&paths, output_dir.as_deref(), &Settings::load()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn convert(plan: Plan, on_event: Channel<Event>, state: State<'_, AppState>) -> Result<Summary, String> {
    let guard = RunGuard::acquire(&state.running).ok_or("a conversion is already running")?;
    let settings = Settings::load();
    let cancel = state.cancel.clone();
    cancel.store(false, Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        space::check_free_space(&plan).map_err(|e| e.to_string())?;
        let manifest_file = undo::manifest_path();
        let (summary, _) = runner::run(&plan, &settings, &cancel, Some(&manifest_file), |event| {
            let _ = on_event.send(event);
        })
        .map_err(|e| e.to_string())?;
        Ok(summary)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_conversion(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
fn undo_last(remove_empty_dirs: bool) -> Result<Option<UndoReport>, String> {
    undo::undo_last_session(&undo::manifest_path(), remove_empty_dirs).map_err(|e| e.to_string())
}

#[tauri::command]
fn has_undo() -> bool {
    undo::manifest_path().exists()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            inspect_packs,
            plan_conversion,
            convert,
            cancel_conversion,
            undo_last,
            has_undo
        ])
        .run(tauri::generate_context!())
        .expect("error while running the SP-404 MKII Converter");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_conversion_runs_at_a_time() {
        let flag = Arc::new(AtomicBool::new(false));
        let first = RunGuard::acquire(&flag);
        assert!(first.is_some());
        assert!(RunGuard::acquire(&flag).is_none());
        drop(first);
        assert!(RunGuard::acquire(&flag).is_some());
    }
}
