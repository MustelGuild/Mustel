use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tokio_postgres::Row;

use crate::error::{MustelError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionLogEntry {
    pub timestamp: String,
    pub server_name: String,
    pub database_name: String,
    pub query_source: String,
    pub rows_affected: u64,
    pub duration_ms: u128,
    pub output_file: String,
    pub status: String,
    pub error_message: Option<String>,
}

pub struct CsvExporter;

impl CsvExporter {
    /// Streams query rows directly into a CSV file at output_path.
    pub fn export_rows(
        rows: &[Row],
        output_path: &Path,
    ) -> Result<u64> {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = File::create(output_path)?;
        let buf_writer = BufWriter::new(file);
        let mut csv_writer = csv::WriterBuilder::new().from_writer(buf_writer);

        if rows.is_empty() {
            csv_writer.flush()?;
            return Ok(0);
        }

        // Write header row
        let columns = rows[0].columns();
        let headers: Vec<&str> = columns.iter().map(|c| c.name()).collect();
        csv_writer.write_record(&headers)
            .map_err(|e| MustelError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

        let mut record_count = 0u64;

        // Stream each row
        for row in rows {
            let mut record = Vec::with_capacity(columns.len());

            for (idx, col) in columns.iter().enumerate() {
                let cell_str = Self::format_cell(row, idx, col.type_().name());
                record.push(cell_str);
            }

            csv_writer.write_record(&record)
                .map_err(|e| MustelError::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            record_count += 1;
        }

        csv_writer.flush()?;
        Ok(record_count)
    }

    /// Formats a single PostgreSQL row cell into its String representation.
    fn format_cell(row: &Row, idx: usize, _type_name: &str) -> String {
        // Try common types
        if let Ok(val) = row.try_get::<_, Option<&str>>(idx) {
            return val.unwrap_or("").to_string();
        }

        if let Ok(val) = row.try_get::<_, Option<i64>>(idx) {
            return val.map(|v| v.to_string()).unwrap_or_default();
        }

        if let Ok(val) = row.try_get::<_, Option<i32>>(idx) {
            return val.map(|v| v.to_string()).unwrap_or_default();
        }

        if let Ok(val) = row.try_get::<_, Option<bool>>(idx) {
            return val.map(|v| v.to_string()).unwrap_or_default();
        }

        if let Ok(val) = row.try_get::<_, Option<f64>>(idx) {
            return val.map(|v| v.to_string()).unwrap_or_default();
        }

        if let Ok(val) = row.try_get::<_, Option<chrono::NaiveDateTime>>(idx) {
            return val.map(|v| v.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();
        }

        if let Ok(val) = row.try_get::<_, Option<serde_json::Value>>(idx) {
            return val.map(|v| v.to_string()).unwrap_or_default();
        }

        if let Ok(val) = row.try_get::<_, Option<uuid::Uuid>>(idx) {
            return val.map(|v| v.to_string()).unwrap_or_default();
        }

        if let Ok(val) = row.try_get::<_, Option<Vec<u8>>>(idx) {
            return val.map(|bytes| format!("\\x{}", hex::encode(bytes))).unwrap_or_default();
        }

        // Fallback placeholder for unhandled complex types
        "<unsupported_type>".to_string()
    }

    /// Appends execution summary entry to execution_log.json
    pub fn log_execution(output_dir: &Path, log_entry: &ExecutionLogEntry) -> Result<()> {
        let log_file_path = output_dir.join("execution_log.json");
        let mut entries: Vec<ExecutionLogEntry> = if log_file_path.exists() {
            let content = fs::read_to_string(&log_file_path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        entries.push(log_entry.clone());
        let json = serde_json::to_string_pretty(&entries)
            .map_err(|e| MustelError::Json(format!("Failed to serialize execution log: {}", e)))?;

        fs::write(log_file_path, json)?;
        Ok(())
    }
}

// Internal hex encoder helper
mod hex {
    pub fn encode(bytes: Vec<u8>) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn create_test_entry(index: usize) -> ExecutionLogEntry {
        ExecutionLogEntry {
            timestamp: format!("2023-10-27 10:00:0{}", index),
            server_name: "test_server".to_string(),
            database_name: "test_db".to_string(),
            query_source: "SELECT 1".to_string(),
            rows_affected: 1,
            duration_ms: 10,
            output_file: "test.csv".to_string(),
            status: "Success".to_string(),
            error_message: None,
        }
    }

    #[test]
    fn test_log_execution_creates_new_file() {
        let dir = tempdir().unwrap();
        let entry = create_test_entry(1);

        CsvExporter::log_execution(dir.path(), &entry).unwrap();

        let log_file = dir.path().join("execution_log.json");
        assert!(log_file.exists());

        let content = fs::read_to_string(&log_file).unwrap();
        let entries: Vec<ExecutionLogEntry> = serde_json::from_str(&content).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], entry);
    }

    #[test]
    fn test_log_execution_appends_to_existing_file() {
        let dir = tempdir().unwrap();
        let entry1 = create_test_entry(1);
        let entry2 = create_test_entry(2);

        // Log first entry
        CsvExporter::log_execution(dir.path(), &entry1).unwrap();

        // Log second entry
        CsvExporter::log_execution(dir.path(), &entry2).unwrap();

        let log_file = dir.path().join("execution_log.json");
        let content = fs::read_to_string(&log_file).unwrap();
        let entries: Vec<ExecutionLogEntry> = serde_json::from_str(&content).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], entry1);
        assert_eq!(entries[1], entry2);
    }

    #[test]
    fn test_log_execution_handles_malformed_json() {
        let dir = tempdir().unwrap();
        let log_file = dir.path().join("execution_log.json");

        // Create malformed JSON file
        fs::write(&log_file, "this is not valid json").unwrap();

        let entry = create_test_entry(1);

        // Should overwrite with default (empty list) + new entry
        CsvExporter::log_execution(dir.path(), &entry).unwrap();

        let content = fs::read_to_string(&log_file).unwrap();
        let entries: Vec<ExecutionLogEntry> = serde_json::from_str(&content).unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], entry);
    }
}
