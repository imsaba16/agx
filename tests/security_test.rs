use agx::bundler::{open_archive, Bundler};
use agx::config::AntigravityPaths;
use agx::db::Database;
use agx::importer::Importer;
use agx::models::{BundleManifest, ConversationSummary, ExportOptions, ImportOptions};
use rusqlite::Connection;
use std::fs::{self, File};
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
        preview: "Audit test preview".to_string(),
        step_count: 5,
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
fn test_security_verification_passed() {
    let source_dir = tempdir().unwrap();
    let source_base = source_dir.path().to_path_buf();
    let source_paths = AntigravityPaths {
        base_dir: source_base.clone(),
    };

    let summaries_db = source_paths.summaries_db();
    setup_mock_db(&summaries_db);

    let conv_id = "test-security-valid";
    let summary = mock_summary(conv_id, "Security Verified Test");

    let db = Database::new(&summaries_db);
    db.upsert_conversation(&summary).unwrap();

    let brain_dir = source_paths.conversation_brain(conv_id);
    fs::create_dir_all(&brain_dir).unwrap();
    fs::write(brain_dir.join("task.md"), "# Secret Task").unwrap();
    fs::write(brain_dir.join("notes.txt"), "Important design decisions").unwrap();

    let conv_db = source_paths.conversation_db(conv_id);
    fs::create_dir_all(conv_db.parent().unwrap()).unwrap();
    let conn = Connection::open(&conv_db).unwrap();
    conn.execute("CREATE TABLE foo (x integer)", []).unwrap();
    drop(conn);

    let output_bundle = source_base.join("valid.agbundle");
    let export_opts = ExportOptions {
        conversation_id: conv_id.to_string(),
        output_path: output_bundle.clone(),
        sanitize: true,
        compact: false,
        workspace_root: None,
        custom_ignore_file: None,
    };

    let manifest = Bundler::export_bundle(&source_paths, summary, &export_opts).unwrap();
    assert!(!manifest.checksums.is_empty(), "Manifest must include cryptographic checksums");

    // Verify bundle
    let report = Bundler::verify_bundle(&output_bundle).unwrap();
    assert!(report.is_valid);
    assert_eq!(report.mismatched_files.len(), 0);
    assert_eq!(report.missing_files.len(), 0);
    assert_eq!(report.errors.len(), 0);
    assert!(report.matched_checksums >= 3);
}

#[test]
fn test_security_tampered_bundle_rejected() {
    let source_dir = tempdir().unwrap();
    let source_base = source_dir.path().to_path_buf();
    let source_paths = AntigravityPaths {
        base_dir: source_base.clone(),
    };

    let summaries_db = source_paths.summaries_db();
    setup_mock_db(&summaries_db);

    let conv_id = "test-security-tamper";
    let summary = mock_summary(conv_id, "Tamper Test");

    let db = Database::new(&summaries_db);
    db.upsert_conversation(&summary).unwrap();

    let brain_dir = source_paths.conversation_brain(conv_id);
    fs::create_dir_all(&brain_dir).unwrap();
    fs::write(brain_dir.join("plan.md"), "# Original Plan Content").unwrap();

    let output_bundle = source_base.join("original.agbundle");
    let export_opts = ExportOptions {
        conversation_id: conv_id.to_string(),
        output_path: output_bundle.clone(),
        sanitize: true,
        compact: false,
        workspace_root: None,
        custom_ignore_file: None,
    };

    Bundler::export_bundle(&source_paths, summary, &export_opts).unwrap();

    // Tamper with the bundle by reading entries via open_archive and replacing plan.md with altered content
    let tampered_bundle = source_base.join("tampered.agbundle");
    {
        let mut archive = open_archive(&output_bundle).unwrap();

        let dst_file = File::create(&tampered_bundle).unwrap();
        let enc = zstd::stream::write::Encoder::new(dst_file, 3).unwrap();
        let mut builder = tar::Builder::new(enc);

        for entry_res in archive.entries().unwrap() {
            let mut entry = entry_res.unwrap();
            let path = entry.path().unwrap().to_path_buf();
            let path_str = path.to_string_lossy();

            if path_str == "brain/plan.md" {
                // Injected malicious/tampered content
                let evil_data = b"# Maliciously Injected Content In Plan";
                let mut header = tar::Header::new_gnu();
                header.set_size(evil_data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, "brain/plan.md", &evil_data[..]).unwrap();
            } else {
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut data).unwrap();
                let mut header = entry.header().clone();
                builder.append_data(&mut header, path, &data[..]).unwrap();
            }
        }
        builder.finish().unwrap();
        builder.into_inner().unwrap().finish().unwrap();
    }

    // Verify report detects the mismatch
    let report = Bundler::verify_bundle(&tampered_bundle).unwrap();
    assert!(!report.is_valid, "Tampered archive must fail integrity check");
    assert!(
        report.mismatched_files.iter().any(|f| f.starts_with("brain/plan.md")),
        "Mismatched files must list brain/plan.md, got {:?}",
        report.mismatched_files
    );

    // Assert that Importer rejects the bundle
    let dest_dir = tempdir().unwrap();
    let dest_paths = AntigravityPaths {
        base_dir: dest_dir.path().to_path_buf(),
    };
    setup_mock_db(&dest_paths.summaries_db());

    let import_opts = ImportOptions {
        bundle_path: tampered_bundle,
        target_workspace: None,
        overwrite: false,
        dry_run: false,
    };

    let result = Importer::import_bundle(&dest_paths, &import_opts);
    assert!(result.is_err(), "Importer must reject tampered bundle");
    let err_str = result.err().unwrap().to_string();
    assert!(err_str.contains("Security check failed") || err_str.contains("Bundle integrity check failed"));
}

