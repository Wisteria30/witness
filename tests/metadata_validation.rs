use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use jsonschema::{Draft, JSONSchema};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn cargo_version() -> String {
    let text = fs::read_to_string(repo_root().join("Cargo.toml")).unwrap();
    let re = Regex::new(r#"(?m)^version = "([^"]+)"$"#).unwrap();
    re.captures(&text)
        .expect("could not read version from Cargo.toml")
        .get(1)
        .unwrap()
        .as_str()
        .to_string()
}

fn json_version(path: &str) -> String {
    let text = fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|_| panic!("could not read {path}"));
    let value: Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("invalid JSON: {path}"));
    value["version"]
        .as_str()
        .unwrap_or_else(|| panic!("no version in {path}"))
        .to_string()
}

fn marketplace_version() -> String {
    let path = ".claude-plugin/marketplace.json";
    let text = fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|_| panic!("could not read {path}"));
    let value: Value =
        serde_json::from_str(&text).unwrap_or_else(|_| panic!("invalid JSON: {path}"));
    let plugins = value["plugins"]
        .as_array()
        .unwrap_or_else(|| panic!("no plugins array in {path}"));
    assert!(!plugins.is_empty(), "{path} has no plugins entry");
    plugins[0]["version"]
        .as_str()
        .unwrap_or_else(|| panic!("no version in {path} plugins[0]"))
        .to_string()
}

fn collect_yml_in_dir(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                paths.extend(collect_yml_in_dir(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("yml") {
                paths.push(path);
            }
        }
    }
    paths
}

fn load_json_schema(path: &str) -> JSONSchema {
    let schema_text = fs::read_to_string(repo_root().join(path))
        .unwrap_or_else(|_| panic!("could not read {path}"));
    let schema_value: Value =
        serde_json::from_str(&schema_text).unwrap_or_else(|_| panic!("invalid JSON: {path}"));
    JSONSchema::options()
        .with_draft(Draft::Draft202012)
        .compile(&schema_value)
        .unwrap_or_else(|err| panic!("failed to compile schema {path}: {err}"))
}

fn assert_schema_accepts(path: &str, instance: &Value) {
    let schema = load_json_schema(path);
    if let Err(errors) = schema.validate(instance) {
        let messages: Vec<String> = errors.map(|err| err.to_string()).collect();
        panic!(
            "schema {path} rejected valid fixture: {}",
            messages.join("; ")
        );
    }
}

fn assert_schema_rejects(path: &str, instance: &Value) {
    let schema = load_json_schema(path);
    let errors: Vec<String> = schema
        .validate(instance)
        .err()
        .unwrap_or_else(|| panic!("schema {path} unexpectedly accepted invalid fixture"))
        .map(|err| err.to_string())
        .collect();
    assert!(
        !errors.is_empty(),
        "schema {path} should report validation errors"
    );
}

#[derive(Default, Deserialize)]
struct OwnershipPolicyFile {
    #[serde(default)]
    layers: HashMap<String, Vec<String>>,
}

#[derive(Default, Deserialize)]
struct DefaultsPolicyFile {
    #[serde(default)]
    defaults: HashMap<String, ApprovedDefault>,
}

#[derive(Default, Deserialize)]
struct ApprovedDefault {
    #[serde(default)]
    allowed_layers: Vec<String>,
}

#[derive(Default, Deserialize)]
struct AdapterPolicyFile {
    #[serde(default)]
    ports: HashMap<String, PortPolicy>,
}

#[derive(Default, Deserialize)]
struct PortPolicy {
    #[serde(default)]
    contract_tests: Vec<String>,
}

#[derive(Default, Deserialize)]
struct ContractPolicyFile {
    #[serde(default)]
    contracts: HashMap<String, ContractPolicy>,
}

#[derive(Default, Deserialize)]
struct ContractPolicy {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    context: String,
    #[serde(default)]
    owner_layer: String,
    #[serde(default)]
    schema: String,
    #[serde(default)]
    witnesses: Vec<String>,
}

#[derive(Default, Deserialize)]
struct ContextPolicyFile {
    #[serde(default)]
    contexts: HashMap<String, ContextPolicy>,
}

#[derive(Default, Deserialize)]
struct ContextPolicy {
    #[serde(default)]
    vocabulary: ContextVocabulary,
}

#[derive(Default, Deserialize)]
struct ContextVocabulary {
    #[serde(default)]
    nouns: Vec<String>,
    #[serde(default)]
    verbs: Vec<String>,
}

