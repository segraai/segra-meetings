//! Meeting knowledge base: overview listing, search and "ask your meetings" chat.

pub mod chat;
pub mod search;

use crate::state::AppState;
use chat::{ChatAnswer, ChatTurn};
use search::{MeetingOverview, MeetingSearchHit};
use tauri::{AppHandle, Manager, Runtime};

const SEARCH_RESULT_LIMIT: usize = 30;

#[tauri::command]
pub async fn api_list_meetings_overview(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<MeetingOverview>, String> {
    search::list_overviews(state.db_manager.pool())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn api_search_meetings(
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<MeetingSearchHit>, String> {
    let terms = search::terms(&query);
    let documents = search::load_matching_documents(state.db_manager.pool(), &terms, true, 100)
        .await
        .map_err(|e| e.to_string())?;
    Ok(search::rank(documents, &terms)
        .into_iter()
        .take(SEARCH_RESULT_LIMIT)
        .map(|(_, hit)| hit)
        .collect())
}

#[tauri::command]
pub async fn api_ask_meetings<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    question: String,
    history: Vec<ChatTurn>,
) -> Result<ChatAnswer, String> {
    if question.trim().is_empty() {
        return Err("Ask a question about your meetings".to_string());
    }
    chat::ask(
        state.db_manager.pool(),
        app.path().app_data_dir().ok(),
        question.trim(),
        &history,
    )
    .await
}
