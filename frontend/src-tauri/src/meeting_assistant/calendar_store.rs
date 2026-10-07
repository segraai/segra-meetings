use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CalendarEvent {
    pub id: String,
    pub calendar_id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub is_all_day: bool,
    pub meeting_url: Option<String>,
    pub html_link: Option<String>,
    pub attendees_json: Option<String>,
    pub reminded_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

pub struct CalendarEventsRepository;

impl CalendarEventsRepository {
    /// Replaces all events of `calendar_id` that start inside [from, to) with `events`,
    /// keeping `reminded_at` for events that already existed.
    pub async fn replace_range(
        pool: &SqlitePool,
        calendar_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        events: &[CalendarEvent],
    ) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;

        let ids: Vec<&str> = events.iter().map(|e| e.id.as_str()).collect();
        let existing: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM calendar_events WHERE calendar_id = ? AND start_time >= ? AND start_time < ?",
        )
        .bind(calendar_id)
        .bind(from)
        .bind(to)
        .fetch_all(&mut *tx)
        .await?;
        for (id,) in existing {
            if !ids.contains(&id.as_str()) {
                sqlx::query("DELETE FROM calendar_events WHERE id = ?")
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;
            }
        }

        for event in events {
            sqlx::query(
                r#"
                INSERT INTO calendar_events
                    (id, calendar_id, title, description, start_time, end_time, is_all_day,
                     meeting_url, html_link, attendees_json, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    title = excluded.title,
                    description = excluded.description,
                    start_time = excluded.start_time,
                    end_time = excluded.end_time,
                    is_all_day = excluded.is_all_day,
                    meeting_url = excluded.meeting_url,
                    html_link = excluded.html_link,
                    attendees_json = excluded.attendees_json,
                    updated_at = excluded.updated_at,
                    -- A rescheduled event should remind again
                    reminded_at = CASE WHEN calendar_events.start_time = excluded.start_time
                                       THEN calendar_events.reminded_at ELSE NULL END
                "#,
            )
            .bind(&event.id)
            .bind(&event.calendar_id)
            .bind(&event.title)
            .bind(&event.description)
            .bind(event.start_time)
            .bind(event.end_time)
            .bind(event.is_all_day)
            .bind(&event.meeting_url)
            .bind(&event.html_link)
            .bind(&event.attendees_json)
            .bind(event.updated_at)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await
    }

    pub async fn list_between(
        pool: &SqlitePool,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<CalendarEvent>, sqlx::Error> {
        sqlx::query_as::<_, CalendarEvent>(
            "SELECT * FROM calendar_events WHERE end_time > ? AND start_time < ? ORDER BY start_time",
        )
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await
    }

    /// Timed events starting within the next `lead` window (or that started in the last
    /// two minutes) that haven't been reminded yet.
    pub async fn due_for_reminder(
        pool: &SqlitePool,
        now: DateTime<Utc>,
        lead: chrono::Duration,
    ) -> Result<Vec<CalendarEvent>, sqlx::Error> {
        sqlx::query_as::<_, CalendarEvent>(
            r#"
            SELECT * FROM calendar_events
            WHERE is_all_day = 0 AND reminded_at IS NULL
              AND start_time <= ? AND start_time >= ?
            ORDER BY start_time
            "#,
        )
        .bind(now + lead)
        .bind(now - chrono::Duration::minutes(2))
        .fetch_all(pool)
        .await
    }

    pub async fn mark_reminded(
        pool: &SqlitePool,
        id: &str,
        at: DateTime<Utc>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE calendar_events SET reminded_at = ? WHERE id = ?")
            .bind(at)
            .bind(id)
            .execute(pool)
            .await
            .map(|_| ())
    }

    /// The timed event happening right now (starting up to 10 minutes from now), if any
    pub async fn current_event(
        pool: &SqlitePool,
        now: DateTime<Utc>,
    ) -> Result<Option<CalendarEvent>, sqlx::Error> {
        sqlx::query_as::<_, CalendarEvent>(
            r#"
            SELECT * FROM calendar_events
            WHERE is_all_day = 0 AND start_time <= ? AND end_time > ?
            ORDER BY (meeting_url IS NULL), start_time DESC
            LIMIT 1
            "#,
        )
        .bind(now + chrono::Duration::minutes(10))
        .bind(now)
        .fetch_optional(pool)
        .await
    }

    pub async fn clear(pool: &SqlitePool) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM calendar_events")
            .execute(pool)
            .await
            .map(|_| ())
    }
}
