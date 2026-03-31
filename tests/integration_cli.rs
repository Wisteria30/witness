use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread::sleep;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn engine_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_witness-engine"))
}

fn fixture_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("witness-v3-{name}-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_engine_in(cwd: &Path, args: &[&str]) -> Output {
    Command::new(engine_bin())
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap()
}

fn run_engine(args: &[&str]) -> Output {
    run_engine_in(&repo_root(), args)
}

fn run_repo_scan_file(relative: &str) -> Output {
    let fixture = fixture_path(relative);
    let fixture_str = fixture.to_string_lossy().to_string();
    let config_str = repo_root().to_string_lossy().to_string();
    run_engine(&[
        "scan-file",
        "--file",
        &fixture_str,
        "--config-dir",
        &config_str,
    ])
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn summary_count(value: &Value, key: &str) -> u64 {
    value["summary"][key].as_u64().unwrap_or(0)
}

fn first_finding(value: &Value) -> &Value {
    &value["findings"][0]
}

fn finding_by_kind<'a>(value: &'a Value, kind: &str) -> &'a Value {
    value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["kind"] == kind)
        .unwrap()
}

fn finding_by_rule<'a>(value: &'a Value, rule_id: &str) -> &'a Value {
    value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["rule_id"] == rule_id)
        .unwrap()
}

fn write_file(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn write_minimal_project(root: &Path) {
    write_file(root, "sgconfig.yml", "ruleDirs:\n  - rules\n");
    write_file(
        root,
        "policy/ownership.yml",
        "layers:\n  boundary:\n    - \"src/api/**\"\n",
    );
    write_file(root, "policy/defaults.yml", "defaults: {}\n");
    write_file(root, "policy/adapters.yml", "ports: {}\n");
    write_file(
        root,
        "policy/surfaces.yml",
        r#"
public_by_default:
  concept_patterns:
    - "*Payload"
    - "*Policy"
    - "*Adapter"
extension_api_patterns:
  - "*Base"
rules:
  forbid_restricted_visibility_for_public_concepts: true
  require_explicit_export_manifest_for_new_public_symbols: true
"#,
    );
    write_file(
        root,
        "policy/contexts.yml",
        r#"
contexts:
  api_boundary:
    paths:
      - "src/api/**"
    vocabulary:
      nouns:
        - Payload
        - Request
        - Response
        - Parser
      verbs:
        - parse
        - validate
    may_depend_on: []
    public_entrypoints:
      - "src/api/__init__.py"
"#,
    );
    write_file(root, "policy/contracts.yml", "contracts: {}\n");
    fs::create_dir_all(root.join("rules")).unwrap();
}

fn active_charter_dir(root: &Path) -> PathBuf {
    root.join(".witness-data/charters/active")
}

fn report_dir(root: &Path) -> PathBuf {
    root.join(".witness-data/reports")
}

fn write_active_charter(root: &Path, name: &str, contents: &str) {
    write_file(
        root,
        &format!(".witness-data/charters/active/{name}.yml"),
        contents,
    );
}

fn write_pending_report(root: &Path, canonical_file: &str, charter_ref: &str) {
    let pending_path = report_dir(root).join("pending/report.json");
    if let Some(parent) = pending_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let report = json!({
        "version": 3,
        "report_id": "wg-test-0001",
        "created_at": "2026-04-01T00:00:00Z",
        "status": "pending",
        "charter_ref": charter_ref,
        "file": "src/api/tool_use.py",
        "canonical_file": canonical_file,
        "summary": {
            "files_scanned": 1,
            "violations": 0,
            "holes": 0,
            "drift": 0,
            "obligations": 1,
            "by_kind": {
                "obligation": 1
            },
            "by_file": {
                "src/api/tool_use.py": 1
            }
        },
        "findings": [
            {
                "kind": "obligation",
                "file": "src/api/tool_use.py",
                "canonical_file": canonical_file,
                "snippet": "",
                "message": "Pending constitutional work remains"
            }
        ]
    });
    fs::write(
        pending_path,
        format!("{}\n", serde_json::to_string_pretty(&report).unwrap()),
    )
    .unwrap();
}

#[test]
fn python_get_default_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/python/fallback/should_fail/get_default.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "violations"), 1);
    assert_eq!(first_finding(&value)["kind"], "violation");
    assert_eq!(
        finding_by_rule(&value, "py-no-fallback-get-default")["owner_layer"],
        "boundary"
    );
}

