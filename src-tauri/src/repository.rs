use std::path::Path;

use rusqlite::{params, Connection, Transaction};

use thiserror::Error;

use crate::domain::{EventKind, Occurrence, WorkdayStatus, WorkdaySummary};

pub const DEPARTURE_GRACE_MS: i64 = 30 * 60 * 1_000;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Owns the SQLite connection and maintains the workday projection transactionally.
///
/// Invariant: an event append and its projection update either both commit or both roll back.
/// The UI never writes to this database directly.
pub struct Repository {
    connection: Connection,
}

impl Repository {
    pub fn open(path: &Path) -> Result<Self, RepositoryError> {
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;

             CREATE TABLE IF NOT EXISTS events (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               occurred_at_utc_ms INTEGER NOT NULL,
               local_date TEXT NOT NULL,
               local_offset_seconds INTEGER NOT NULL,
               event_type TEXT NOT NULL
             );

             CREATE INDEX IF NOT EXISTS idx_events_local_date
               ON events(local_date, occurred_at_utc_ms);

             CREATE TABLE IF NOT EXISTS workdays (
               local_date TEXT PRIMARY KEY,
               arrival_utc_ms INTEGER NOT NULL,
               departure_utc_ms INTEGER,
               pending_departure_utc_ms INTEGER,
               CHECK(departure_utc_ms IS NULL OR departure_utc_ms >= arrival_utc_ms),
               CHECK(pending_departure_utc_ms IS NULL OR pending_departure_utc_ms >= arrival_utc_ms)
             );",
        )?;

