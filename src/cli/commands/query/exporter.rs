use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tokio_postgres::Row;

use crate::error::{MustelError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
