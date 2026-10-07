//! Search across meeting titles, AI notes, the user's own notes and transcripts.

use crate::database::models::DateTimeUtc;
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct MeetingOverview {
    pub id: String,
    pub title: String,
    pub created_at: DateTimeUtc,
    /// Recording length in seconds, from the last transcript segment
    pub duration_seconds: Option<f64>,
    /// First lines of the AI notes, if generated
    pub summary_preview: Option<String>,
}

/// Everything searchable about one meeting
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MeetingDocument {
    pub id: String,
    pub title: String,
    pub created_at: DateTimeUtc,
    pub summary_markdown: Option<String>,
    pub notes_markdown: Option<String>,
    pub transcript: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingSearchHit {
    pub id: String,
    pub title: String,
    pub created_at: DateTimeUtc,
    /// Where the snippet came from: "title" | "notes" | "summary" | "transcript"
    pub matched_in: String,
    pub snippet: String,
    #[serde(skip)]
    pub score: f64,
}

const SUMMARY_MARKDOWN_SQL: &str = "(SELECT json_extract(sp.result, '$.markdown') FROM summary_processes sp \
     WHERE sp.meeting_id = m.id AND json_valid(sp.result))";

const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "did", "do", "does", "for", "from", "had",
    "has", "have", "how", "i", "in", "is", "it", "me", "my", "of", "on", "or", "our", "so",
    "that", "the", "their", "there", "they", "this", "to", "was", "we", "were", "what", "when",
    "where", "which", "who", "why", "will", "with", "you", "about", "any", "can", "could",
    "would", "should", "tell", "meeting", "meetings", "said", "say", "talk", "talked",
];

/// Splits a query into lowercase search terms, dropping stop words and punctuation
pub fn terms(query: &str) -> Vec<String> {
    let mut terms: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-')
        .map(|t| t.trim_matches(|c: char| c == '\'' || c == '-').to_string())
        .filter(|t| t.chars().count() >= 2 && !STOP_WORDS.contains(&t.as_str()))
        .collect();
    terms.dedup();
    terms
}

fn like_pattern(term: &str) -> String {
    let escaped = term.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
    format!("%{}%", escaped)
}

pub async fn list_overviews(pool: &SqlitePool) -> Result<Vec<MeetingOverview>, sqlx::Error> {
    let sql = format!(
        "SELECT m.id, m.title, m.created_at, \
           (SELECT MAX(t.audio_end_time) FROM transcripts t WHERE t.meeting_id = m.id) AS duration_seconds, \
           substr({}, 1, 400) AS summary_preview \
         FROM meetings m ORDER BY m.created_at DESC",
        SUMMARY_MARKDOWN_SQL
    );
    sqlx::query_as::<_, MeetingOverview>(&sql).fetch_all(pool).await
}

/// Loads full documents for the meetings matching `terms`. With `match_all`, every term
/// must appear somewhere in the meeting; otherwise any term is enough.
pub async fn load_matching_documents(
    pool: &SqlitePool,
    terms: &[String],
    match_all: bool,
    limit: i64,
) -> Result<Vec<MeetingDocument>, sqlx::Error> {
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let term_clause = format!(
        "(LOWER(m.title) LIKE ? ESCAPE '\\' \
          OR LOWER(COALESCE({summary}, '')) LIKE ? ESCAPE '\\' \
          OR EXISTS (SELECT 1 FROM meeting_notes n WHERE n.meeting_id = m.id AND LOWER(n.notes_markdown) LIKE ? ESCAPE '\\') \
          OR EXISTS (SELECT 1 FROM transcripts t WHERE t.meeting_id = m.id AND LOWER(t.transcript) LIKE ? ESCAPE '\\'))",
        summary = SUMMARY_MARKDOWN_SQL
    );
    let joiner = if match_all { " AND " } else { " OR " };
    let where_clause = vec![term_clause; terms.len()].join(joiner);
    let sql = format!(
        "SELECT m.id FROM meetings m WHERE {} ORDER BY m.created_at DESC LIMIT ?",
        where_clause
    );

    let mut query = sqlx::query_as::<_, (String,)>(&sql);
    for term in terms {
        let pattern = like_pattern(term);
        for _ in 0..4 {
            query = query.bind(pattern.clone());
        }
    }
    let ids: Vec<String> = query
        .bind(limit)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|(id,)| id)
        .collect();

    load_documents(pool, &ids).await
}

pub async fn load_documents(
    pool: &SqlitePool,
    ids: &[String],
) -> Result<Vec<MeetingDocument>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(", ");
    let sql = format!(
        "SELECT m.id, m.title, m.created_at, \
           {summary} AS summary_markdown, \
           (SELECT n.notes_markdown FROM meeting_notes n WHERE n.meeting_id = m.id) AS notes_markdown, \
           (SELECT group_concat(transcript, ' ') FROM \
              (SELECT t.transcript FROM transcripts t WHERE t.meeting_id = m.id \
               ORDER BY COALESCE(t.audio_start_time, 0), t.timestamp)) AS transcript \
         FROM meetings m WHERE m.id IN ({placeholders}) ORDER BY m.created_at DESC",
        summary = SUMMARY_MARKDOWN_SQL,
        placeholders = placeholders
    );
    let mut query = sqlx::query_as::<_, MeetingDocument>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    query.fetch_all(pool).await
}

