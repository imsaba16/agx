use agx::bundler::Bundler;
use agx::config::AntigravityPaths;
use agx::diff::DiffEngine;
use agx::extract::Extractor;
use agx::models::{ConversationSummary, ExportOptions};
use agx::search::SearchEngine;
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

fn mock_summary(conv_id: &str, title: &str) -> ConversationSummary {
    ConversationSummary {
        conversation_id: conv_id.to_string(),
        title: title.to_string(),
        preview: "Test Preview".to_string(),
        step_count: 10,
        last_modified_time: "2026-10-13T12:00:00Z".to_string(),
        workspace_uris: r#"["file:///fake/path"]"#.to_string(),
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
        last_user_input_time: "2026-10-13T11:55:00Z".to_string(),
        last_user_input_step_index: 2,
        app_data_dir: "".to_string(),
        raw_summary: None,
        group_id: "".to_string(),
    }
}

#[test]
fn test_extract_and_cat_features() {
    let temp = tempdir().unwrap();
    let source_paths = AntigravityPaths {
        base_dir: temp.path().to_path_buf(),
    };
    setup_mock_db(&source_paths.summaries_db());

    let conv_id = "test-extract-1";
    let summary = mock_summary(conv_id, "Extract Test Session");

    let brain_dir = source_paths.conversation_brain(conv_id);
    fs::create_dir_all(&brain_dir).unwrap();
    fs::write(
        brain_dir.join("implementation_plan.md"),
        "# Architecture Plan\n- Migrate Auth",
    )
    .unwrap();
    fs::write(
        brain_dir.join("walkthrough.md"),
        "# Walkthrough\nTests passed",
    )
    .unwrap();

    let bundle_path = temp.path().join("extract_test.agbundle");
    let export_opts = ExportOptions {
        conversation_id: conv_id.to_string(),
        output_path: bundle_path.clone(),
        sanitize: true,
        compact: false,
        workspace_root: None,
        custom_ignore_file: None,
    };

    Bundler::export_bundle(&source_paths, summary, &export_opts).unwrap();

    // 1. Test listing archive files
    let file_list = Extractor::list_files(&bundle_path).unwrap();
    assert!(file_list
        .iter()
        .any(|f| f.path.ends_with("implementation_plan.md")));

    // 2. Test reading content into memory
    let (name, content) = Extractor::read_file_content(&bundle_path, "plan").unwrap();
    assert!(name.ends_with("implementation_plan.md"));
    let text = String::from_utf8(content).unwrap();
    assert!(text.contains("# Architecture Plan"));

    // 3. Test surgical extraction to output folder
    let out_dir = temp.path().join("extracted_output");
    fs::create_dir_all(&out_dir).unwrap();
    let extracted = Extractor::extract(&bundle_path, Some("walkthrough"), Some(&out_dir)).unwrap();
    assert_eq!(extracted.len(), 1);
    assert!(extracted[0].exists());
    let walkthrough_text = fs::read_to_string(&extracted[0]).unwrap();
    assert!(walkthrough_text.contains("# Walkthrough"));
}

#[test]
fn test_diff_bundles() {
    let temp = tempdir().unwrap();
    let source_paths = AntigravityPaths {
        base_dir: temp.path().to_path_buf(),
    };
    setup_mock_db(&source_paths.summaries_db());

    let conv_id_a = "test-diff-a";
    let summary_a = mock_summary(conv_id_a, "Session A");
    let brain_a = source_paths.conversation_brain(conv_id_a);
    fs::create_dir_all(&brain_a).unwrap();
    fs::write(
        brain_a.join("implementation_plan.md"),
        "# Plan A\n- Task 1\n- Task 2",
    )
    .unwrap();

    let bundle_a = temp.path().join("bundle_a.agbundle");
    let export_opts_a = ExportOptions {
        conversation_id: conv_id_a.to_string(),
        output_path: bundle_a.clone(),
        sanitize: true,
        compact: false,
        workspace_root: None,
        custom_ignore_file: None,
    };
    Bundler::export_bundle(&source_paths, summary_a, &export_opts_a).unwrap();

    let conv_id_b = "test-diff-b";
    let summary_b = mock_summary(conv_id_b, "Session B");
    let brain_b = source_paths.conversation_brain(conv_id_b);
    fs::create_dir_all(&brain_b).unwrap();
    fs::write(
        brain_b.join("implementation_plan.md"),
        "# Plan B\n- Task 1\n- Task 3 (Modified)",
    )
    .unwrap();

    let bundle_b = temp.path().join("bundle_b.agbundle");
    let export_opts_b = ExportOptions {
        conversation_id: conv_id_b.to_string(),
        output_path: bundle_b.clone(),
        sanitize: true,
        compact: false,
        workspace_root: None,
        custom_ignore_file: None,
    };
    Bundler::export_bundle(&source_paths, summary_b, &export_opts_b).unwrap();

    // Test diff engine execution
    let res = DiffEngine::diff_bundles(&bundle_a, &bundle_b, Some("plan"));
    assert!(res.is_ok());
}

#[test]
fn test_zero_decompression_search() {
    let temp = tempdir().unwrap();
    let source_paths = AntigravityPaths {
        base_dir: temp.path().to_path_buf(),
    };
    setup_mock_db(&source_paths.summaries_db());

    let conv_id = "test-search-1";
    let summary = mock_summary(conv_id, "Searchable Session");
    let brain = source_paths.conversation_brain(conv_id);
    fs::create_dir_all(&brain).unwrap();
    fs::write(
        brain.join("notes.txt"),
        "Line 1\nCriticalStripeWebhookToken\nLine 3",
    )
    .unwrap();

    let bundle = temp.path().join("search_test.agbundle");
    let export_opts = ExportOptions {
        conversation_id: conv_id.to_string(),
        output_path: bundle.clone(),
        sanitize: true,
        compact: false,
        workspace_root: None,
        custom_ignore_file: None,
    };
    Bundler::export_bundle(&source_paths, summary, &export_opts).unwrap();

    // Search inside the archive directory
    let hits =
        SearchEngine::search_archives("CriticalStripeWebhookToken", temp.path(), false).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line_number, 2);
    assert!(hits[0].line_content.contains("CriticalStripeWebhookToken"));
}