#[test]
fn python_registered_approval_with_blessed_symbol_is_clean() {
    let output = run_repo_scan_file("fixtures/python/fallback/approved/approved_default.py");
    assert!(output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "violations"), 0);
    assert!(value["findings"].as_array().unwrap().is_empty());
}

#[test]
fn python_invalid_approval_is_still_violation() {
    let output = run_repo_scan_file("fixtures/python/fallback/approved/invalid_approval.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "violations"), 1);
    assert!(
        first_finding(&value)["message"]
            .as_str()
            .unwrap()
            .contains("not registered")
    );
}

#[test]
fn python_runtime_double_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/python/test_double/should_fail/runtime_fake.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "py-no-test-double-identifier")["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn typescript_nullish_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/typescript/fallback/should_fail/nullish.ts");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "ts-no-fallback-nullish")["kind"],
        "violation"
    );
    assert_eq!(summary_count(&value, "violations"), 1);
}

#[test]
fn tsx_nullish_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/typescript/fallback/should_fail/nullish.tsx");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "ts-no-fallback-nullish")["kind"],
        "violation"
    );
    assert_eq!(summary_count(&value, "violations"), 1);
}

#[test]
fn tsx_lookup_else_default_is_violation_v3() {
    let output =
        run_repo_scan_file("fixtures/typescript/fallback/should_fail/lookup_else_default.tsx");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "ts-no-fallback-lookup-else-default")["kind"],
        "violation"
    );
}

#[test]
fn tsx_promise_catch_default_is_violation_v3() {
    let output =
        run_repo_scan_file("fixtures/typescript/fallback/should_fail/promise_catch_default.tsx");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "ts-no-promise-catch-default")["kind"],
        "violation"
    );
}

#[test]
fn tsx_runtime_fake_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/typescript/test_double/should_fail/runtime_fake.tsx");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "ts-no-test-double-identifier")["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn tsx_runtime_test_support_import_is_violation_v3() {
    let output = run_repo_scan_file(
        "fixtures/typescript/test_double/should_fail/runtime_test_support_import.tsx",
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "ts-no-test-support-import")["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn tsx_hidden_owner_concept_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/typescript/fallback/should_fail/hidden_payload.tsx");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "violations"), 1);
    assert_eq!(
        finding_by_rule(&value, "ts-no-hidden-owner-concept")["violation_class"],
        "surface_hidden_owner_concept"
    );
}

#[test]
fn go_getenv_default_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/go/fallback/should_fail/getenv_default.go");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "go-no-fallback-getenv-default")["kind"],
        "violation"
    );
}

#[test]
fn go_lookupenv_default_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/go/fallback/should_fail/lookupenv_default.go");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "go-no-fallback-lookupenv-default")["kind"],
        "violation"
    );
}

#[test]
fn go_error_default_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/go/fallback/should_fail/error_return_default.go");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "go-no-swallowing-error-return-default")["kind"],
        "violation"
    );
}

#[test]
fn go_empty_error_branch_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/go/fallback/should_fail/error_empty_branch.go");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "go-no-swallowing-error-empty-branch")["kind"],
        "violation"
    );
}

#[test]
fn go_runtime_fake_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/go/test_double/should_fail/runtime_fake.go");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "go-no-test-double-identifier")["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn go_runtime_test_support_import_is_violation_v3() {
    let output =
        run_repo_scan_file("fixtures/go/test_double/should_fail/runtime_test_support_import.go");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "go-no-test-support-import")["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn go_hidden_owner_concept_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/go/fallback/should_fail/hidden_payload.go");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "go-no-hidden-owner-concept")["violation_class"],
        "surface_hidden_owner_concept"
    );
}

#[test]
fn rust_unwrap_or_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/rust/fallback/should_fail/unwrap_or.rs");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-fallback-unwrap-or")["kind"],
        "violation"
    );
}

#[test]
fn rust_unwrap_or_default_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/rust/fallback/should_fail/unwrap_or_default.rs");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-fallback-unwrap-or-default")["kind"],
        "violation"
    );
}

#[test]
fn rust_map_or_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/rust/fallback/should_fail/map_or.rs");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-fallback-map-or")["kind"],
        "violation"
    );
}

