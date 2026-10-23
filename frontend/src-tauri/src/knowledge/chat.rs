//! "Ask your meetings": retrieves the most relevant meetings for a question and answers it
//! with the configured summary LLM.

use super::search::{self, MeetingDocument};
use crate::database::models::DateTimeUtc;
use crate::database::repositories::setting::SettingsRepository;
use crate::summary::llm_client::{generate_summary, LLMProvider};
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::PathBuf;

const MAX_CONTEXT_MEETINGS: usize = 6;
const MAX_CONTEXT_CHARS: usize = 36_000;
const MAX_SUMMARY_CHARS: usize = 5_000;
const MAX_TRANSCRIPT_CHARS: usize = 8_000;
const INDEX_MEETINGS: usize = 50;
const MAX_HISTORY_TURNS: usize = 6;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatTurn {
    /// "user" | "assistant"
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatSource {
    pub id: String,
    pub title: String,
    pub created_at: DateTimeUtc,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatAnswer {
    pub answer: String,
    pub sources: Vec<ChatSource>,
}

const SYSTEM_PROMPT: &str = "You are Meetily's assistant. You answer questions about the user's past meetings \
using ONLY the meeting notes and transcripts provided in the context. \
Be concise and specific. Use Markdown (short paragraphs or bullet lists). \
When you use information from a meeting, mention it by title and date, e.g. (Weekly sync, Oct 3). \
If the context does not contain the answer, say so plainly and suggest what to search for. \
Never invent decisions, names, numbers or dates.";

pub async fn ask(
    pool: &SqlitePool,
    app_data_dir: Option<PathBuf>,
    question: &str,
    history: &[ChatTurn],
) -> Result<ChatAnswer, String> {
    let documents = retrieve(pool, question, history)
        .await
        .map_err(|e| format!("Failed to search meetings: {}", e))?;
    let overviews = search::list_overviews(pool)
        .await
        .map_err(|e| format!("Failed to list meetings: {}", e))?;

    let mut context = String::from("## All meetings (newest first)\n");
    for meeting in overviews.iter().take(INDEX_MEETINGS) {
        context.push_str(&format!("- {} — {}\n", format_date(&meeting.created_at.0), meeting.title));
    }
    if overviews.len() > INDEX_MEETINGS {
        context.push_str(&format!("- …and {} older meetings\n", overviews.len() - INDEX_MEETINGS));
    }

    let mut sources = Vec::new();
    for doc in &documents {
        if context.len() >= MAX_CONTEXT_CHARS {
            break;
        }
        context.push_str(&meeting_context(doc, question));
        sources.push(ChatSource {
            id: doc.id.clone(),
            title: doc.title.clone(),
            created_at: doc.created_at.clone(),
        });
    }
    if documents.is_empty() {
        context.push_str("\n(No meeting content matched this question.)\n");
    }

    let mut user_prompt = format!(
        "Today is {}.\n\n# Meeting context\n{}\n",
        Local::now().format("%A, %B %-d, %Y"),
        search::truncate_chars(&context, MAX_CONTEXT_CHARS)
    );
    let recent_history: Vec<&ChatTurn> = history.iter().rev().take(MAX_HISTORY_TURNS).rev().collect();
    if !recent_history.is_empty() {
        user_prompt.push_str("\n# Conversation so far\n");
        for turn in recent_history {
            let speaker = if turn.role == "assistant" { "Assistant" } else { "User" };
            user_prompt.push_str(&format!("{}: {}\n", speaker, search::truncate_chars(&turn.content, 2_000)));
        }
    }
    user_prompt.push_str(&format!("\n# Question\n{}\n", question));

    let answer = complete(pool, app_data_dir, SYSTEM_PROMPT, &user_prompt).await?;
    Ok(ChatAnswer { answer, sources })
}

/// Picks meetings for the question: keyword matches, plus meetings in a date range the
/// question mentions ("today", "yesterday", "this week", "last week").
async fn retrieve(
    pool: &SqlitePool,
    question: &str,
    history: &[ChatTurn],
) -> Result<Vec<MeetingDocument>, sqlx::Error> {
    // Follow-ups like "what about the deadline?" lean on the previous question's terms
    let mut query = question.to_string();
    if let Some(previous) = history.iter().rev().find(|t| t.role == "user") {
        query.push(' ');
        query.push_str(&previous.content);
    }
    let terms = search::terms(&query);

    let mut selected: Vec<MeetingDocument> = Vec::new();
    if let Some((from, to)) = date_range_in(question, Local::now()) {
        let ids: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM meetings WHERE created_at >= ? AND created_at < ? ORDER BY created_at DESC LIMIT ?",
        )
        .bind(from)
        .bind(to)
        .bind(MAX_CONTEXT_MEETINGS as i64)
        .fetch_all(pool)
        .await?;
        let ids: Vec<String> = ids.into_iter().map(|(id,)| id).collect();
        selected = search::load_documents(pool, &ids).await?;
    }

    let matches = search::load_matching_documents(pool, &terms, false, 40).await?;
    for (doc, hit) in search::rank(matches, &terms) {
        if selected.len() >= MAX_CONTEXT_MEETINGS {
            break;
        }
        if hit.score > 0.0 && !selected.iter().any(|d| d.id == doc.id) {
            selected.push(doc);
        }
    }

    // Nothing matched: fall back to the most recent meetings
    if selected.is_empty() {
        let ids: Vec<(String,)> =
            sqlx::query_as("SELECT id FROM meetings ORDER BY created_at DESC LIMIT 3")
                .fetch_all(pool)
                .await?;
        let ids: Vec<String> = ids.into_iter().map(|(id,)| id).collect();
        selected = search::load_documents(pool, &ids).await?;
    }
    Ok(selected)
}

fn meeting_context(doc: &MeetingDocument, question: &str) -> String {
    let mut text = format!("\n## {} — {}\n", doc.title, format_date(&doc.created_at.0));
    if let Some(notes) = doc.notes_markdown.as_deref().filter(|n| !n.trim().is_empty()) {
        text.push_str("### User's notes\n");
        text.push_str(&search::truncate_chars(notes, MAX_SUMMARY_CHARS / 2));
        text.push('\n');
    }
    let has_summary = match doc.summary_markdown.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(summary) => {
            text.push_str("### AI notes\n");
            text.push_str(&search::truncate_chars(summary, MAX_SUMMARY_CHARS));
            text.push('\n');
            true
        }
        None => false,
    };
    if let Some(transcript) = doc.transcript.as_deref().filter(|t| !t.trim().is_empty()) {
        // With AI notes present, only add transcript passages around the question's terms
        let budget = if has_summary { MAX_TRANSCRIPT_CHARS / 2 } else { MAX_TRANSCRIPT_CHARS };
        let excerpt = transcript_excerpt(transcript, &search::terms(question), budget);
        if !excerpt.is_empty() {
            text.push_str("### Transcript excerpts\n");
            text.push_str(&excerpt);
            text.push('\n');
        }
    }
    text
}

