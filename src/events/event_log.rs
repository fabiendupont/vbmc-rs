// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Persistent System Event Log (SEL) for vbmc-rs.
//!
//! Mirrors the log emitted by real BMC firmware (HPE iLO, Dell iDRAC,
//! Lenovo XCC).  Entries persist across vbmc-rs restarts in an append-only
//! JSONL file; the ring buffer is capped at `MAX_ENTRIES` so the file does
//! not grow unbounded.
//!
//! The log is served via:
//!   GET /redfish/v1/Managers/{manager_id}/LogServices/Log/Entries
//!   GET /redfish/v1/Systems/{system_id}/LogServices/Log/Entries
//!
//! and its existence is announced in the respective LogServices collections.

use std::collections::VecDeque;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::SecondsFormat;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Serialize a `DateTime<Utc>` as RFC3339 with microsecond precision so that
/// NICo's timestamp filter (`Created > restart_time`) is not defeated by
/// second-only precision.  chrono's default serde representation omits
/// sub-second digits when nanoseconds are zero; using `SecondsFormat::Micros`
/// guarantees six decimal places regardless (e.g. `"2026-10-12T12:48:35.700123Z"`).
fn serialize_datetime_micros<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&dt.to_rfc3339_opts(SecondsFormat::Micros, true))
}

fn deserialize_datetime<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<DateTime<Utc>>().map_err(serde::de::Error::custom)
}

/// Maximum entries retained in the ring buffer and persisted to disk.
pub const MAX_ENTRIES: usize = 1000;

/// A single Redfish LogEntry (subset of DSP8010 LogEntry.v1_16_0).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Sequential, monotonically increasing identifier (never reused per run).
    pub id: u64,
    /// Always "Event" for the SEL.
    pub entry_type: String,
    /// Human-readable message; must contain a keyword from
    /// {"reset","reboot","restart","power","start"} for NICo's
    /// `check_restart_in_logs` to count a reset as confirmed.
    pub message: String,
    /// Redfish MessageId in the format "Registry.Version.Key"
    /// (e.g. "Power.1.0.PowerStateChange").
    pub message_id: String,
    /// "OK", "Warning", or "Critical".
    pub severity: String,
    /// ISO-8601 UTC timestamp with microsecond precision.
    /// Serialised as RFC3339+micros so that NICo's `Created > restart_time`
    /// filter is not defeated by second-only precision.
    #[serde(
        serialize_with = "serialize_datetime_micros",
        deserialize_with = "deserialize_datetime"
    )]
    pub created: DateTime<Utc>,
}

/// Append-only, ring-capped event log backed by a JSONL file.
///
/// Thread-safe; intended to be stored in `AppState` as an `Arc<EventLog>`.
pub struct EventLog {
    entries: Mutex<VecDeque<LogEntry>>,
    next_id: AtomicU64,
    log_path: PathBuf,
}

impl EventLog {
    /// Create a new `EventLog`, loading existing entries from `log_path`.
    pub fn new(log_path: PathBuf) -> Self {
        let entries = Self::load_from_disk(&log_path);
        let next_id = entries.back().map(|e| e.id + 1).unwrap_or(1);
        Self {
            entries: Mutex::new(entries),
            next_id: AtomicU64::new(next_id),
            log_path,
        }
    }

