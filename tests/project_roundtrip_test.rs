use agx::config::AntigravityPaths;
use agx::db::Database;
use agx::models::{ConversationSummary, ProjectExportOptions, ProjectImportOptions};
use agx::project_bundler::ProjectBundler;
use agx::project_importer::ProjectImporter;
use agx::remapper::{clean_path, to_file_uri};
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

    let _ = conn.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS conversations_fts USING fts5(
            conversation_id UNINDEXED,
            chunk_index UNINDEXED,
            chunk_first_step UNINDEXED,
            chunk_last_step UNINDEXED,
            last_modified_time UNINDEXED,
            step_count UNINDEXED,
            last_step_created_at UNINDEXED,
            archived UNINDEXED,
            user_step_byte_offsets UNINDEXED,
            agent_step_byte_offsets UNINDEXED,
            title,
            workspace,
            project,
            user_body,
            agent_body
        )",
        [],
    );
}

#[test]
fn test_project_export_import_roundtrip() {
    let source_ag_dir = tempdir().unwrap();
    let source_paths = AntigravityPaths {
        base_dir: source_ag_dir.path().to_path_buf(),
    };

    let source_ws_dir = tempdir().unwrap();
    let source_ws = clean_path(source_ws_dir.path());
    let source_ws_str = source_ws.to_string_lossy().to_string();
    let source_uri = to_file_uri(&source_ws);

    // 1. Setup mock database
    let summaries_db = source_paths.summaries_db();
    setup_mock_db(&summaries_db);
    let db = Database::new(&summaries_db);

    // 2. Add two conversations for this workspace
    let conv1_id = "project-conv-1";
    let conv2_id = "project-conv-2";

    let conv1 = ConversationSummary {
        conversation_id: conv1_id.to_string(),
        title: "Session 1 Feature Setup".to_string(),
        preview: "Setting up project feature".to_string(),
        step_count: 25,
        last_modified_time: "2026-10-13T10:00:00Z".to_string(),
        workspace_uris: format!(r#"["{}"]"#, source_uri),
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
        last_user_input_time: "2026-10-13T09:50:00Z".to_string(),
        last_user_input_step_index: 5,
        app_data_dir: "".to_string(),
        raw_summary: None,
        group_id: "".to_string(),
    };

    let conv2 = ConversationSummary {
        conversation_id: conv2_id.to_string(),
        title: "Session 2 Bug Fix".to_string(),
        preview: "Fixing edge cases".to_string(),
        step_count: 14,
        last_modified_time: "2026-10-13T11:00:00Z".to_string(),
        workspace_uris: format!(r#"["{}"]"#, source_uri),
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
        last_user_input_time: "2026-10-13T10:55:00Z".to_string(),
        last_user_input_step_index: 3,
        app_data_dir: "".to_string(),
        raw_summary: None,
        group_id: "".to_string(),
    };

    db.upsert_conversation(&conv1).unwrap();
    db.upsert_conversation(&conv2).unwrap();

    // Create source project config file
    let src_proj_file = source_paths.project_config_file("test-proj");
    fs::create_dir_all(src_proj_file.parent().unwrap()).unwrap();
    fs::write(
        &src_proj_file,
        format!(
            r#"{{"id":"test-proj","name":"test-proj","projectResources":{{"resources":[{{"gitFolder":{{"folderUri":"{}"}}}}]}}}}"#,
            source_uri
        ),
    )
    .unwrap();

    // 3. Create mock conversation.db files & brain directories
    let convs_dir = source_paths.conversations_dir();
    fs::create_dir_all(&convs_dir).unwrap();
    fs::write(source_paths.conversation_db(conv1_id), b"CONV1_SQLITE_DATA").unwrap();
    fs::write(source_paths.conversation_db(conv2_id), b"CONV2_SQLITE_DATA").unwrap();

    let brain1 = source_paths.conversation_brain(conv1_id);
    fs::create_dir_all(&brain1).unwrap();
    fs::write(
        brain1.join("implementation_plan.md"),
        format!(
            "# Plan 1\nWorkspace file: {}/src/app.rs\nAPI Key: sk-proj-1234567890abcdef12345678\n",
            source_uri
        ),
    )
    .unwrap();

    let brain2 = source_paths.conversation_brain(conv2_id);
    fs::create_dir_all(&brain2).unwrap();
    fs::write(
        brain2.join("walkthrough.md"),
        format!("# Walkthrough 2\nFixed bug in {}/src/lib.rs\n", source_uri),
    )
    .unwrap();

    // 4. Create mock project customizations in source workspace
    let skills_dir = source_ws
        .join(".agents")
        .join("skills")
        .join("super-helper");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(
        skills_dir.join("SKILL.md"),
        format!("name: super-helper\npath: {}\n", source_ws_str),
    )
    .unwrap();

    let rules_dir = source_ws.join(".agents").join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(rules_dir.join("rules.md"), "# Code Rules\nClean code.").unwrap();

    let mcp_file = source_ws.join(".agents").join("mcp_config.json");
    fs::write(
        &mcp_file,
        r#"{"mcpServers": {"test": {"command": "node"}}}"#,
    )
    .unwrap();

    // 5. Export project bundle
    let bundle_file = source_ag_dir.path().join("my_test_project.agproject");
    let export_options = ProjectExportOptions {
        workspace_root: source_ws.clone(),
        output_path: bundle_file.clone(),
        session_ids: None, // include all
        include_customizations: true,
        sanitize: true,
        compact: true,
        custom_ignore_file: None,
    };

    let manifest = ProjectBundler::export_project_bundle(&source_paths, &export_options).unwrap();
    assert_eq!(manifest.conversations.len(), 2);
    assert!(manifest.customizations.has_skills);
    assert_eq!(manifest.customizations.skills_count, 1);
    assert!(manifest.customizations.has_rules);
    assert!(manifest.customizations.has_mcp);
    assert!(manifest.sanitized);
    assert!(manifest.compact);
    assert!(bundle_file.exists());

    // 6. Inspect project bundle
    let inspected = ProjectBundler::inspect_project_bundle(&bundle_file).unwrap();
    assert_eq!(inspected.conversations.len(), 2);
    assert_eq!(inspected.project_name, manifest.project_name);
    assert!(inspected.customizations.has_skills);

    // 7. Test dry-run import
    let target_ag_dir = tempdir().unwrap();
    let target_paths = AntigravityPaths {
        base_dir: target_ag_dir.path().to_path_buf(),
    };
    setup_mock_db(&target_paths.summaries_db());

    let target_ws_dir = tempdir().unwrap();
    let target_ws = clean_path(target_ws_dir.path());

    let dry_run_options = ProjectImportOptions {
        bundle_path: bundle_file.clone(),
        target_workspace: target_ws.clone(),
        overwrite: false,
        dry_run: true,
        restore_customizations: true,
    };

    let (dry_count, _) =
        ProjectImporter::import_project_bundle(&target_paths, &dry_run_options).unwrap();
    assert_eq!(dry_count, 2);
    assert!(!target_ws.join(".agents").exists()); // nothing written in dry run

    // 8. Full Import
    let import_options = ProjectImportOptions {
        bundle_path: bundle_file.clone(),
        target_workspace: target_ws.clone(),
        overwrite: false,
        dry_run: false,
        restore_customizations: true,
    };

    let (imported_count, _) =
        ProjectImporter::import_project_bundle(&target_paths, &import_options).unwrap();
    assert_eq!(imported_count, 2);

    // Verify DB restored
    let target_db = Database::new(&target_paths.summaries_db());
    let list = target_db.list_conversations().unwrap();
    assert_eq!(list.len(), 2);

    let target_ws_str = target_ws.to_string_lossy().to_string();
    let target_uri = to_file_uri(&target_ws);
    for conv in list {
        assert!(conv.workspace_uris.contains(&target_uri));
        assert!(!conv.workspace_uris.contains(&source_uri));
    }

    // Verify brain files restored and sanitized + remapped
    let plan1 = fs::read_to_string(
        target_paths
            .conversation_brain(conv1_id)
            .join("implementation_plan.md"),
    )
    .unwrap();
    assert!(plan1.contains(&format!("{}/src/app.rs", target_uri)));
    assert!(!plan1.contains(&source_uri));
    assert!(!plan1.contains(&source_ws_str));
    // Verify secret sanitization worked
    assert!(plan1.contains("[REDACTED_OPENAI_KEY]"));
    assert!(!plan1.contains("sk-proj-1234567890abcdef12345678"));

    // Verify customizations restored in target workspace
    let restored_skill = target_ws
        .join(".agents")
        .join("skills")
        .join("super-helper")
        .join("SKILL.md");
    assert!(restored_skill.exists());
    let skill_text = fs::read_to_string(restored_skill).unwrap();
    assert!(
        skill_text.contains(&target_ws_str)
            || skill_text.contains(&target_ws_str.replace('\\', "/"))
    );

    assert!(target_ws
        .join(".agents")
        .join("rules")
        .join("rules.md")
        .exists());
    assert!(target_ws.join(".agents").join("mcp_config.json").exists());

    // Verify conversations_fts contains rows with project and workspace
    let conn = Connection::open(&target_paths.summaries_db()).unwrap();
    let mut stmt = conn
        .prepare("SELECT conversation_id, project, workspace, title FROM conversations_fts WHERE chunk_index = -1")
        .unwrap();
    let fts_rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(fts_rows.len(), 2);
    for row in fts_rows {
        assert!(!row.1.is_empty(), "Project must not be empty in FTS");
        assert!(!row.2.is_empty(), "Workspace must not be empty in FTS");
        assert!(!row.3.is_empty(), "Title must not be empty in FTS");
    }

    // Verify ~/.gemini/config/projects/test-proj.json was restored and remapped
    let target_proj_file = target_paths.project_config_file("test-proj");
    assert!(target_proj_file.exists(), "Target project config file must be created on import");
    let proj_content = fs::read_to_string(&target_proj_file).unwrap();
    assert!(proj_content.contains(&target_uri), "Project config must contain remapped target_uri");
    // Verify agyhub_summaries_proto.pb was created and contains the summaries
    let pb_path = target_paths.base_dir.join("agyhub_summaries_proto.pb");
    assert!(pb_path.exists(), "agyhub_summaries_proto.pb must be created on import");
    let pb_data = fs::read(&pb_path).unwrap();
    let (entries, _) = agx::summaries_pb::SummariesProto::parse(&pb_data).unwrap();
    assert_eq!(entries.len(), 2, "Must contain 2 conversation summaries");
    assert!(entries.contains_key(conv1_id));
    assert!(entries.contains_key(conv2_id));

    // 9. Verify safe-forking when destination already has colliding conversation IDs
    let (fork_count, _) =
        ProjectImporter::import_project_bundle(&target_paths, &import_options).unwrap();
    assert_eq!(fork_count, 2);
    let after_fork = target_db.list_conversations().unwrap();
    assert_eq!(after_fork.len(), 4); // 2 original + 2 safely forked with new UUIDs!
    let forked = after_fork
        .iter()
        .filter(|c| c.title.contains("(Imported)"))
        .count();
    assert_eq!(forked, 2);
}