#[test]
fn rust_match_default_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/rust/fallback/should_fail/match_default.rs");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-swallowing-error-match-default")["kind"],
        "violation"
    );
}

#[test]
fn rust_if_let_default_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/rust/fallback/should_fail/if_let_default.rs");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-swallowing-error-if-let")["kind"],
        "violation"
    );
}

#[test]
fn rust_runtime_fake_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/rust/test_double/should_fail/runtime_fake.rs");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-test-double-identifier")["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn rust_runtime_test_support_import_is_violation_v3() {
    let output =
        run_repo_scan_file("fixtures/rust/test_double/should_fail/runtime_test_support_import.rs");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-test-support-import")["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn rust_hidden_owner_concept_is_violation_v3() {
    let output = run_repo_scan_file("fixtures/rust/fallback/should_fail/hidden_payload.rs");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-hidden-owner-concept")["violation_class"],
        "surface_hidden_owner_concept"
    );
}

#[test]
fn hook_response_is_compact_and_persists_v3_report() {
    let report_dir = unique_temp_dir("reports");
    let fixture = fixture_path("fixtures/python/fallback/should_fail/get_default.py");
    let fixture_str = fixture.to_string_lossy().to_string();
    let config_str = repo_root().to_string_lossy().to_string();
    let report_str = report_dir.to_string_lossy().to_string();

    let output = run_engine(&[
        "scan-file",
        "--file",
        &fixture_str,
        "--config-dir",
        &config_str,
        "--report-dir",
        &report_str,
        "--hook-response",
    ]);

    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["decision"], "block");
    let capsule = value["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(capsule.contains("witness count=1"));

    let pending_dir = report_dir.join("pending");
    let entries: Vec<_> = fs::read_dir(&pending_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);

    let path = entries[0].as_ref().unwrap().path();
    let report: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(report["version"], 3);
    assert_eq!(report["status"], "pending");
    assert_eq!(report["summary"]["violations"], 1);
}

#[test]
fn history_reports_are_append_only() {
    let report_dir = unique_temp_dir("history");
    let fixture = fixture_path("fixtures/python/fallback/should_fail/get_default.py");
    let fixture_str = fixture.to_string_lossy().to_string();
    let config_str = repo_root().to_string_lossy().to_string();
    let report_str = report_dir.to_string_lossy().to_string();

    let first = run_engine(&[
        "scan-file",
        "--file",
        &fixture_str,
        "--config-dir",
        &config_str,
        "--report-dir",
        &report_str,
    ]);
    assert!(!first.status.success());

    sleep(Duration::from_millis(2));

    let second = run_engine(&[
        "scan-file",
        "--file",
        &fixture_str,
        "--config-dir",
        &config_str,
        "--report-dir",
        &report_str,
    ]);
    assert!(!second.status.success());

    let history_dir = report_dir.join("history");
    let mut entries: Vec<_> = fs::read_dir(&history_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    entries.sort();
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0].file_name(), entries[1].file_name());
}

#[test]
fn scan_stop_blocks_when_pending_reports_exist() {
    let report_dir = unique_temp_dir("stop");
    let fixture = fixture_path("fixtures/python/fallback/should_fail/get_default.py");
    let fixture_str = fixture.to_string_lossy().to_string();
    let config_str = repo_root().to_string_lossy().to_string();
    let report_str = report_dir.to_string_lossy().to_string();

    let first = run_engine(&[
        "scan-file",
        "--file",
        &fixture_str,
        "--config-dir",
        &config_str,
        "--report-dir",
        &report_str,
    ]);
    assert!(!first.status.success());

    let stop = run_engine(&[
        "scan-stop",
        "--config-dir",
        &config_str,
        "--report-dir",
        &report_str,
        "--hook-response",
    ]);
    assert!(!stop.status.success());
    let value = stdout_json(&stop);
    assert_eq!(value["decision"], "block");
    assert!(value["reason"].as_str().unwrap().contains("violations=1"));
}

#[test]
fn scan_stop_rejects_unsupported_pending_report_version() {
    let report_dir = unique_temp_dir("unsupported-report");
    write_file(
        &report_dir,
        "pending/legacy.json",
        r#"{"version":1,"report_id":"legacy"}"#,
    );
    let config_str = repo_root().to_string_lossy().to_string();
    let report_str = report_dir.to_string_lossy().to_string();

    let output = run_engine(&[
        "scan-stop",
        "--config-dir",
        &config_str,
        "--report-dir",
        &report_str,
    ]);
    assert!(!output.status.success());
    assert!(stderr_text(&output).contains("unsupported pending report schema"));
}

