use super::*;
use uuid::Uuid;

struct TestConfigFile {
    directory: PathBuf,
    path: PathBuf,
}

impl TestConfigFile {
    fn new(label: &str) -> Self {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-config")
            .join(Uuid::new_v4().to_string());
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!("{label}.json"));
        Self { directory, path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestConfigFile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn saved_connection(connection_type: &str, id: &str) -> SavedConnection {
    SavedConnection {
        id: id.into(),
        connection_type: connection_type.into(),
        ..SavedConnection::default()
    }
}

#[test]
fn edited_config_merges_only_changed_settings_into_latest() {
    let base = AppConfig::default();
    let mut edited = base.clone();
    edited.terminal.scrollback = 20_000;

    let mut latest = base.clone();
    latest.language = "ja".into();
    latest.terminal.font_size = 18;
    latest
        .saved_connections
        .push(saved_connection("ssh", "latest profile"));

    let merged = merge_edited_config(&base, &edited, &latest).unwrap();

    assert_eq!(merged.language, "ja");
    assert_eq!(merged.terminal.font_size, 18);
    assert_eq!(merged.terminal.scrollback, 20_000);
    assert_eq!(merged.saved_connections, latest.saved_connections);
}

#[test]
fn later_edit_wins_when_the_same_setting_changed() {
    let base = AppConfig::default();
    let mut edited = base.clone();
    edited.terminal.scrollback = 20_000;
    let mut latest = base.clone();
    latest.terminal.scrollback = 5_000;

    let merged = merge_edited_config(&base, &edited, &latest).unwrap();

    assert_eq!(merged.terminal.scrollback, 20_000);
}

#[test]
fn shortcut_removal_is_applied_as_an_atomic_change() {
    let base = AppConfig::default();
    let mut edited = base.clone();
    edited.shortcuts.open_settings = None;
    let mut latest = base.clone();
    latest.shortcuts.new_window = None;

    let merged = merge_edited_config(&base, &edited, &latest).unwrap();

    assert_eq!(merged.shortcuts.open_settings, None);
    assert_eq!(merged.shortcuts.new_window, None);
}

#[test]
fn shortcut_binding_is_not_merged_from_individual_properties() {
    let base = AppConfig::default();
    let mut edited = base.clone();
    let edited_binding = ShortcutBinding {
        key: "s".into(),
        ctrl: true,
        alt: false,
        shift: true,
    };
    edited.shortcuts.open_settings = Some(edited_binding.clone());
    let mut latest = base.clone();
    latest.shortcuts.open_settings = Some(ShortcutBinding {
        key: ",".into(),
        ctrl: false,
        alt: true,
        shift: false,
    });

    let merged = merge_edited_config(&base, &edited, &latest).unwrap();

    assert_eq!(merged.shortcuts.open_settings, Some(edited_binding));
}

#[test]
fn saved_connection_mutations_preserve_unrelated_entries() {
    let mut config = AppConfig::default();
    config
        .saved_connections
        .push(saved_connection("ssh", "existing ssh"));
    config
        .saved_connections
        .push(saved_connection("telnet", "existing telnet"));

    assert!(
        upsert_saved_connection(&mut config, None, saved_connection("ssh", "new ssh")).unwrap()
    );
    delete_saved_connection(&mut config, "ssh", "existing ssh").unwrap();

    assert_eq!(config.saved_connections.len(), 2);
    assert!(config
        .saved_connections
        .iter()
        .any(|profile| profile.id == "existing telnet"));
    assert!(config
        .saved_connections
        .iter()
        .any(|profile| profile.id == "new ssh"));
}

#[test]
fn saved_connection_update_rejects_duplicates_and_missing_targets() {
    let mut config = AppConfig::default();
    config
        .saved_connections
        .push(saved_connection("ssh", "first"));
    config
        .saved_connections
        .push(saved_connection("ssh", "second"));

    let duplicate = upsert_saved_connection(
        &mut config,
        Some("first"),
        saved_connection("ssh", "second"),
    )
    .unwrap_err();
    let missing = upsert_saved_connection(
        &mut config,
        Some("missing"),
        saved_connection("ssh", "renamed"),
    )
    .unwrap_err();

    assert_eq!(duplicate.code, "config.saved_connection_duplicate");
    assert_eq!(missing.code, "config.saved_connection_not_found");
}

#[test]
fn canonical_config_load_does_not_rewrite_the_file() {
    let config_file = TestConfigFile::new("canonical");
    let path = config_file.path();
    let compact = serde_json::to_string(&AppConfig::default()).unwrap();
    fs::write(&path, &compact).unwrap();

    let loaded = config_load_from_path(&path).unwrap();

    assert_eq!(loaded.config_version, CURRENT_CONFIG_VERSION);
    assert_eq!(fs::read_to_string(&path).unwrap(), compact);
}

#[test]
fn missing_config_is_created_with_current_defaults() {
    let config_file = TestConfigFile::new("missing");
    let path = config_file.path();

    let loaded = config_load_from_path(&path).unwrap();
    let stored: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

    assert_eq!(loaded.config_version, CURRENT_CONFIG_VERSION);
    assert_eq!(stored, serde_json::to_value(&loaded).unwrap());
}

#[test]
fn legacy_or_partial_config_is_rewritten_once() {
    let config_file = TestConfigFile::new("legacy");
    let path = config_file.path();
    fs::write(&path, r#"{"config_version":1,"language":"ja"}"#).unwrap();

    let loaded = config_load_from_path(&path).unwrap();
    let rewritten = fs::read_to_string(&path).unwrap();

    assert_eq!(loaded.config_version, CURRENT_CONFIG_VERSION);
    assert_eq!(loaded.language, "ja");
    assert_ne!(rewritten, r#"{"config_version":1,"language":"ja"}"#);
    assert!(!config_read_from_path(&path).unwrap().1);
}

#[test]
fn partial_config_uses_defaults_and_migrates_version() {
    let cfg: AppConfig = serde_json::from_str(r#"{"config_version":4,"language":"ja"}"#).unwrap();
    let cfg = cfg.migrate();

    assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
    assert_eq!(cfg.language, "ja");
    assert!(cfg.updates.check_on_startup);
    assert!(cfg.connection_history.enabled);
    assert_eq!(cfg.ai.default_provider, DEFAULT_AI_PROVIDER);
    assert_eq!(cfg.ai.default_model, DEFAULT_AI_MODEL);
    assert!(!cfg.ai.debug_log_enabled);
    assert!(!cfg.ai.azure_openai_enabled);
    assert!(!cfg.external_control.enabled);
    assert!(!cfg.external_control.connect_enabled);
    assert!(!cfg.external_control.direct_connect_enabled);
    assert!(!cfg.external_control.mcp_enabled);
    assert!(!cfg.external_control.cli_enabled);
    assert_eq!(cfg.shortcuts, ShortcutConfig::default());
    assert_eq!(cfg.ai.azure_openai_endpoint, "");
    assert_eq!(cfg.ai.azure_openai_deployment, "");
    assert_eq!(cfg.terminal.font_size, 14);
    assert_eq!(cfg.terminal.scrollback, 10000);
    assert!(!cfg.terminal.auto_session_log);
    assert_eq!(cfg.terminal.log_format, "display");
    assert!(!cfg.terminal.include_log_header);
    assert_eq!(cfg.ssh.algorithm_mode, "default");
    assert_eq!(cfg.ssh.algorithms, SshAlgorithmSelection::default());
    assert_eq!(cfg.ssh.default_private_key_path, "");
    assert!(cfg.saved_connections.is_empty());
}

#[test]
fn update_preference_round_trips_when_disabled() {
    let cfg = serde_json::from_str::<AppConfig>(
        r#"{
                "config_version": 5,
                "updates": {"check_on_startup": false}
            }"#,
    )
    .unwrap()
    .migrate();

    assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
    assert!(!cfg.updates.check_on_startup);
    let value = serde_json::to_value(cfg).unwrap();
    assert_eq!(value["updates"]["check_on_startup"], false);
}

#[test]
fn connection_history_preference_round_trips_when_disabled() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "config_version": 6,
                "connection_history": { "enabled": false }
            }"#,
    )
    .unwrap();

    assert!(!cfg.connection_history.enabled);
    let value = serde_json::to_value(&cfg).unwrap();
    assert_eq!(value["connection_history"]["enabled"], false);
}

#[test]
fn ssh_default_private_key_path_round_trips() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "config_version": 7,
                "ssh": {
                    "algorithm_mode": "default",
                    "default_private_key_path": "C:\\Users\\me\\.ssh\\id_ed25519"
                }
            }"#,
    )
    .unwrap();

    assert_eq!(
        cfg.ssh.default_private_key_path,
        "C:\\Users\\me\\.ssh\\id_ed25519"
    );
    let value = serde_json::to_value(cfg).unwrap();
    assert_eq!(
        value["ssh"]["default_private_key_path"],
        "C:\\Users\\me\\.ssh\\id_ed25519"
    );
}

