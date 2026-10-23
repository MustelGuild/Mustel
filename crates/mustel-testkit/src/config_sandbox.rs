//! Fixtures de configuração usados pelos testes e pelo comparador.
//!
//! Só o conteúdo de exemplo permanece: o sandbox do arquivo do **legado** existia para a captura e
//! saiu com o `legacy/` (o lado do Mustel é isolado por `MUSTEL_CONFIG_DIR`).

pub const SAMPLE_CONFIG: &str = r#"{
  "servers": [
    {
      "name": "dev",
      "host": "localhost",
      "port": 5432,
      "username": "postgres",
      "databases": ["mydb", "app_dev"],
      "sslMode": "Prefer",
      "timeout": 30,
      "maxParallelism": 4
    },
    {
      "name": "offline",
      "host": "127.0.0.1",
      "port": 1,
      "username": "postgres",
      "sslMode": "Disable",
      "timeout": 1
    }
  ],
  "defaults": {
    "outputDirectory": "./results",
    "requireConfirmation": true,
    "maxParallelism": 4
  }
}
"#;
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_config_is_valid_json_with_two_servers() {
        let parsed: serde_json::Value =
            serde_json::from_str(SAMPLE_CONFIG).expect("sample config must parse");
        let servers = parsed["servers"].as_array().expect("servers array");
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0]["name"], "dev");
        assert_eq!(servers[1]["name"], "offline");
    }
}