#[test]
fn hidden_owner_concept_is_violation() {
    let root = unique_temp_dir("hidden-owner");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class _ToolUsePayload:\n    pass\n",
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "violations"), 1);
    assert_eq!(
        finding_by_rule(&value, "py-no-hidden-owner-concept")["violation_class"],
        "surface_hidden_owner_concept"
    );
}

#[test]
fn bundled_policy_is_used_when_repo_has_no_policy_dir() {
    let root = unique_temp_dir("bundled-policy");
    write_file(
        &root,
        "src/api/tool_use.py",
        "class _ToolUsePayload:\n    pass\n",
    );

    let file = root.join("src/api/tool_use.py");
    let config = repo_root().to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "violations"), 1);
    assert_eq!(
        finding_by_rule(&value, "py-no-hidden-owner-concept")["violation_class"],
        "surface_hidden_owner_concept"
    );
}

#[test]
fn repo_policy_file_overrides_bundled_default() {
    let root = unique_temp_dir("policy-override");
    write_file(
        &root,
        "policy/surfaces.yml",
        r#"
rules:
  forbid_restricted_visibility_for_public_concepts: false
  require_explicit_export_manifest_for_new_public_symbols: false
"#,
    );
    write_file(
        &root,
        "src/api/tool_use.py",
        "class _ToolUsePayload:\n    pass\n",
    );

    let file = root.join("src/api/tool_use.py");
    let config = repo_root().to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(output.status.success());
    let value = stdout_json(&output);
    assert!(value["findings"].as_array().unwrap().is_empty());
}

#[test]
fn scan_tree_catches_structural_v3_findings() {
    let root = unique_temp_dir("scan-tree-structure");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class _ToolUsePayload:\n    pass\n",
    );

    let config = root.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-tree", "--root", &config, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["summary"]["files_scanned"], 1);
    assert_eq!(summary_count(&value, "violations"), 1);
    assert_eq!(
        finding_by_rule(&value, "py-no-hidden-owner-concept")["violation_class"],
        "surface_hidden_owner_concept"
    );
}

#[test]
fn scan_tree_discovers_go_and_rust_sources() {
    let root = unique_temp_dir("scan-tree-go-rust");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.go",
        "package api\n\ntype toolUsePayload struct{}\n",
    );
    write_file(&root, "src/api/tool_use.rs", "struct ToolUsePayload;\n");

    let config = root.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-tree", "--root", &config, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["summary"]["files_scanned"], 2);
    assert_eq!(
        finding_by_rule(&value, "go-no-hidden-owner-concept")["violation_class"],
        "surface_hidden_owner_concept"
    );
    assert_eq!(
        finding_by_rule(&value, "rs-no-hidden-owner-concept")["violation_class"],
        "surface_hidden_owner_concept"
    );
}

#[test]
fn boundary_parser_without_contract_is_hole() {
    let root = unique_temp_dir("boundary-hole");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "from pydantic import BaseModel\n\nclass ToolUsePayload(BaseModel):\n    toolUseId: str\n",
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert!(summary_count(&value, "holes") >= 1);
    assert!(
        finding_by_kind(&value, "hole")["message"]
            .as_str()
            .unwrap()
            .contains("contract witness")
    );
}

#[test]
fn go_boundary_parser_without_contract_is_hole() {
    let root = unique_temp_dir("boundary-hole-go");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.go",
        r#"package api

import (
	"encoding/json"
	"net/http"
)

type ToolUsePayload struct {
	ToolUseID string `json:"toolUseId"`
}

func ParseToolUse(r *http.Request) ToolUsePayload {
	var payload ToolUsePayload
	_ = json.NewDecoder(r.Body).Decode(&payload)
	return payload
}
"#,
    );

    let file = root.join("src/api/tool_use.go");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert!(summary_count(&value, "holes") >= 1);
    assert!(
        finding_by_kind(&value, "hole")["message"]
            .as_str()
            .unwrap()
            .contains("contract witness")
    );
}

#[test]
fn rust_boundary_parser_without_contract_is_hole() {
    let root = unique_temp_dir("boundary-hole-rust");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.rs",
        r#"use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ToolUsePayload {
    tool_use_id: String,
}

