use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread::sleep;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

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