#[test]
fn partial_nested_config_uses_field_defaults() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "config_version": 1,
                "ai": {"default_provider": "Ollama"},
                "ssh": {"allow_legacy_algorithms": true},
                "terminal": {"auto_session_log": true},
                "saved_connections": [{"id": "dev box", "connection_type": "ssh"}]
            }"#,
    )
    .unwrap();

    assert_eq!(cfg.ai.default_provider, "Ollama");
    assert!(!cfg.ai.azure_openai_enabled);
    assert_eq!(cfg.ai.azure_openai_endpoint, "");
    assert_eq!(cfg.ai.azure_openai_deployment, "");
    assert_eq!(cfg.ai.ollama_base_url, "http://localhost:11434");
    assert_eq!(cfg.ai.default_model, DEFAULT_AI_MODEL);
    assert!(!cfg.ai.debug_log_enabled);
    assert_eq!(cfg.terminal.font_size, 14);
    assert!(cfg.terminal.auto_session_log);
    assert_eq!(cfg.terminal.log_format, "display");
    assert!(!cfg.terminal.include_log_header);
    assert_eq!(cfg.ssh.algorithm_mode, "custom");
    assert!(cfg
        .ssh
        .algorithms
        .kex
        .contains(&"diffie-hellman-group1-sha1".to_string()));
    let serialized = serde_json::to_value(&cfg).unwrap();
    assert!(serialized["ssh"].get("allow_legacy_algorithms").is_none());
    assert_eq!(serialized["ssh"]["algorithm_mode"], "custom");
    assert_eq!(cfg.saved_connections[0].id, "dev box");
    assert_eq!(cfg.saved_connections[0].encoding, None);
    assert_eq!(cfg.saved_connections[0].terminal_mode, None);
    assert_eq!(cfg.saved_connections[0].auth_method, None);
    assert_eq!(cfg.saved_connections[0].private_key_path, None);
    assert_eq!(cfg.saved_connections[0].jump_profile_id, None);
    assert_eq!(cfg.saved_connections[0].memo, None);
    assert!(cfg.saved_connections[0].external_control_enabled);
}