pub async fn parse_tool_use(payload: Json<ToolUsePayload>) -> String {
    payload.0.tool_use_id
}
"#,
    );

    let file = root.join("src/api/tool_use.rs");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert!(summary_count(&value, "holes") >= 1);
    assert!(
        finding_by_kind(&value, "hole")["message"]
            .as_str()
            .unwrap()
            .contains("contract witness")
    );
}

#[test]
fn go_adapter_choice_outside_composition_root_is_violation() {
    let root = unique_temp_dir("adapter-go");
    write_minimal_project(&root);
    write_file(
        &root,
        "policy/adapters.yml",
        r#"
ports:
  UserRepository:
    allowed_runtime_adapters:
      - SqlUserRepository
"#,
    );
    write_file(
        &root,
        "src/api/tool_use.go",
        r#"package api

func BuildRepository() any {
	return NewSqlUserRepository()
}
"#,
    );

    let file = root.join("src/api/tool_use.go");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "go-no-adapter-choice-outside-composition-root")["violation_class"],
        "adapter_choice_outside_composition_root"
    );
}

#[test]
fn rust_adapter_choice_outside_composition_root_is_violation() {
    let root = unique_temp_dir("adapter-rust");
    write_minimal_project(&root);
    write_file(
        &root,
        "policy/adapters.yml",
        r#"
ports:
  UserRepository:
    allowed_runtime_adapters:
      - SqlUserRepository
"#,
    );
    write_file(
        &root,
        "src/api/tool_use.rs",
        r#"pub fn build_repository() -> SqlUserRepository {
    SqlUserRepository::new()
}
"#,
    );

    let file = root.join("src/api/tool_use.rs");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        finding_by_rule(&value, "rs-no-adapter-choice-outside-composition-root")["violation_class"],
        "adapter_choice_outside_composition_root"
    );
}

#[test]
fn rust_cfg_test_module_is_ignored_for_runtime_double_rules() {
    let root = unique_temp_dir("rust-cfg-test");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.rs",
        r#"#[cfg(test)]
mod tests {
    use mockall::predicate::*;

    #[test]
    fn allows_test_only_mock_usage() {
        let fake_repo = 1;
        assert!(eq(1).eval(&fake_repo));
    }
}
"#,
    );

    let file = root.join("src/api/tool_use.rs");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(output.status.success());
    let value = stdout_json(&output);
    assert!(value["findings"].as_array().unwrap().is_empty());
}

#[test]
fn charter_public_symbol_without_export_is_obligation() {
    let root = unique_temp_dir("charter-obligation");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_file(
        &root,
        ".witness-data/charters/active/change.yml",
        r#"
version: 1
change_id: CHG-1
constitution_mode: extend
surfaces:
  public_symbols:
    src/api/tool_use.py:
      ToolUsePayload: public_concept
contexts:
  assignments:
    src/api/tool_use.py: api_boundary
contracts:
  add: []
defaults:
  approvals: []
adapters:
  add: []
holes: []
"#,
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let charter_dir = root.join(".witness-data/charters/active");
    let charter_str = charter_dir.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "scan-file",
            "--file",
            &file_str,
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
        ],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "obligations"), 1);
    assert!(
        finding_by_kind(&value, "obligation")["message"]
            .as_str()
            .unwrap()
            .contains("export witness")
    );
}

#[test]
fn overlapping_context_paths_become_hole() {
    let root = unique_temp_dir("context-hole");
    write_minimal_project(&root);
    write_file(
        &root,
        "policy/contexts.yml",
        r#"
contexts:
  api_boundary:
    paths:
      - "src/api/**"
    vocabulary:
      nouns:
        - Payload
      verbs:
        - parse
  ordering:
    paths:
      - "src/api/**"
    vocabulary:
      nouns:
        - Order
      verbs:
        - place
"#,
    );
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert!(summary_count(&value, "holes") >= 1);
    assert!(
        finding_by_kind(&value, "hole")["message"]
            .as_str()
            .unwrap()
            .contains("Multiple bounded contexts")
    );
}