fn read_yaml<T: Default + for<'de> Deserialize<'de>>(path: &str) -> T {
    let full_path = repo_root().join(path);
    let text = fs::read_to_string(&full_path)
        .unwrap_or_else(|_| panic!("could not read {}", full_path.display()));
    serde_yml::from_str(&text).unwrap_or_else(|err| panic!("invalid YAML {}: {err}", path))
}

fn read_text(path: &str) -> String {
    let full_path = repo_root().join(path);
    fs::read_to_string(&full_path)
        .unwrap_or_else(|_| panic!("could not read {}", full_path.display()))
}

#[test]
fn versions_are_consistent() {
    let cargo = cargo_version();
    let plugin = json_version(".claude-plugin/plugin.json");
    let market = marketplace_version();

    let versions: HashSet<&str> = [cargo.as_str(), plugin.as_str(), market.as_str()]
        .into_iter()
        .collect();
    assert_eq!(
        versions.len(),
        1,
        "version mismatch: Cargo.toml={cargo} plugin.json={plugin} marketplace.json={market}"
    );
}

#[test]
fn yaml_files_are_valid() {
    let root = repo_root();
    let mut yaml_paths: Vec<PathBuf> = Vec::new();

    for dir in &["policy", "rules"] {
        yaml_paths.extend(collect_yml_in_dir(&root.join(dir)));
    }
    yaml_paths.push(root.join("sgconfig.yml"));

    for path in &yaml_paths {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("could not read {}", path.display()));
        let _: serde_yml::Value = serde_yml::from_str(&text)
            .unwrap_or_else(|err| panic!("invalid YAML {}: {err}", path.display()));
    }
}

#[test]
fn report_schema_accepts_valid_fixture() {
    let report = json!({
        "version": 3,
        "report_id": "wg-20260401-0001-tool_use",
        "created_at": "2026-04-01T00:00:00Z",
        "charter_ref": "CHG-1",
        "status": "pending",
        "file": "src/api/tool_use.py",
        "canonical_file": "/tmp/project/src/api/tool_use.py",
        "summary": {
            "files_scanned": 1,
            "violations": 0,
            "holes": 0,
            "drift": 1,
            "obligations": 0,
            "by_kind": {
                "drift": 1
            },
            "by_file": {
                "src/api/tool_use.py": 1
            }
        },
        "findings": [
            {
                "kind": "drift",
                "file": "src/api/tool_use.py",
                "canonical_file": "/tmp/project/src/api/tool_use.py",
                "line": 1,
                "message": "Public symbol `ToolUsePayload` is missing an explicit export witness",
                "required_judgements": ["surface"],
                "remedy_candidates": ["add export witness"],
                "proof_options": ["__all__/named export/pub"]
            }
        ]
    });

    assert_schema_accepts("docs/report-schema-v3.json", &report);
}

#[test]
fn report_schema_rejects_invalid_fixture() {
    let report = json!({
        "version": 3,
        "report_id": "",
        "status": "pending",
        "file": "src/api/tool_use.py",
        "findings": [
            {
                "kind": "unknown_kind",
                "message": ""
            }
        ]
    });

    assert_schema_rejects("docs/report-schema-v3.json", &report);
}

#[test]
fn charter_schema_accepts_valid_fixture() {
    let charter = json!({
        "version": 1,
        "change_id": "CHG-42",
        "constitution_mode": "extend",
        "source": {
            "kind": "approved-plan",
            "ref": "conversation"
        },
        "surfaces": {
            "public_symbols": {
                "src/api/tool_use.py": {
                    "ToolUsePayload": "public_concept"
                }
            }
        },
        "contexts": {
            "assignments": {
                "src/api/tool_use.py": "api_boundary"
            }
        },
        "contracts": {
            "add": [
                {
                    "id": "http.tool_use_payload.v1",
                    "kind": "shape",
                    "compatibility": "exact"
                }
            ]
        },
        "defaults": {
            "approvals": ["REQ-123"]
        },
        "adapters": {
            "add": ["SqlUserRepository"]
        },
        "holes": [
            {
                "kind": "context",
                "question": "Which bounded context owns ToolUsePayload?"
            }
        ]
    });

    assert_schema_accepts("docs/charter-schema-v1.json", &charter);
}

#[test]
fn charter_schema_rejects_invalid_fixture() {
    let charter = json!({
        "version": 2,
        "change_id": "",
        "constitution_mode": "rewrite",
        "holes": [
            {
                "kind": "unknown",
                "question": ""
            }
        ]
    });

    assert_schema_rejects("docs/charter-schema-v1.json", &charter);
}