#[test]
fn legacy_mcp_config_is_migrated_to_external_control() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "config_version": 1,
                "mcp": {
                    "enabled": true,
                    "connect_enabled": true,
                    "stdio_enabled": true,
                    "cli_enabled": false
                }
            }"#,
    )
    .unwrap();

    let cfg = cfg.migrate();

    assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
    assert!(cfg.external_control.enabled);
    assert!(cfg.external_control.connect_enabled);
    assert!(cfg.external_control.mcp_enabled);
    assert!(!cfg.external_control.cli_enabled);
}

#[test]
fn new_external_control_config_takes_priority_over_legacy_mcp() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "config_version": 1,
                "mcp": {
                    "enabled": false,
                    "connect_enabled": false,
                    "stdio_enabled": true,
                    "cli_enabled": false
                },
                "external_control": {
                    "enabled": true,
                    "cli_enabled": true
                }
            }"#,
    )
    .unwrap();

    assert!(cfg.external_control.enabled);
    assert!(!cfg.external_control.connect_enabled);
    assert!(cfg.external_control.mcp_enabled);
    assert!(cfg.external_control.cli_enabled);
}

#[test]
fn saved_connection_preserves_encoding() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "legacy-router",
                    "connection_type": "ssh",
                    "encoding": "shift-jis"
                }]
            }"#,
    )
    .unwrap();

    assert_eq!(
        cfg.saved_connections[0].encoding.as_deref(),
        Some("shift-jis")
    );
}