#[test]
fn test_security_zip_slip_rejected() {
    let temp = tempdir().unwrap();
    let evil_bundle = temp.path().join("evil_slip.agbundle");

    // Craft a bundle containing a Zip-Slip entry: brain/../../escaped.txt
    {
        let dst_file = File::create(&evil_bundle).unwrap();
        let enc = zstd::stream::write::Encoder::new(dst_file, 3).unwrap();
        let mut builder = tar::Builder::new(enc);

        // Add valid manifest
        let manifest = BundleManifest {
            version: "1.0".to_string(),
            created_at: "2026-10-13 12:00:00".to_string(),
            source_username: "tester".to_string(),
            source_os: "unix".to_string(),
            sanitized: true,
            compact: false,
            conversation: mock_summary("evil-id", "Evil Slip Test"),
            artifacts_count: 1,
            ignored_count: 0,
            has_db: false,
            original_workspace_paths: vec![],
            checksums: std::collections::HashMap::new(),
        };
        let manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();

        let mut manifest_header = tar::Header::new_gnu();
        manifest_header.set_size(manifest_bytes.len() as u64);
        manifest_header.set_mode(0o644);
        manifest_header.set_cksum();
        builder.append_data(&mut manifest_header, "manifest.json", &manifest_bytes[..]).unwrap();

        // Add malicious path traversal file bypassing builder.append_data by setting raw header bytes
        let evil_data = b"malicious payload";
        let mut evil_header = tar::Header::new_gnu();
        evil_header.set_size(evil_data.len() as u64);
        evil_header.set_mode(0o644);
        let path_bytes = b"brain/../../escaped.txt";
        evil_header.as_mut_bytes()[..path_bytes.len()].copy_from_slice(path_bytes);
        evil_header.set_cksum();
        builder.append(&evil_header, &evil_data[..]).unwrap();

        builder.finish().unwrap();
        builder.into_inner().unwrap().finish().unwrap();
    }

    // Verify that Bundler::verify_bundle detects the Zip-Slip
    let report = Bundler::verify_bundle(&evil_bundle).unwrap();
    assert!(!report.is_valid, "Zip-Slip bundle must be marked invalid");
    assert!(report.errors.iter().any(|e| e.contains("Path traversal") || e.contains("Zip-Slip")));

    // Verify that Importer rejects it before extraction
    let dest_paths = AntigravityPaths {
        base_dir: temp.path().join("storage"),
    };
    let import_opts = ImportOptions {
        bundle_path: evil_bundle,
        target_workspace: None,
        overwrite: false,
        dry_run: false,
    };
    let result = Importer::import_bundle(&dest_paths, &import_opts);
    assert!(result.is_err(), "Importer must refuse to unpack zip slip archive");
}