fn transcript_excerpt(transcript: &str, terms: &[String], budget: usize) -> String {
    let passages: Vec<String> = terms
        .iter()
        .filter(|t| transcript.to_lowercase().contains(t.as_str()))
        .map(|t| search::context_around(transcript, t, 400))
        .collect();
    if passages.is_empty() {
        search::truncate_chars(transcript, budget)
    } else {
        search::truncate_chars(&passages.join("\n"), budget)
    }
}

fn format_date(date: &DateTime<Utc>) -> String {
    date.with_timezone(&Local).format("%a %b %-d, %Y %-I:%M %p").to_string()
}

/// Recognizes simple relative dates; returns a UTC [from, to) range
fn date_range_in(question: &str, now: DateTime<Local>) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
    let q = question.to_lowercase();
    let day_start = |days_ago: i64| {
        let date = (now - Duration::days(days_ago)).date_naive();
        Local
            .from_local_datetime(&date.and_hms_opt(0, 0, 0)?)
            .earliest()
            .map(|d| d.with_timezone(&Utc))
    };
    let weekday = now.weekday().num_days_from_monday() as i64;
    let (from, to) = if q.contains("today") {
        (day_start(0)?, now.with_timezone(&Utc) + Duration::hours(1))
    } else if q.contains("yesterday") {
        (day_start(1)?, day_start(0)?)
    } else if q.contains("last week") {
        (day_start(weekday + 7)?, day_start(weekday)?)
    } else if q.contains("this week") {
        (day_start(weekday)?, now.with_timezone(&Utc) + Duration::hours(1))
    } else {
        return None;
    };
    Some((from, to))
}

