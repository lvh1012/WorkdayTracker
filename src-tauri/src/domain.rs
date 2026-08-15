use chrono::{DateTime, Local};
use serde::Serialize;

/// A raw fact captured from Windows or the application lifecycle.
///
/// These values are intentionally append-only in SQLite. Daily summaries are projections
/// and can be rebuilt later if the heuristic changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    AppStarted,
    SessionLogon,
    SessionLogoff,
    SessionLock,
    SessionUnlock,
    SystemSuspend,
    SystemResume,
    SystemShutdown,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppStarted => "app_started",
            Self::SessionLogon => "session_logon",
            Self::SessionLogoff => "session_logoff",
            Self::SessionLock => "session_lock",
            Self::SessionUnlock => "session_unlock",
            Self::SystemSuspend => "system_suspend",
            Self::SystemResume => "system_resume",
            Self::SystemShutdown => "system_shutdown",
        }
    }

    pub fn is_present(self) -> bool {
        matches!(
            self,
            Self::AppStarted | Self::SessionLogon | Self::SessionUnlock | Self::SystemResume
        )
    }

    pub fn is_away(self) -> bool {
        matches!(
            self,
            Self::SessionLogoff | Self::SessionLock | Self::SystemSuspend | Self::SystemShutdown
        )
    }
}

/// Captures UTC and local-day information at the same instant.
///
/// UTC milliseconds make duration calculations stable if the timezone or DST offset changes.
/// `local_date` preserves the user's calendar day at the time the event occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence {
    pub utc_ms: i64,
    pub local_date: String,
    pub offset_seconds: i32,
}

impl Occurrence {
    pub fn now() -> Self {
        Self::from_local(Local::now())
    }

    pub fn from_local(value: DateTime<Local>) -> Self {
        Self {
            utc_ms: value.timestamp_millis(),
            local_date: value.format("%Y-%m-%d").to_string(),
            offset_seconds: value.offset().local_minus_utc(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkdayStatus {
    Active,
    DeparturePending,
    Complete,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkdaySummary {
    pub date: String,
    pub arrival_ms: i64,
    pub departure_ms: Option<i64>,
    pub pending_departure_ms: Option<i64>,
    pub duration_seconds: i64,
    pub status: WorkdayStatus,
}

#[cfg(test)]
mod tests {
    use super::EventKind;

    #[test]
    fn event_categories_are_disjoint() {
        let kinds = [
            EventKind::AppStarted,
            EventKind::SessionLogon,
            EventKind::SessionLogoff,
            EventKind::SessionLock,
            EventKind::SessionUnlock,
            EventKind::SystemSuspend,
            EventKind::SystemResume,
            EventKind::SystemShutdown,
        ];

        assert!(kinds.iter().all(|kind| kind.is_present() ^ kind.is_away()));
    }
}