#[test]
fn test_project_security_verification_and_tampering() {
    use agx::models::{ProjectExportOptions, ProjectImportOptions};
    use agx::project_bundler::ProjectBundler;
    use agx::project_importer::ProjectImporter;

    let source_ag_dir = tempdir().unwrap();
    let source_paths = AntigravityPaths {
        base_dir: source_ag_dir.path().to_path_buf(),
    };

    let source_ws_dir = tempdir().unwrap();
    let source_ws = source_ws_dir.path().canonicalize().unwrap();
    let source_ws_str = source_ws.to_string_lossy().to_string();

    let summaries_db = source_paths.summaries_db();
    setup_mock_db(&summaries_db);
    let db = Database::new(&summaries_db);

    let conv_id = "project-sec-conv-1";
    let mut summary = mock_summary(conv_id, "Security Project Conversation");
    summary.workspace_uris = format!(r#"["file://{}"]"#, source_ws_str);
    db.upsert_conversation(&summary).unwrap();

    let brain_dir = source_paths.conversation_brain(conv_id);
    fs::create_dir_all(&brain_dir).unwrap();
    fs::write(brain_dir.join("architecture.md"), "# Architecture Doc").unwrap();

    // Create workspace customization
    let skill_dir = source_ws.join(".agents").join("skills").join("sec-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), "---\nname: sec-skill\n---\nHardened Skill").unwrap();

    let output_bundle = source_ag_dir.path().join("project.agproject");
    let export_opts = ProjectExportOptions {
        workspace_root: source_ws.clone(),
        output_path: output_bundle.clone(),
        session_ids: None,
        include_customizations: true,
        sanitize: true,
        compact: false,
        custom_ignore_file: None,
    };

    let manifest = ProjectBundler::export_project_bundle(&source_paths, &export_opts).unwrap();
    assert!(!manifest.checksums.is_empty(), "Project manifest must contain checksums");

    // 1. Verify valid project bundle
    let report = ProjectBundler::verify_project_bundle(&output_bundle).unwrap();
    assert!(report.is_valid, "Valid project bundle must pass verification");
    assert!(report.matched_checksums >= 2);
    assert_eq!(report.mismatched_files.len(), 0);

    // 2. Tamper with project archive by replacing customization file
    let tampered_project = source_ag_dir.path().join("tampered_project.agproject");
    {
        let mut archive = open_archive(&output_bundle).unwrap();

        let dst_file = File::create(&tampered_project).unwrap();
        let enc = zstd::stream::write::Encoder::new(dst_file, 3).unwrap();
        let mut builder = tar::Builder::new(enc);

        for entry_res in archive.entries().unwrap() {
            let mut entry = entry_res.unwrap();
            let path = entry.path().unwrap().to_path_buf();
            let path_str = path.to_string_lossy().replace('\\', "/");

            if path_str.ends_with("SKILL.md") {
                let evil_data = b"---\nname: malicious-skill\n---\nCompromised Skill";
                let mut header = tar::Header::new_gnu();
                header.set_size(evil_data.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append_data(&mut header, path, &evil_data[..]).unwrap();
            } else {
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut data).unwrap();
                let mut header = entry.header().clone();
                builder.append_data(&mut header, path, &data[..]).unwrap();
            }
        }
        builder.finish().unwrap();
        builder.into_inner().unwrap().finish().unwrap();
    }

    // 3. Verify tampering detection
    let tampered_report = ProjectBundler::verify_project_bundle(&tampered_project).unwrap();
    assert!(!tampered_report.is_valid, "Tampered project must fail verification");
    assert!(
        tampered_report.mismatched_files.iter().any(|f| f.contains("SKILL.md")),
        "Mismatched files must list SKILL.md, got {:?}",
        tampered_report.mismatched_files
    );

    // 4. Assert ProjectImporter rejects the tampered archive
    let dest_ag_dir = tempdir().unwrap();
    let dest_paths = AntigravityPaths {
        base_dir: dest_ag_dir.path().to_path_buf(),
    };
    setup_mock_db(&dest_paths.summaries_db());
    let dest_ws_dir = tempdir().unwrap();

    let import_opts = ProjectImportOptions {
        bundle_path: tampered_project,
        target_workspace: dest_ws_dir.path().to_path_buf(),
        overwrite: false,
        dry_run: false,
        restore_customizations: true,
    };

    let import_res = ProjectImporter::import_project_bundle(&dest_paths, &import_opts);
    assert!(import_res.is_err(), "ProjectImporter must reject tampered bundle");
}