#[test]
fn bundled_policy_files_are_semantically_consistent() {
    let ownership: OwnershipPolicyFile = read_yaml("policy/ownership.yml");
    let defaults: DefaultsPolicyFile = read_yaml("policy/defaults.yml");
    let adapters: AdapterPolicyFile = read_yaml("policy/adapters.yml");
    let contracts: ContractPolicyFile = read_yaml("policy/contracts.yml");
    let contexts: ContextPolicyFile = read_yaml("policy/contexts.yml");
    let owner_layers: HashSet<&str> = ownership.layers.keys().map(String::as_str).collect();
    let context_names: HashSet<&str> = contexts.contexts.keys().map(String::as_str).collect();
    let root = repo_root();

    assert!(
        !owner_layers.is_empty(),
        "ownership.yml must declare owner layers"
    );
    assert!(
        !context_names.is_empty(),
        "contexts.yml must declare bounded contexts"
    );

    for (approval_id, approval) in defaults.defaults {
        assert!(
            !approval.allowed_layers.is_empty(),
            "default approval {approval_id} must declare allowed_layers"
        );
        for layer in approval.allowed_layers {
            assert!(
                owner_layers.contains(layer.as_str()),
                "default approval {approval_id} references unknown owner layer {layer}"
            );
        }
    }

    for (port_name, port) in adapters.ports {
        assert!(
            !port.contract_tests.is_empty(),
            "adapter port {port_name} must declare at least one contract test"
        );
        for contract_test in port.contract_tests {
            assert!(
                root.join(&contract_test).is_file(),
                "adapter port {port_name} references missing contract test {contract_test}"
            );
        }
    }

    for (contract_id, contract) in contracts.contracts {
        assert!(
            ["shape", "interaction", "law"].contains(&contract.kind.as_str()),
            "contract {contract_id} must use a known kind"
        );
        assert!(
            owner_layers.contains(contract.owner_layer.as_str()),
            "contract {contract_id} references unknown owner layer {}",
            contract.owner_layer
        );
        assert!(
            context_names.contains(contract.context.as_str()),
            "contract {contract_id} references unknown context {}",
            contract.context
        );
        assert!(
            !contract.witnesses.is_empty(),
            "contract {contract_id} must declare at least one witness"
        );
        for witness in contract.witnesses {
            assert!(
                root.join(&witness).is_file(),
                "contract {contract_id} references missing witness {witness}"
            );
        }
        if contract.kind != "law" {
            assert!(
                !contract.schema.is_empty(),
                "contract {contract_id} of kind {} must declare schema",
                contract.kind
            );
        }
        if !contract.schema.is_empty() {
            assert!(
                root.join(&contract.schema).is_file(),
                "contract {contract_id} references missing schema {}",
                contract.schema
            );
        }
    }

    for (context_name, context) in contexts.contexts {
        assert!(
            !context.vocabulary.nouns.is_empty(),
            "context {context_name} must declare nouns"
        );
        assert!(
            !context.vocabulary.verbs.is_empty(),
            "context {context_name} must declare verbs"
        );
    }
}

#[test]
fn skill_docs_reference_stable_charter_paths_and_retirement() {
    let charter_skill = read_text("skills/charter/SKILL.md");
    assert!(
        charter_skill.contains("${CLAUDE_PLUGIN_DATA}/charters/active/<change-id>.yml"),
        "charter skill should document the stable active charter path"
    );
    assert!(
        !charter_skill.contains("charters/active/.yml"),
        "charter skill must not document anonymous charter filenames"
    );
    assert!(
        charter_skill.contains("update it in place"),
        "charter skill should document stable change_id updates"
    );
    assert!(
        charter_skill.contains("temporary"),
        "charter skill should describe temporary charter lifecycle"
    );

    let repair_skill = read_text("skills/repair/SKILL.md");
    assert!(
        repair_skill.contains("reports containing `hole` are the majority"),
        "repair skill should document hole-majority triage"
    );
    assert!(
        repair_skill.contains("retire-charters"),
        "repair skill should document engine-backed charter retirement"
    );

    let retire_skill = read_text("skills/retire/SKILL.md");
    assert!(
        retire_skill.contains("retire-charters"),
        "retire skill should use the engine retirement command"
    );
    assert!(
        retire_skill.contains("charters/history"),
        "retire skill should archive into charters/history"
    );
}