#[test]
fn saved_connection_preserves_terminal_mode() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "ios-router",
                    "connection_type": "ssh",
                    "terminal_mode": "cisco_ios"
                }, {
                    "id": "eos-switch",
                    "connection_type": "ssh",
                    "terminal_mode": "arista_eos"
                }, {
                    "id": "junos-router",
                    "connection_type": "ssh",
                    "terminal_mode": "juniper_junos"
                }, {
                    "id": "vyos-router",
                    "connection_type": "ssh",
                    "terminal_mode": "vyos"
                }, {
                    "id": "sir-router",
                    "connection_type": "ssh",
                    "terminal_mode": "fujitsu_sir"
                }, {
                    "id": "awplus-switch",
                    "connection_type": "ssh",
                    "terminal_mode": "allied_telesis_awplus"
                }, {
                    "id": "fitelnet-router",
                    "connection_type": "ssh",
                    "terminal_mode": "furukawa_fitelnet"
                }]
            }"#,
    )
    .unwrap();

    assert_eq!(
        cfg.saved_connections[0].terminal_mode.as_deref(),
        Some("cisco_ios")
    );
    assert_eq!(
        cfg.saved_connections[1].terminal_mode.as_deref(),
        Some("arista_eos")
    );
    assert_eq!(
        cfg.saved_connections[2].terminal_mode.as_deref(),
        Some("juniper_junos")
    );
    assert_eq!(
        cfg.saved_connections[3].terminal_mode.as_deref(),
        Some("vyos")
    );
    assert_eq!(
        cfg.saved_connections[4].terminal_mode.as_deref(),
        Some("fujitsu_sir")
    );
    assert_eq!(
        cfg.saved_connections[5].terminal_mode.as_deref(),
        Some("allied_telesis_awplus")
    );
    assert_eq!(
        cfg.saved_connections[6].terminal_mode.as_deref(),
        Some("furukawa_fitelnet")
    );
}

#[test]
fn saved_connection_preserves_telnet_profile_fields() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "legacy-telnet",
                    "connection_type": "telnet",
                    "host": "192.168.1.10",
                    "port": 23,
                    "encoding": "euc-jp"
                }]
            }"#,
    )
    .unwrap();

    let profile = &cfg.saved_connections[0];
    assert_eq!(profile.id, "legacy-telnet");
    assert_eq!(profile.connection_type, "telnet");
    assert_eq!(profile.host.as_deref(), Some("192.168.1.10"));
    assert_eq!(profile.port, Some(23));
    assert_eq!(profile.encoding.as_deref(), Some("euc-jp"));
}

#[test]
fn saved_connection_preserves_public_key_auth_path() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "key-router",
                    "connection_type": "ssh",
                    "auth_method": "public_key",
                    "private_key_path": "C:\\Users\\me\\.ssh\\id_ed25519"
                }]
            }"#,
    )
    .unwrap();

    assert_eq!(
        cfg.saved_connections[0].auth_method.as_deref(),
        Some("public_key")
    );
    assert_eq!(
        cfg.saved_connections[0].private_key_path.as_deref(),
        Some("C:\\Users\\me\\.ssh\\id_ed25519")
    );
}

#[test]
fn saved_connection_preserves_jump_profile_id() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "inside",
                    "connection_type": "ssh",
                    "jump_profile_id": "bastion"
                }]
            }"#,
    )
    .unwrap();

    assert_eq!(
        cfg.saved_connections[0].jump_profile_id.as_deref(),
        Some("bastion")
    );
}

#[test]
fn saved_connection_preserves_memo() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh",
                    "memo": "Cisco ISR at branch A"
                }]
            }"#,
    )
    .unwrap();

    assert_eq!(
        cfg.saved_connections[0].memo.as_deref(),
        Some("Cisco ISR at branch A")
    );
}

#[test]
fn saved_connection_defaults_external_control_enabled_to_true() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh"
                }]
            }"#,
    )
    .unwrap();

    assert!(cfg.saved_connections[0].external_control_enabled);
}

#[test]
fn saved_connection_migrates_legacy_mcp_enabled_false() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh",
                    "mcp_enabled": false
                }]
            }"#,
    )
    .unwrap();

    assert!(!cfg.saved_connections[0].external_control_enabled);
}

#[test]
fn saved_connection_new_flag_takes_priority_over_legacy_flag() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh",
                    "external_control_enabled": true,
                    "mcp_enabled": false
                }]
            }"#,
    )
    .unwrap();

    assert!(cfg.saved_connections[0].external_control_enabled);
}

