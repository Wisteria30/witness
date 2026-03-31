use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use regex::Regex;
use serde_json::Value;

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

fn collect_yml_in_dir(dir: &std::path::Path) -> Vec<PathBuf> {
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
