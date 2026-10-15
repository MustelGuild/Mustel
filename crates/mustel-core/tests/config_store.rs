//! Testes de integração do `ConfigStore` (comportamento herdado do `UserConfigService`).

use mustel_core::config::{ConfigError, ConfigStore, ServerConfigEntry, UserConfig};
use tempfile::TempDir;

fn store_in(dir: &TempDir) -> ConfigStore {
    ConfigStore::new(dir.path().join("mustel.jsonc"))
}

#[test]
fn creates_an_empty_config_on_first_use() {
    let dir = TempDir::new().expect("tempdir");
    let store = store_in(&dir);

    assert!(!store.exists());
    let config = store.load().expect("load");

    assert!(store.exists(), "first load must create the file");
    assert!(config.servers.is_empty());
    assert_eq!(
        std::fs::read_to_string(store.path()).expect("read"),
        "{\n  \"servers\": [],\n  \"defaults\": {\n    \"outputDirectory\": \"./results\",\n    \"requireConfirmation\": true,\n    \"maxParallelism\": 4\n  }\n}"
    );
}

#[test]
fn reads_jsonc_with_comments_and_legacy_extra_fields() {
    let dir = TempDir::new().expect("tempdir");
    let store = store_in(&dir);
    std::fs::write(
        store.path(),
        r#"{
  // servidores
  "servers": [
    {
      "name": "dev",
      "host": "http://db.local",   // host com // de propósito
      "port": 0,                    /* zero vira default */
      "username": "postgres",
      "password": "campo legado ignorado",
      "encryptedPassword": null,
      "commandTimeout": 300,
      "sslMode": ""
    }
  ],
  "defaults": { "outputFormat": "csv" }
}"#,
    )
    .expect("write fixture");

    let config = store.load().expect("load");
    assert_eq!(config.servers.len(), 1);
    let server = &config.servers[0];
    assert_eq!(server.name, "dev");
    assert_eq!(server.host, "http://db.local");
    assert_eq!(server.port, 5432, "port 0 becomes the default");
    assert_eq!(
        server.ssl_mode, "Prefer",
        "empty sslMode becomes the default"
    );
    assert_eq!(server.timeout, 30);
    assert_eq!(server.exclude_patterns, vec!["template*", "postgres"]);
    assert_eq!(
        config.defaults.expect("defaults").output_directory,
        "./results"
    );
}

#[test]
fn reports_invalid_json_as_parse_error() {
    let dir = TempDir::new().expect("tempdir");
    let store = store_in(&dir);
    std::fs::write(store.path(), "{ not json }").expect("write fixture");

    let error = store.load().expect_err("must fail");
    assert!(matches!(error, ConfigError::Parse { .. }), "got {error:?}");
}

#[test]
fn adds_updates_and_removes_servers_case_insensitively() {
    let dir = TempDir::new().expect("tempdir");
    let store = store_in(&dir);

    let dev = ServerConfigEntry {
        name: "dev".to_string(),
        host: "localhost".to_string(),
        username: "postgres".to_string(),
        ..ServerConfigEntry::default()
    };
    store.add_or_update_server(&dev).expect("add");
    assert_eq!(store.get_servers().expect("list").len(), 1);

    let updated = ServerConfigEntry {
        host: "db.internal".to_string(),
        ..dev.clone()
    };
    store.add_or_update_server(&updated).expect("update");
    let servers = store.get_servers().expect("list");
    assert_eq!(servers.len(), 1, "same name must not duplicate");
    assert_eq!(servers[0].host, "db.internal");

    assert!(store.get_server("DEV").expect("get").is_some());
    assert!(store.remove_server("DeV").expect("remove"));
    assert!(store.get_servers().expect("list").is_empty());
    assert!(!store.remove_server("dev").expect("remove twice"));
}

#[test]
fn rejects_empty_server_name() {
    let dir = TempDir::new().expect("tempdir");
    let store = store_in(&dir);

    let error = store
        .add_or_update_server(&ServerConfigEntry::default())
        .expect_err("must fail");
    assert!(matches!(error, ConfigError::EmptyServerName));
}

#[test]
fn saved_config_is_reloadable_and_has_no_temporary_leftover() {
    let dir = TempDir::new().expect("tempdir");
    let store = store_in(&dir);
    let mut config = UserConfig::default();
    config.servers.push(ServerConfigEntry {
        name: "prod".to_string(),
        host: "prod-db.company.com".to_string(),
        username: "readonly".to_string(),
        fetch_all_databases: true,
        ssl_mode: "Require".to_string(),
        ..ServerConfigEntry::default()
    });
    store.save(&config).expect("save");

    assert_eq!(store.load().expect("load"), config);
    assert!(
        !store.path().with_extension("jsonc.tmp").exists(),
        "atomic write must not leave the temp file behind"
    );
}