#[test]
fn serialization_omits_legacy_mcp_fields() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "config_version": 1,
                "mcp": {
                    "enabled": true,
                    "connect_enabled": true,
                    "stdio_enabled": true,
                    "cli_enabled": true
                },
                "saved_connections": [{
                    "id": "edge-router",
                    "connection_type": "ssh",
                    "mcp_enabled": false
                }]
            }"#,
    )
    .unwrap();

    let cfg = cfg.migrate();
    let value = serde_json::to_value(&cfg).unwrap();

    assert_eq!(value["config_version"], CURRENT_CONFIG_VERSION);
    assert!(value.get("mcp").is_none());
    assert_eq!(value["external_control"]["mcp_enabled"], true);
    assert!(value["saved_connections"][0].get("mcp_enabled").is_none());
    assert_eq!(
        value["saved_connections"][0]["external_control_enabled"],
        false
    );
}

#[test]
fn direct_external_connection_permission_round_trips() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "external_control": {
                    "enabled": true,
                    "connect_enabled": true,
                    "direct_connect_enabled": true,
                    "cli_enabled": true
                }
            }"#,
    )
    .unwrap();

    assert!(cfg.external_control.direct_connect_enabled);
    let value = serde_json::to_value(cfg).unwrap();
    assert_eq!(value["external_control"]["direct_connect_enabled"], true);
}

#[test]
fn default_config_uses_system_language() {
    let cfg = AppConfig::default();

    assert_eq!(cfg.language, "system");
    assert_eq!(cfg.config_version, CURRENT_CONFIG_VERSION);
}

#[test]
fn shortcut_config_preserves_explicit_null_and_defaults_missing_actions() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "shortcuts": {
                    "new_connection": null,
                    "new_window": {"key": "F2"},
                    "terminal_copy": null,
                    "terminal_log_start_append": null,
                    "new_tab": {"key": "t", "ctrl": true}
                }
            }"#,
    )
    .unwrap();

    assert_eq!(cfg.shortcuts.new_connection, None);
    assert_eq!(
        cfg.shortcuts.new_window,
        Some(ShortcutBinding {
            key: "F2".into(),
            ctrl: false,
            alt: false,
            shift: false,
        })
    );
    assert_eq!(
        cfg.shortcuts.open_settings,
        default_open_settings_shortcut()
    );
    assert_eq!(cfg.shortcuts.exit, None);
    assert_eq!(
        cfg.shortcuts.terminal_select_all,
        default_terminal_select_all_shortcut()
    );
    assert_eq!(cfg.shortcuts.terminal_copy, None);
    assert_eq!(
        cfg.shortcuts.terminal_paste,
        default_terminal_paste_shortcut()
    );
    assert_eq!(cfg.shortcuts.terminal_clear_viewport, None);
    assert_eq!(cfg.shortcuts.terminal_clear_buffer, None);
    assert_eq!(
        cfg.shortcuts.terminal_mode_menu,
        default_terminal_mode_menu_shortcut()
    );
    assert_eq!(
        cfg.shortcuts.terminal_log_start_overwrite,
        default_terminal_log_start_overwrite_shortcut()
    );
    assert_eq!(cfg.shortcuts.terminal_log_start_append, None);
    assert_eq!(
        cfg.shortcuts.terminal_log_stop,
        default_terminal_log_stop_shortcut()
    );
    assert_eq!(cfg.shortcuts.terminal_log_pause, None);
    assert_eq!(cfg.shortcuts.terminal_log_resume, None);
    let serialized = serde_json::to_value(cfg).unwrap();
    assert!(serialized["shortcuts"].get("new_tab").is_none());
}

