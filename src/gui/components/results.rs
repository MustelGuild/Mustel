use iced::widget::{column, container, row, scrollable, text};
use iced::{Element, Length};
use super::super::app::Message;

#[derive(Clone, Debug)]
pub struct QueryResultData {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub execution_time_ms: u128,
}

impl Default for QueryResultData {
    fn default() -> Self {
        Self {
            columns: vec!["id".to_string(), "name".to_string(), "status".to_string()],
            rows: vec![
                vec!["1".to_string(), "PostgreSQL Local".to_string(), "Pronto para executar".to_string()],
            ],
            execution_time_ms: 0,
        }
    }
}

impl QueryResultData {
    pub fn from_postgres_rows(rows: &[tokio_postgres::Row], execution_time_ms: u128) -> Self {
        if rows.is_empty() {
            return Self {
                columns: vec!["status".to_string()],
                rows: vec![vec!["Query executada com sucesso (0 linhas retornadas)".to_string()]],
                execution_time_ms,
            };
        }

        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| c.name().to_string())
            .collect();

        let mut data_rows = Vec::with_capacity(rows.len());

        for row in rows {
            let mut record = Vec::with_capacity(columns.len());
            for (idx, col) in row.columns().iter().enumerate() {
                let cell_str = format_cell(row, idx, col.type_().name());
                record.push(cell_str);
            }
            data_rows.push(record);
        }

        Self {
            columns,
            rows: data_rows,
            execution_time_ms,
        }
    }
}

fn format_cell(row: &tokio_postgres::Row, idx: usize, _type_name: &str) -> String {
    if let Ok(val) = row.try_get::<_, Option<&str>>(idx) {
        return val.unwrap_or("NULL").to_string();
    }
    if let Ok(val) = row.try_get::<_, Option<String>>(idx) {
        return val.unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<i64>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<i32>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<i16>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<bool>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<f64>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<f32>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<chrono::NaiveDateTime>>(idx) {
        return val.map(|v| v.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<chrono::DateTime<chrono::Utc>>>(idx) {
        return val.map(|v| v.format("%Y-%m-%d %H:%M:%S UTC").to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<serde_json::Value>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<uuid::Uuid>>(idx) {
        return val.map(|v| v.to_string()).unwrap_or_else(|| "NULL".to_string());
    }
    if let Ok(val) = row.try_get::<_, Option<Vec<u8>>>(idx) {
        return val.map(|bytes| format!("\\x{}", bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>())).unwrap_or_else(|| "NULL".to_string());
    }

    "NULL".to_string()
}

pub struct QueryResultTable {
    pub data: Option<QueryResultData>,
    pub is_loading: bool,
    pub error_message: Option<String>,
}

impl Default for QueryResultTable {
    fn default() -> Self {
        Self {
            data: Some(QueryResultData::default()),
            is_loading: false,
            error_message: None,
        }
    }
}

impl QueryResultTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let title_str = if self.is_loading {
            "Resultados (Executando no PostgreSQL...)".to_string()
        } else if let Some(ref data) = self.data {
            format!("Resultados ({} linhas, {} ms)", data.rows.len(), data.execution_time_ms)
        } else {
            "Resultados".to_string()
        };

        let title = text(title_str).size(20);

        if let Some(ref err) = self.error_message {
            return container(
                column![
                    title,
                    text(format!("❌ Erro no PostgreSQL: {}", err)).style(text::danger),
                ]
                .spacing(10)
            )
            .into();
        }

        let Some(ref data) = self.data else {
            return column![title, text("Nenhum resultado para exibir.")]
                .spacing(10)
                .into();
        };

        let mut header_row = row![].spacing(20);
        for col in &data.columns {
            header_row = header_row.push(text(col).size(16));
        }

        let mut table_column = column![header_row].spacing(10);

        for row_data in &data.rows {
            let mut r = row![].spacing(20);
            for val in row_data {
                r = r.push(text(val));
            }
            table_column = table_column.push(r);
        }

        container(
            column![
                title,
                scrollable(table_column).height(Length::Fill),
            ]
            .spacing(10)
        )
        .into()
    }
}
