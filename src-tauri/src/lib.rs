pub mod catalog;
pub mod color;
pub mod render;

use std::{path::PathBuf, sync::Mutex};

use color::{DebugOutputMode, GradeParams};
use render::engine::{PreviewFrame, RenderEngine, RenderState};

type SharedRenderEngine = Mutex<Option<RenderEngine>>;

fn with_render_engine<R>(
    state: &tauri::State<'_, SharedRenderEngine>,
    f: impl FnOnce(&mut RenderEngine) -> Result<R, String>,
) -> Result<R, String> {
    let mut guard = state
        .lock()
        .map_err(|_| "render engine lock poisoned".to_string())?;
    if guard.is_none() {
        *guard = Some(pollster::block_on(RenderEngine::new())?);
    }
    f(guard.as_mut().expect("render engine initialized"))
}

#[tauri::command]
fn load_image(
    path: String,
    state: tauri::State<'_, SharedRenderEngine>,
) -> Result<RenderState, String> {
    with_render_engine(&state, |engine| engine.load_image(PathBuf::from(path)))
}

#[tauri::command]
fn update_grade_params(
    params: GradeParams,
    state: tauri::State<'_, SharedRenderEngine>,
) -> Result<RenderState, String> {
    with_render_engine(&state, |engine| engine.update_grade_params(params))
}

#[tauri::command]
fn set_debug_output(
    mode: DebugOutputMode,
    state: tauri::State<'_, SharedRenderEngine>,
) -> Result<RenderState, String> {
    with_render_engine(&state, |engine| engine.set_debug_output(mode))
}

#[tauri::command]
fn render_identity_preview(
    enabled: bool,
    state: tauri::State<'_, SharedRenderEngine>,
) -> Result<RenderState, String> {
    with_render_engine(&state, |engine| engine.render_identity_preview(enabled))
}

#[tauri::command]
fn read_preview_rgba(state: tauri::State<'_, SharedRenderEngine>) -> Result<PreviewFrame, String> {
    with_render_engine(&state, |engine| engine.read_preview_rgba())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Mutex::new(None) as SharedRenderEngine)
        .invoke_handler(tauri::generate_handler![
            load_image,
            update_grade_params,
            set_debug_output,
            render_identity_preview,
            read_preview_rgba
        ])
        .run(tauri::generate_context!())
        .expect("error while running Tone application");
}
