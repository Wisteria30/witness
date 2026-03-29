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
    let text = fs::read_to_string(repo_root().join("marketplace.json"))
        .expect("could not read marketplace.json");
    let value: Value = serde_json::from_str(&text).expect("invalid JSON: marketplace.json");
    let plugins = value["plugins"]
        .as_array()
        .expect("no plugins array in marketplace.json");
    assert!(!plugins.is_empty(), "marketplace.json has no plugins entry");
    plugins[0]["version"]
        .as_str()
        .expect("no version in marketplace.json plugins[0]")
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

#[test]
fn yaml_files_are_valid() {
    let root = repo_root();
    let mut yaml_paths: Vec<PathBuf> = Vec::new();

    for dir in &["policy", "rules"] {
        let dir_path = root.join(dir);
        if dir_path.is_dir() {
            for entry in fs::read_dir(&dir_path).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().and_then(|e| e.to_str()) == Some("yml") {
                    yaml_paths.push(path);
                }
            }
        }
    }
    yaml_paths.push(root.join("sgconfig.yml"));

    for path in &yaml_paths {
        let text = fs::read_to_string(path)
            .unwrap_or_else(|_| panic!("could not read {}", path.display()));
        let _: serde_yml::Value = serde_yml::from_str(&text)
            .unwrap_or_else(|err| panic!("invalid YAML {}: {err}", path.display()));
    }
}