/// Sends one prompt to the summary model configured in Settings
async fn complete(
    pool: &SqlitePool,
    app_data_dir: Option<PathBuf>,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    let setting = SettingsRepository::get_model_config(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No AI model is configured. Choose a summary model in Settings first.")?;
    let provider = LLMProvider::from_str(&setting.provider)?;
    let mut model_name = setting.model.clone();

    let mut api_key = String::new();
    let mut custom_endpoint = None;
    let (mut max_tokens, mut temperature, mut top_p) = (None, None, None);
    match provider {
        LLMProvider::Ollama | LLMProvider::BuiltInAI => {}
        LLMProvider::Segra => {
            api_key = crate::segra::require_api_key(pool).await?;
        }
        LLMProvider::CustomOpenAI => {
            let config = SettingsRepository::get_custom_openai_config(pool)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("Custom OpenAI provider selected but no configuration found")?;
            if !config.model.trim().is_empty() {
                model_name = config.model;
            }
            api_key = config.api_key.unwrap_or_default();
            custom_endpoint = Some(config.endpoint);
            max_tokens = config.max_tokens.map(|t| t as u32);
            temperature = config.temperature;
            top_p = config.top_p;
        }
        _ => {
            api_key = SettingsRepository::get_api_key(pool, &setting.provider)
                .await
                .map_err(|e| e.to_string())?
                .filter(|k| !k.is_empty())
                .ok_or_else(|| format!("API key not found for {}", setting.provider))?;
        }
    }
    let ollama_endpoint = if provider == LLMProvider::Ollama {
        setting.ollama_endpoint.clone()
    } else {
        None
    };

    // Cloudflare-fronted APIs (e.g. Nous Research) reject requests without a User-Agent
    let client = reqwest::Client::builder()
        .user_agent(concat!("Meetily/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    generate_summary(
        &client,
        &provider,
        &model_name,
        &api_key,
        system_prompt,
        user_prompt,
        ollama_endpoint.as_deref(),
        custom_endpoint.as_deref(),
        max_tokens,
        temperature,
        top_p,
        app_data_dir.as_ref(),
        None,
    )
    .await
    .map(|completion| completion.content.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_relative_dates() {
        // Wednesday
        let now = Local.with_ymd_and_hms(2026, 10, 7, 15, 0, 0).unwrap();
        let (from, to) = date_range_in("What did we decide yesterday?", now).unwrap();
        assert_eq!(from.with_timezone(&Local).date_naive().to_string(), "2026-10-06");
        assert_eq!(to.with_timezone(&Local).date_naive().to_string(), "2026-10-07");

        let (from, to) = date_range_in("summarize last week", now).unwrap();
        assert_eq!(from.with_timezone(&Local).date_naive().to_string(), "2026-09-28");
        assert_eq!(to.with_timezone(&Local).date_naive().to_string(), "2026-10-05");

        assert!(date_range_in("what's the budget?", now).is_none());
    }
}
