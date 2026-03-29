use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
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
    let dir = std::env::temp_dir().join(format!("witness-{name}-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_engine(args: &[&str]) -> Output {
    Command::new(engine_bin())
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap()
}

fn run_scan_file(relative: &str) -> Output {
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

fn violation_count(value: &Value) -> u64 {
    value["summary"]["violation_count"].as_u64().unwrap_or(0)
}

#[test]
fn python_get_default_is_blocked() {
    let output = run_scan_file("fixtures/python/fallback/should_fail/get_default.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(violation_count(&value), 1);
    assert_eq!(
        value["violations"][0]["rule_id"],
        "py-no-fallback-get-default"
    );
    assert_eq!(value["violations"][0]["owner_guess"], "boundary");
}

#[test]
fn python_equivalent_rewrite_is_blocked() {
    let output =
        run_scan_file("fixtures/python/fallback/should_fail/equivalent_membership_rewrite.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["violations"][0]["rule_id"],
        "py-no-fallback-conditional-membership-default"
    );
    assert!(
        value["capsule"]
            .as_str()
            .unwrap()
            .contains("equivalent_rewrite")
    );
}

#[test]
fn python_except_return_default_is_blocked() {
    let output = run_scan_file("fixtures/python/fallback/should_fail/except_return_default.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["violations"][0]["rule_id"],
        "py-no-fallback-except-return-default"
    );
}

#[test]
fn python_registered_approval_suppresses_finding() {
    let output = run_scan_file("fixtures/python/fallback/approved/approved_default.py");
    assert!(output.status.success());
    let value = stdout_json(&output);
    assert_eq!(violation_count(&value), 0);
}

#[test]
fn python_invalid_approval_is_reported() {
    let output = run_scan_file("fixtures/python/fallback/approved/invalid_approval.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["violations"][0]["approval_status"], "invalid");
}

#[test]
fn python_runtime_double_is_blocked() {
    let output = run_scan_file("fixtures/python/test_double/should_fail/runtime_fake.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["violations"][0]["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn python_test_support_import_is_blocked() {
    let output =
        run_scan_file("fixtures/python/test_double/should_fail/runtime_test_support_import.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["violations"][0]["rule_id"],
        "py-no-test-support-import"
    );
}

#[test]
fn python_adapter_choice_outside_composition_root_is_blocked() {
    let output = run_scan_file("fixtures/python/architecture/should_fail/adapter_choice.py");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["violations"][0]["violation_class"],
        "adapter_choice_outside_composition_root"
    );
}

#[test]
fn typescript_nullish_is_blocked() {
    let output = run_scan_file("fixtures/typescript/fallback/should_fail/nullish.ts");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["violations"][0]["rule_id"], "ts-no-fallback-nullish");
}

#[test]
fn typescript_equivalent_rewrite_is_blocked() {
    let output = run_scan_file("fixtures/typescript/fallback/should_fail/lookup_else_default.ts");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["violations"][0]["rule_id"],
        "ts-no-fallback-lookup-else-default"
    );
}

#[test]
fn typescript_promise_catch_default_is_blocked() {
    let output = run_scan_file("fixtures/typescript/fallback/should_fail/promise_catch_default.ts");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["violations"][0]["rule_id"],
        "ts-no-promise-catch-default"
    );
}

#[test]
fn typescript_runtime_double_is_blocked() {
    let output = run_scan_file("fixtures/typescript/test_double/should_fail/runtime_fake.ts");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(
        value["violations"][0]["violation_class"],
        "runtime_double_in_graph"
    );
}

#[test]
fn typescript_registered_approval_suppresses_finding() {
    let output = run_scan_file("fixtures/typescript/fallback/approved/approved_default.ts");
    assert!(output.status.success());
    let value = stdout_json(&output);
    assert_eq!(violation_count(&value), 0);
}

#[test]
fn typescript_invalid_approval_is_reported() {
    let output = run_scan_file("fixtures/typescript/fallback/approved/invalid_approval.ts");
    assert!(!output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["violations"][0]["approval_status"], "invalid");
}

#[test]
fn hook_response_is_compact_and_persists_report() {
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
    assert!(capsule.contains("guardrail count=1"));
    assert!(!capsule.contains("toolUseId"));

    let pending_dir = report_dir.join("pending");
    let entries: Vec<_> = fs::read_dir(pending_dir).unwrap().collect();
    assert_eq!(entries.len(), 1);
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
    assert!(
        value["reason"]
            .as_str()
            .unwrap()
            .contains("unresolved pending report")
    );
}