/// Ranks documents by how often and where the terms appear, best first.
pub fn rank(documents: Vec<MeetingDocument>, terms: &[String]) -> Vec<(MeetingDocument, MeetingSearchHit)> {
    let mut ranked: Vec<(MeetingDocument, MeetingSearchHit)> = documents
        .into_iter()
        .map(|doc| {
            let hit = score(&doc, terms);
            (doc, hit)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.score.partial_cmp(&a.1.score).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

fn score(doc: &MeetingDocument, terms: &[String]) -> MeetingSearchHit {
    let fields: [(&str, &str, f64); 4] = [
        ("title", doc.title.as_str(), 8.0),
        ("notes", doc.notes_markdown.as_deref().unwrap_or(""), 3.0),
        ("summary", doc.summary_markdown.as_deref().unwrap_or(""), 3.0),
        ("transcript", doc.transcript.as_deref().unwrap_or(""), 1.0),
    ];

    let mut score = 0.0;
    let mut best: Option<(&str, &str, &str, f64)> = None; // (field, text, term, weight)
    for (name, text, weight) in fields {
        let lower = text.to_lowercase();
        for term in terms {
            let count = lower.matches(term.as_str()).count();
            if count == 0 {
                continue;
            }
            // Diminishing returns for repeated mentions
            let contribution = weight * (1.0 + (count as f64).ln());
            score += contribution;
            if best.map_or(true, |b| contribution > b.3) {
                best = Some((name, text, term.as_str(), contribution));
            }
        }
    }

    let (matched_in, snippet) = match best {
        // A title match isn't a useful snippet; show the start of the notes instead
        Some(("title", _, _, _)) | None => (
            "title".to_string(),
            preview(doc.summary_markdown.as_deref().or(doc.notes_markdown.as_deref()).unwrap_or("")),
        ),
        Some((field, text, term, _)) => (field.to_string(), context_around(text, term, 90)),
    };

    MeetingSearchHit {
        id: doc.id.clone(),
        title: doc.title.clone(),
        created_at: doc.created_at.clone(),
        matched_in,
        snippet,
        score,
    }
}

fn preview(text: &str) -> String {
    let plain: String = text
        .lines()
        .map(|l| l.trim_start_matches(|c: char| c == '#' || c == '-' || c == '*' || c == ' '))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    truncate_chars(&plain, 180)
}

/// Text around the first case-insensitive occurrence of `term`, on char boundaries
pub fn context_around(text: &str, term: &str, radius: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let lower: Vec<char> = text.to_lowercase().chars().collect();
    // Lowercasing can change char counts for some scripts; fall back to the start
    let position = if lower.len() == chars.len() {
        let term: Vec<char> = term.chars().collect();
        lower
            .windows(term.len().max(1))
            .position(|w| w == term.as_slice())
            .unwrap_or(0)
    } else {
        0
    };
    let start = position.saturating_sub(radius);
    let end = (position + term.chars().count() + radius).min(chars.len());
    let mut snippet: String = chars[start..end].iter().collect();
    snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
    if start > 0 {
        snippet = format!("…{}", snippet);
    }
    if end < chars.len() {
        snippet.push('…');
    }
    snippet
}

pub fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(title: &str, summary: Option<&str>, transcript: Option<&str>) -> MeetingDocument {
        MeetingDocument {
            id: title.to_string(),
            title: title.to_string(),
            created_at: DateTimeUtc(chrono::Utc::now()),
            summary_markdown: summary.map(str::to_string),
            notes_markdown: None,
            transcript: transcript.map(str::to_string),
        }
    }

    #[test]
    fn extracts_terms_without_stop_words() {
        assert_eq!(terms("What did we decide about the Q3 budget?"), vec!["decide", "q3", "budget"]);
    }

    #[test]
    fn ranks_title_and_summary_matches_above_transcript_mentions() {
        let docs = vec![
            doc("Standup", None, Some("budget budget mentioned in passing")),
            doc("Budget review", Some("## Decisions\n- Approve budget"), None),
        ];
        let ranked = rank(docs, &terms("budget"));
        assert_eq!(ranked[0].0.title, "Budget review");
        assert_eq!(ranked[1].1.matched_in, "transcript");
    }

    #[test]
    fn context_snippet_is_char_safe() {
        let text = "ünïcödé text before the keyword and after it";
        let snippet = context_around(text, "keyword", 5);
        assert_eq!(snippet, "…the keyword and…");
    }
}
