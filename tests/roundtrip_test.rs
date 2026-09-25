use agx::bundler::Bundler;
use agx::config::AntigravityPaths;
use agx::db::Database;
use agx::importer::Importer;
use agx::models::{ConversationSummary, ExportOptions, ImportOptions};
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

fn setup_mock_db(db_path: &PathBuf) {
    let conn = Connection::open(db_path).unwrap();
    conn.execute(
        "CREATE TABLE conversation_summaries (
            conversation_id text PRIMARY KEY,
            title text NOT NULL DEFAULT '',
            preview text NOT NULL DEFAULT '',
            step_count integer NOT NULL DEFAULT 0,
            last_modified_time datetime NOT NULL,
            workspace_uris text NOT NULL,
            status text NOT NULL DEFAULT '',
            source text NOT NULL DEFAULT '',
            project_id text NOT NULL DEFAULT '',
            agent_name text NOT NULL DEFAULT '',
            parent_conversation_id text NOT NULL DEFAULT '',
            nesting_depth integer NOT NULL DEFAULT 0,
            battle_id text NOT NULL DEFAULT '',
            winning_conversation_id text NOT NULL DEFAULT '',
            not_fully_idle numeric NOT NULL DEFAULT false,
            killed numeric NOT NULL DEFAULT false,
            last_user_input_time datetime NOT NULL,
            last_user_input_step_index integer NOT NULL DEFAULT -1,
            app_data_dir text NOT NULL DEFAULT '',
            raw_summary blob,
            group_id text NOT NULL DEFAULT ''
        )",
        [],
    )
    .unwrap();
}

#[test]
fn test_full_export_import_roundtrip() {
    let source_dir = tempdir().unwrap();
    let source_base = source_dir.path().to_path_buf();
    let source_paths = AntigravityPaths {
        base_dir: source_base.clone(),
    };

    // 1. Create mock source files
    let summaries_db = source_paths.summaries_db();
    setup_mock_db(&summaries_db);

    let conv_id = "test-conversation-uuid-1234";
    let original_ws = "/fake/original/path";

    let summary = ConversationSummary {
        conversation_id: conv_id.to_string(),
        title: "Test Feature Planning".to_string(),
        preview: "Building a cool feature".to_string(),
        step_count: 42,
        last_modified_time: "2026-09-25T12:00:00Z".to_string(),
        workspace_uris: format!(r#"["file://{}"]"#, original_ws),
        status: "DONE".to_string(),
        source: "USER".to_string(),
        project_id: "test-proj".to_string(),
        agent_name: "Antigravity".to_string(),
        parent_conversation_id: "".to_string(),
        nesting_depth: 0,
        battle_id: "".to_string(),
        winning_conversation_id: "".to_string(),
        not_fully_idle: false,
        killed: false,
        last_user_input_time: "2026-09-25T11:50:00Z".to_string(),
        last_user_input_step_index: 10,
        app_data_dir: "".to_string(),
        raw_summary: None,
        group_id: "".to_string(),
    };

    let db = Database::new(&summaries_db);
    db.upsert_conversation(&summary).unwrap();

    // Mock conversation.db
    let convs_dir = source_paths.conversations_dir();
    fs::create_dir_all(&convs_dir).unwrap();
    let conv_db_file = source_paths.conversation_db(conv_id);
    fs::write(&conv_db_file, b"MOCK_SQLITE_BINARY_DATA").unwrap();

    // Mock brain directory and artifacts
    let brain_dir = source_paths.conversation_brain(conv_id);
    fs::create_dir_all(&brain_dir).unwrap();

    let plan_content = format!(
        "# Plan\nWork on file://{}/src/main.rs with secret sk-1234567890abcdef1234567890\n",
        original_ws
    );
    fs::write(brain_dir.join("implementation_plan.md"), plan_content).unwrap();
    fs::write(brain_dir.join("walkthrough.md"), "# Walkthrough\nAll done.").unwrap();

    // 2. Export bundle
    let bundle_path = source_base.join("backup.agbundle");
    let export_options = ExportOptions {
        conversation_id: conv_id.to_string(),
        output_path: bundle_path.clone(),
        sanitize: true,
        compact: false,
        workspace_root: Some(PathBuf::from(original_ws)),
        custom_ignore_file: None,
    };

    let manifest = Bundler::export_bundle(&source_paths, summary, &export_options).unwrap();
    assert_eq!(manifest.artifacts_count, 2);
    assert!(manifest.sanitized);
    assert!(bundle_path.exists());

    // 3. Inspect bundle
    let inspected = Bundler::inspect_bundle(&bundle_path).unwrap();
    assert_eq!(inspected.conversation.title, "Test Feature Planning");

    // 4. Import into destination
    let dest_dir = tempdir().unwrap();
    let dest_base = dest_dir.path().to_path_buf();
    let dest_paths = AntigravityPaths {
        base_dir: dest_base.clone(),
    };
    setup_mock_db(&dest_paths.summaries_db());

    let target_ws = "/colleague/new/workspace";
    let import_options = ImportOptions {
        bundle_path: bundle_path.clone(),
        target_workspace: Some(PathBuf::from(target_ws)),
        overwrite: true,
        dry_run: false,
    };

    let (imported_id, _imported_manifest) = Importer::import_bundle(&dest_paths, &import_options).unwrap();
    assert_eq!(imported_id, conv_id);

    // 5. Verify restored artifacts and path remapping
    let restored_plan = fs::read_to_string(
        dest_paths.conversation_brain(&imported_id).join("implementation_plan.md"),
    )
    .unwrap();

    // Ensure path was remapped to target workspace
    assert!(
        restored_plan.contains(&format!("file://{}/src/main.rs", target_ws)),
        "Expected remapped path in plan, got: {}",
        restored_plan
    );
    // Ensure secret was redacted
    assert!(
        restored_plan.contains("[REDACTED_OPENAI_KEY]"),
        "Expected redacted secret in plan, got: {}",
        restored_plan
    );
    assert!(
        !restored_plan.contains("sk-1234567890abcdef"),
        "Secret was not redacted!"
    );

    // Verify destination DB was populated
    let dest_db = Database::new(&dest_paths.summaries_db());
    let dest_conv = dest_db.get_conversation(&imported_id).unwrap().unwrap();
    assert_eq!(dest_conv.title, "Test Feature Planning");
    assert!(dest_conv.workspace_uris.contains(target_ws));

    // Verify conversation.db was restored
    let dest_conv_db = dest_paths.conversation_db(&imported_id);
    assert!(dest_conv_db.exists());
    let db_bytes = fs::read(dest_conv_db).unwrap();
    assert_eq!(db_bytes, b"MOCK_SQLITE_BINARY_DATA");
}