#[test]
fn charter_hole_is_reported() {
    let root = unique_temp_dir("charter-hole");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_active_charter(
        &root,
        "hole",
        r#"
version: 1
change_id: CHG-HOLE
constitution_mode: extend
surfaces:
  public_symbols: {}
contexts:
  assignments: {}
contracts:
  add: []
defaults:
  approvals: []
adapters:
  add: []
holes:
  - kind: context
    question: Which bounded context owns ToolUsePayload?
"#,
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "scan-file",
            "--file",
            &file_str,
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
        ],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert!(summary_count(&value, "holes") >= 1);
    assert!(value["findings"].as_array().unwrap().iter().any(|finding| {
        finding["kind"] == "hole"
            && finding["message"]
                .as_str()
                .unwrap_or_default()
                .contains("Unresolved charter hole")
    }));
}

#[test]
fn charter_contract_without_compile_is_obligation() {
    let root = unique_temp_dir("charter-contract-uncompiled");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_active_charter(
        &root,
        "contract",
        r#"
version: 1
change_id: CHG-CONTRACT
constitution_mode: extend
surfaces:
  public_symbols: {}
contexts:
  assignments: {}
contracts:
  add:
    - id: http.new_payload.v1
      kind: shape
      compatibility: exact
defaults:
  approvals: []
adapters:
  add: []
holes: []
"#,
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "scan-file",
            "--file",
            &file_str,
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
        ],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "obligations"), 1);
    assert!(
        finding_by_kind(&value, "obligation")["message"]
            .as_str()
            .unwrap()
            .contains("policy/contracts.yml has not been updated")
    );
}

#[test]
fn charter_contract_missing_schema_is_obligation() {
    let root = unique_temp_dir("charter-contract-schema");
    write_minimal_project(&root);
    write_file(
        &root,
        "policy/contracts.yml",
        r#"
contracts:
  http.tool_use_payload.v1:
    kind: shape
    context: api_boundary
    owner_layer: boundary
    schema: schemas/http/tool_use_payload.v1.json
    compatibility: exact
    witnesses:
      - tests/contracts/http/test_tool_use_payload_schema.py
"#,
    );
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_active_charter(
        &root,
        "contract",
        r#"
version: 1
change_id: CHG-CONTRACT-SCHEMA
constitution_mode: extend
surfaces:
  public_symbols: {}
contexts:
  assignments: {}
contracts:
  add:
    - id: http.tool_use_payload.v1
      kind: shape
      compatibility: exact
defaults:
  approvals: []
adapters:
  add: []
holes: []
"#,
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "scan-file",
            "--file",
            &file_str,
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
        ],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert!(summary_count(&value, "obligations") >= 1);
    assert!(value["findings"].as_array().unwrap().iter().any(|finding| {
        finding["kind"] == "obligation"
            && finding["message"]
                .as_str()
                .unwrap_or_default()
                .contains("requires schema")
    }));
}

#[test]
fn charter_defaults_approval_without_compile_is_obligation() {
    let root = unique_temp_dir("charter-defaults");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_active_charter(
        &root,
        "defaults",
        r#"
version: 1
change_id: CHG-DEFAULT
constitution_mode: extend
surfaces:
  public_symbols: {}
contexts:
  assignments: {}
contracts:
  add: []
defaults:
  approvals:
    - REQ-999
adapters:
  add: []
holes: []
"#,
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "scan-file",
            "--file",
            &file_str,
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
        ],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "obligations"), 1);
    assert!(
        finding_by_kind(&value, "obligation")["message"]
            .as_str()
            .unwrap()
            .contains("policy/defaults.yml")
    );
}

#[test]
fn charter_adapter_without_compile_is_obligation() {
    let root = unique_temp_dir("charter-adapter");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_active_charter(
        &root,
        "adapter",
        r#"
version: 1
change_id: CHG-ADAPTER
constitution_mode: extend
surfaces:
  public_symbols: {}
contexts:
  assignments: {}
contracts:
  add: []
defaults:
  approvals: []
adapters:
  add:
    - RedisUserRepository
holes: []
"#,
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "scan-file",
            "--file",
            &file_str,
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
        ],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(summary_count(&value, "obligations"), 1);
    assert!(
        finding_by_kind(&value, "obligation")["message"]
            .as_str()
            .unwrap()
            .contains("policy/adapters.yml")
    );
}