#[test]
fn shortcut_config_normalizes_keys_during_migration() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{
                "shortcuts": {
                    "new_connection": {"key": "N", "ctrl": true},
                    "new_window": {"key": "f2"},
                    "open_settings": null,
                    "exit": {"key": "Q", "ctrl": true, "shift": true},
                    "terminal_select_all": {"key": "A", "ctrl": true, "shift": true},
                    "terminal_clear_viewport": {"key": "K", "ctrl": true, "shift": true},
                    "terminal_log_stop": {"key": "f10", "ctrl": true, "shift": true}
                }
            }"#,
    )
    .unwrap();
    let cfg = cfg.migrate();

    assert_eq!(cfg.shortcuts.new_connection.as_ref().unwrap().key, "n");
    assert_eq!(cfg.shortcuts.new_window.as_ref().unwrap().key, "F2");
    assert_eq!(cfg.shortcuts.exit.as_ref().unwrap().key, "q");
    assert_eq!(cfg.shortcuts.terminal_select_all.as_ref().unwrap().key, "a");
    assert_eq!(
        cfg.shortcuts.terminal_clear_viewport.as_ref().unwrap().key,
        "k"
    );
    assert_eq!(cfg.shortcuts.terminal_log_stop.as_ref().unwrap().key, "F10");
    assert!(validate_shortcut_config(&cfg.shortcuts).is_ok());
}

#[test]
fn shortcut_config_preserves_explicit_null_exit() {
    let cfg: AppConfig = serde_json::from_str(r#"{"shortcuts":{"exit":null}}"#).unwrap();

    assert_eq!(cfg.shortcuts.exit, None);
    assert!(serde_json::to_value(cfg).unwrap()["shortcuts"]["exit"].is_null());
}

#[test]
fn shortcut_config_preserves_explicit_null_terminal_clear_actions() {
    let cfg: AppConfig = serde_json::from_str(
        r#"{"shortcuts":{"terminal_clear_viewport":null,"terminal_clear_buffer":null}}"#,
    )
    .unwrap();

    assert_eq!(cfg.shortcuts.terminal_clear_viewport, None);
    assert_eq!(cfg.shortcuts.terminal_clear_buffer, None);
    let value = serde_json::to_value(cfg).unwrap();
    assert!(value["shortcuts"]["terminal_clear_viewport"].is_null());
    assert!(value["shortcuts"]["terminal_clear_buffer"].is_null());
}

#[test]
fn shortcut_config_preserves_explicit_null_terminal_mode_menu() {
    let cfg: AppConfig =
        serde_json::from_str(r#"{"shortcuts":{"terminal_mode_menu":null}}"#).unwrap();

    assert_eq!(cfg.shortcuts.terminal_mode_menu, None);
    assert!(serde_json::to_value(cfg).unwrap()["shortcuts"]["terminal_mode_menu"].is_null());
}

#[test]
fn shortcut_config_rejects_duplicate_assignments() {
    let duplicate = Some(ShortcutBinding {
        key: "n".into(),
        ctrl: true,
        alt: false,
        shift: false,
    });
    let shortcuts = ShortcutConfig {
        new_connection: duplicate.clone(),
        new_window: None,
        open_settings: None,
        exit: None,
        terminal_select_all: None,
        terminal_copy: None,
        terminal_paste: None,
        terminal_clear_viewport: None,
        terminal_clear_buffer: duplicate,
        terminal_mode_menu: None,
        terminal_log_start_overwrite: None,
        terminal_log_start_append: None,
        terminal_log_stop: None,
        terminal_log_pause: None,
        terminal_log_resume: None,
    };

    assert_eq!(
        validate_shortcut_config(&shortcuts),
        Err("Shortcut assignments must be unique".into())
    );
}

#[test]
fn shortcut_config_validates_modifier_and_function_key_rules() {
    let mut shortcuts = ShortcutConfig {
        new_connection: Some(ShortcutBinding {
            key: "x".into(),
            ctrl: false,
            alt: false,
            shift: false,
        }),
        new_window: None,
        open_settings: None,
        exit: None,
        terminal_select_all: None,
        terminal_copy: None,
        terminal_paste: None,
        terminal_clear_viewport: None,
        terminal_clear_buffer: None,
        terminal_mode_menu: None,
        terminal_log_start_overwrite: None,
        terminal_log_start_append: None,
        terminal_log_stop: None,
        terminal_log_pause: None,
        terminal_log_resume: None,
    };
    assert!(validate_shortcut_config(&shortcuts).is_err());

    shortcuts.new_connection = Some(ShortcutBinding {
        key: "F12".into(),
        ctrl: false,
        alt: false,
        shift: false,
    });
    assert!(validate_shortcut_config(&shortcuts).is_ok());

    shortcuts.new_connection = Some(ShortcutBinding {
        key: "F4".into(),
        ctrl: false,
        alt: true,
        shift: false,
    });
    assert!(validate_shortcut_config(&shortcuts).is_err());
}