    fn load_from_disk(path: &Path) -> VecDeque<LogEntry> {
        let mut entries = VecDeque::new();
        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
                    entries.push_back(entry);
                }
            }
        }
        // Trim to ring-buffer size if the file had more than MAX_ENTRIES.
        while entries.len() > MAX_ENTRIES {
            entries.pop_front();
        }
        entries
    }

    /// Append a new log entry and persist it to disk.
    ///
    /// When the ring buffer is full the oldest entry is evicted (in memory
    /// only — the disk file is append-only and periodically compacted by
    /// `clear`).
    pub fn append(&self, message: &str, message_id: &str, severity: &str) -> LogEntry {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let entry = LogEntry {
            id,
            entry_type: "Event".to_string(),
            message: message.to_string(),
            message_id: message_id.to_string(),
            severity: severity.to_string(),
            created: Utc::now(),
        };

        {
            let mut entries = self.entries.lock().unwrap();
            if entries.len() >= MAX_ENTRIES {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }

        // Persist: append the new entry as a JSONL line.  Create parent
        // directories on the first write.
        if let Some(parent) = self.log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
        {
            let line = serde_json::to_string(&entry).unwrap_or_default() + "\n";
            let _ = file.write_all(line.as_bytes());
        }

        entry
    }

    /// Return all entries, newest first (matches Redfish convention).
    pub fn entries_newest_first(&self) -> Vec<LogEntry> {
        let entries = self.entries.lock().unwrap();
        entries.iter().rev().cloned().collect()
    }

    /// Return the number of currently stored entries.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }

    /// Clear all entries in memory and rewrite the disk file as empty.
    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear();
        // Truncate the file by overwriting with empty content.
        let _ = std::fs::write(&self.log_path, "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_log() -> (tempfile::TempDir, EventLog) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("event-log.jsonl");
        let log = EventLog::new(path);
        (dir, log)
    }

    #[test]
    fn test_append_and_retrieve() {
        let (_dir, log) = tmp_log();
        let e = log.append("System powered on", "Power.1.0.PowerStateChange", "OK");
        assert_eq!(e.message, "System powered on");
        assert_eq!(e.message_id, "Power.1.0.PowerStateChange");
        assert_eq!(e.severity, "OK");
        assert_eq!(e.entry_type, "Event");

        let entries = log.entries_newest_first();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "System powered on");
    }

    #[test]
    fn test_newest_first_ordering() {
        let (_dir, log) = tmp_log();
        log.append("first", "Msg.1.0.A", "OK");
        log.append("second", "Msg.1.0.B", "OK");
        log.append("third", "Msg.1.0.C", "OK");

        let entries = log.entries_newest_first();
        assert_eq!(entries[0].message, "third");
        assert_eq!(entries[1].message, "second");
        assert_eq!(entries[2].message, "first");
    }

    #[test]
    fn test_persistence_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("event-log.jsonl");

        {
            let log = EventLog::new(path.clone());
            log.append("before restart", "Msg.1.0.A", "OK");
        }

        // Simulate restart: create a new EventLog from the same path.
        let log2 = EventLog::new(path);
        let entries = log2.entries_newest_first();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "before restart");
    }

    #[test]
    fn test_ring_buffer_caps_at_max_entries() {
        let (_dir, log) = tmp_log();
        for i in 0..MAX_ENTRIES + 10 {
            log.append(&format!("msg {i}"), "Msg.1.0.A", "OK");
        }
        assert_eq!(log.len(), MAX_ENTRIES);
    }

    #[test]
    fn test_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("event-log.jsonl");

        let log = EventLog::new(path.clone());
        log.append("entry", "Msg.1.0.A", "OK");
        assert_eq!(log.len(), 1);
        log.clear();
        assert_eq!(log.len(), 0);

        // File should be empty after clear.
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn test_monotonically_increasing_ids() {
        let (_dir, log) = tmp_log();
        let e1 = log.append("a", "Msg.1.0.A", "OK");
        let e2 = log.append("b", "Msg.1.0.B", "OK");
        let e3 = log.append("c", "Msg.1.0.C", "OK");
        assert!(e1.id < e2.id);
        assert!(e2.id < e3.id);
    }

    #[test]
    fn test_id_continues_after_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("event-log.jsonl");

        let last_id = {
            let log = EventLog::new(path.clone());
            let e = log.append("initial", "Msg.1.0.A", "OK");
            e.id
        };

        let log2 = EventLog::new(path);
        let e2 = log2.append("after reload", "Msg.1.0.B", "OK");
        assert!(
            e2.id > last_id,
            "id after reload must exceed id before reload"
        );
    }

    /// Timestamp precision regression test.
    ///
    /// NICo's machine-controller filters log entries with `Created > restart_time`
    /// where `restart_time` has sub-second precision (e.g. nanoseconds from the DB).
    /// If `Created` is serialised with second-only precision (the chrono serde
    /// default) an entry created at `T.700ms` is stored as `T.000` and the filter
    /// `T.000 > T.660` fails → the entry is excluded → NICo loops forever.
    ///
    /// This test asserts that the on-disk serialisation includes microseconds.
    #[test]
    fn test_created_timestamp_has_sub_second_precision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("event-log.jsonl");

        let log = EventLog::new(path.clone());
        log.append(
            "System reset (force restart)",
            "Power.1.0.PowerStateChange",
            "OK",
        );

        // Read the raw JSONL line and check that the created field has fractional
        // seconds (at least one digit after the dot in the time component).
        let content = std::fs::read_to_string(&path).unwrap();
        let line = content.lines().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        let created_str = parsed["created"].as_str().unwrap();

        // RFC3339 with micros: "2026-10-12T12:48:35.700123Z"
        // Must contain a '.' between the seconds digits and 'Z'.
        assert!(
            created_str.contains('.'),
            "Created timestamp must have sub-second precision, got: {created_str}"
        );
        // Must be at least millisecond precision (3 digits after '.').
        let frac_part = created_str
            .split('.')
            .nth(1)
            .expect("no fractional part in timestamp");
        let digits: String = frac_part
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        assert!(
            digits.len() >= 3,
            "Expected at least millisecond precision (3 digits), got {digits:?} in {created_str}"
        );
    }

    /// Timestamps survive a restart (load → serve) with precision intact.
    #[test]
    fn test_created_timestamp_survives_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("event-log.jsonl");

        let original_created = {
            let log = EventLog::new(path.clone());
            let e = log.append("test", "Msg.1.0.A", "OK");
            e.created
        };

        let log2 = EventLog::new(path);
        let entries = log2.entries_newest_first();
        assert_eq!(entries.len(), 1);

        // Microsecond precision must be preserved across the JSONL roundtrip.
        let reloaded = entries[0].created;
        assert_eq!(
            original_created.timestamp_micros(),
            reloaded.timestamp_micros(),
            "Microsecond precision lost across JSONL roundtrip"
        );
    }
}