        Ok(Self { connection })
    }

    pub fn record_event(
        &mut self,
        kind: EventKind,
        occurrence: &Occurrence,
    ) -> Result<(), RepositoryError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT INTO events (
               occurred_at_utc_ms, local_date, local_offset_seconds, event_type
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                occurrence.utc_ms,
                occurrence.local_date,
                occurrence.offset_seconds,
                kind.as_str()
            ],
        )?;

        if kind.is_present() {
            Self::apply_present(&transaction, occurrence)?;
        } else if kind.is_away() {
            Self::apply_away(&transaction, occurrence)?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn advance_projection(&mut self, now: &Occurrence) -> Result<bool, RepositoryError> {
        let transaction = self.connection.transaction()?;
        let mut changed =
            Self::finalize_older_days(&transaction, &now.local_date, now.utc_ms)?;
        changed += transaction.execute(
            "UPDATE workdays
             SET departure_utc_ms = pending_departure_utc_ms
             WHERE local_date = ?1
               AND pending_departure_utc_ms IS NOT NULL
               AND departure_utc_ms IS NULL
               AND ?2 - pending_departure_utc_ms >= ?3",
            params![now.local_date, now.utc_ms, DEPARTURE_GRACE_MS],
        )?;
        transaction.commit()?;
        Ok(changed > 0)
    }

    pub fn list_workdays(&self, now_utc_ms: i64) -> Result<Vec<WorkdaySummary>, RepositoryError> {
        let mut statement = self.connection.prepare(
            "SELECT local_date, arrival_utc_ms, departure_utc_ms, pending_departure_utc_ms
             FROM workdays
             ORDER BY local_date DESC",
        )?;

        let rows = statement.query_map([], |row| {
            let arrival_ms: i64 = row.get(1)?;
            let departure_ms: Option<i64> = row.get(2)?;
            let pending_departure_ms: Option<i64> = row.get(3)?;
            let status = if departure_ms.is_some() {
                WorkdayStatus::Complete
            } else if pending_departure_ms.is_some() {
                WorkdayStatus::DeparturePending
            } else {
                WorkdayStatus::Active
            };
            let end_ms = departure_ms.unwrap_or(now_utc_ms).max(arrival_ms);

            Ok(WorkdaySummary {
                date: row.get(0)?,
                arrival_ms,
                departure_ms,
                pending_departure_ms,
                duration_seconds: (end_ms - arrival_ms) / 1_000,
                status,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn event_count(&self) -> Result<i64, RepositoryError> {
        self.connection
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .map_err(Into::into)
    }

    fn apply_present(
        transaction: &Transaction<'_>,
        occurrence: &Occurrence,
    ) -> Result<(), rusqlite::Error> {
        Self::finalize_older_days(transaction, &occurrence.local_date, occurrence.utc_ms)?;
        transaction.execute(
            "INSERT INTO workdays (local_date, arrival_utc_ms)
             VALUES (?1, ?2)
             ON CONFLICT(local_date) DO UPDATE SET
               departure_utc_ms = NULL,
               pending_departure_utc_ms = NULL",
            params![occurrence.local_date, occurrence.utc_ms],
        )?;
        Ok(())
    }

    fn apply_away(
        transaction: &Transaction<'_>,
        occurrence: &Occurrence,
    ) -> Result<(), rusqlite::Error> {
        transaction.execute(
            "UPDATE workdays
             SET pending_departure_utc_ms = ?2,
                 departure_utc_ms = NULL
             WHERE local_date = ?1",
            params![occurrence.local_date, occurrence.utc_ms],
        )?;
        Ok(())
    }

    fn finalize_older_days(
        transaction: &Transaction<'_>,
        current_local_date: &str,
        now_utc_ms: i64,
    ) -> Result<usize, rusqlite::Error> {
        transaction.execute(
            "UPDATE workdays
             SET departure_utc_ms = pending_departure_utc_ms,
                 pending_departure_utc_ms = NULL
             WHERE local_date < ?1
               AND pending_departure_utc_ms IS NOT NULL
               AND ?2 - pending_departure_utc_ms >= ?3",
            params![current_local_date, now_utc_ms, DEPARTURE_GRACE_MS],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(date: &str, utc_ms: i64) -> Occurrence {
        Occurrence {
            utc_ms,
            local_date: date.to_owned(),
            offset_seconds: 7 * 3600,
        }
    }

    fn repository() -> Repository {
        Repository::open(Path::new(":memory:")).expect("in-memory database")
    }

    #[test]
    fn repeated_present_events_preserve_the_first_arrival() {
        let mut repository = repository();
        repository
            .record_event(EventKind::AppStarted, &at("2026-08-15", 1_000))
            .unwrap();
        repository
            .record_event(EventKind::SessionUnlock, &at("2026-08-15", 5_000))
            .unwrap();

        let rows = repository.list_workdays(6_000).unwrap();
        assert_eq!(rows[0].arrival_ms, 1_000);
        assert_eq!(repository.event_count().unwrap(), 2);
    }

    #[test]
    fn lock_is_final_only_after_the_grace_period() {
        let mut repository = repository();
        repository
            .record_event(EventKind::AppStarted, &at("2026-08-15", 1_000))
            .unwrap();
        repository
            .record_event(EventKind::SessionLock, &at("2026-08-15", 10_000))
            .unwrap();

        repository
            .advance_projection(&at("2026-08-15", 10_000 + DEPARTURE_GRACE_MS - 1))
            .unwrap();
        assert!(repository.list_workdays(20_000).unwrap()[0].departure_ms.is_none());

        repository
            .advance_projection(&at("2026-08-15", 10_000 + DEPARTURE_GRACE_MS))
            .unwrap();
        assert_eq!(repository.list_workdays(20_000).unwrap()[0].departure_ms, Some(10_000));
    }

    #[test]
    fn unlock_reopens_a_completed_day() {
        let mut repository = repository();
        repository
            .record_event(EventKind::AppStarted, &at("2026-08-15", 1_000))
            .unwrap();
        repository
            .record_event(EventKind::SessionLock, &at("2026-08-15", 10_000))
            .unwrap();
        repository
            .advance_projection(&at("2026-08-15", 10_000 + DEPARTURE_GRACE_MS))
            .unwrap();

        repository
            .record_event(EventKind::SessionUnlock, &at("2026-08-15", 20_000))
            .unwrap();

        let row = &repository.list_workdays(21_000).unwrap()[0];
        assert!(row.departure_ms.is_none());
        assert!(row.pending_departure_ms.is_none());
    }

    #[test]
    fn next_day_start_finalizes_shutdown_candidate() {
        let mut repository = repository();
        repository
            .record_event(EventKind::AppStarted, &at("2026-08-15", 1_000))
            .unwrap();
        repository
            .record_event(EventKind::SystemShutdown, &at("2026-08-15", 10_000))
            .unwrap();

        repository
            .record_event(EventKind::AppStarted, &at("2026-08-16", 2_000_000))
            .unwrap();

        let rows = repository.list_workdays(2_001_000).unwrap();
        assert_eq!(rows[1].departure_ms, Some(10_000));
        assert_eq!(rows[0].date, "2026-08-16");
    }

    #[test]
    fn midnight_does_not_bypass_the_grace_period() {
        let mut repository = repository();
        repository
            .record_event(EventKind::AppStarted, &at("2026-08-15", 1_000))
            .unwrap();
        repository
            .record_event(EventKind::SessionLock, &at("2026-08-15", 10_000))
            .unwrap();

        repository
            .advance_projection(&at("2026-08-16", 10_000 + DEPARTURE_GRACE_MS - 1))
            .unwrap();

        let previous_day = &repository.list_workdays(2_000_000).unwrap()[0];
        assert!(previous_day.departure_ms.is_none());
        assert_eq!(previous_day.pending_departure_ms, Some(10_000));
    }
}