#[test]
fn charter_context_mismatch_is_drift() {
    let root = unique_temp_dir("charter-drift");
    write_minimal_project(&root);
    write_file(
        &root,
        "policy/contexts.yml",
        r#"
contexts:
  api_boundary:
    paths:
      - "src/api/**"
    vocabulary:
      nouns:
        - Payload
      verbs:
        - parse
  ordering:
    paths:
      - "src/domain/ordering/**"
    vocabulary:
      nouns:
        - Order
      verbs:
        - place
"#,
    );
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_active_charter(
        &root,
        "context",
        r#"
version: 1
change_id: CHG-DRIFT
constitution_mode: extend
surfaces:
  public_symbols: {}
contexts:
  assignments:
    src/api/tool_use.py: ordering
contracts:
  add: []
defaults:
  approvals: []
adapters:
  add: []
holes: []
"#,
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "scan-file",
            "--file",
            &file_str,
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
        ],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert!(summary_count(&value, "drift") >= 1);
    assert!(value["findings"].as_array().unwrap().iter().any(|finding| {
        finding["kind"] == "drift"
            && finding["message"]
                .as_str()
                .unwrap_or_default()
                .contains("Charter assigns")
    }));
}

#[test]
fn public_symbol_without_export_and_without_charter_is_drift() {
    let root = unique_temp_dir("public-symbol-drift");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );

    let file = root.join("src/api/tool_use.py");
    let config = root.to_string_lossy().to_string();
    let file_str = file.to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &["scan-file", "--file", &file_str, "--config-dir", &config],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert!(summary_count(&value, "drift") >= 1);
    assert!(value["findings"].as_array().unwrap().iter().any(|finding| {
        finding["kind"] == "drift"
            && finding["message"]
                .as_str()
                .unwrap_or_default()
                .contains("missing an explicit export witness")
    }));
}

#[test]
fn retire_charter_archives_when_clean() {
    let root = unique_temp_dir("retire-clean");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_active_charter(
        &root,
        "change",
        r#"
version: 1
change_id: CHG-RETIRE
constitution_mode: extend
surfaces:
  public_symbols: {}
contexts:
  assignments: {}
contracts:
  add: []
defaults:
  approvals: []
adapters:
  add: []
holes: []
"#,
    );

    let config = root.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let report_str = report_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "retire-charters",
            "--change-id",
            "CHG-RETIRE",
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
            "--report-dir",
            &report_str,
        ],
    );
    assert!(output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["clean"], true);
    assert_eq!(value["archived"][0], "CHG-RETIRE");
    assert!(!active_charter_dir(&root).join("change.yml").exists());
    let history_dir = root.join(".witness-data/charters/history");
    let entries: Vec<_> = fs::read_dir(&history_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);
}

#[test]
fn retire_charter_blocks_when_pending_report_references_change_id() {
    let root = unique_temp_dir("retire-blocked");
    write_minimal_project(&root);
    write_file(
        &root,
        "src/api/tool_use.py",
        "class ToolUsePayload:\n    pass\n",
    );
    write_active_charter(
        &root,
        "change",
        r#"
version: 1
change_id: CHG-RETIRE
constitution_mode: extend
surfaces:
  public_symbols: {}
contexts:
  assignments: {}
contracts:
  add: []
defaults:
  approvals: []
adapters:
  add: []
holes: []
"#,
    );
    write_pending_report(
        &root,
        &root.join("src/api/tool_use.py").to_string_lossy(),
        "CHG-RETIRE",
    );

    let config = root.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let report_str = report_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "retire-charters",
            "--change-id",
            "CHG-RETIRE",
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
            "--report-dir",
            &report_str,
        ],
    );
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["clean"], false);
    assert!(
        value["skipped"][0]["reason"]
            .as_str()
            .unwrap()
            .contains("unresolved pending report")
    );
    assert!(active_charter_dir(&root).join("change.yml").exists());
}

#[test]
fn retire_charters_requires_change_id_argument() {
    let root = unique_temp_dir("retire-missing-change-id");
    write_minimal_project(&root);
    let config = root.to_string_lossy().to_string();
    let charter_str = active_charter_dir(&root).to_string_lossy().to_string();
    let report_str = report_dir(&root).to_string_lossy().to_string();
    let output = run_engine_in(
        &root,
        &[
            "retire-charters",
            "--config-dir",
            &config,
            "--charter-dir",
            &charter_str,
            "--report-dir",
            &report_str,
        ],
    );
    assert!(!output.status.success());
    assert!(stderr_text(&output).contains("retire-charters requires at least one --change-id"));
}
