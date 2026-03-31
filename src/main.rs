use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use glob::{MatchOptions, Pattern};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const APPROVAL_MODE_NONE: &str = "none";
const APPROVAL_MODE_REGISTRY: &str = "registry_policy_comment";
const DEFAULT_TEST_GLOBS: &[&str] = &[
    "**/test/**",
    "**/tests/**",
    "**/*_test.py",
    "**/test_*.py",
    "**/*.test.ts",
    "**/*.spec.ts",
    "**/__tests__/**",
    "**/conftest.py",
];
const DEFAULT_SKIP_GLOBS: &[&str] = &[
    "**/generated/**",
    "**/openapi/**",
    "**/swagger/**",
    "**/codegen/**",
    "**/__generated__/**",
];
const REPORT_VERSION: u8 = 3;
const OWNER_PRECEDENCE: &[&str] = &[
    "tests",
    "composition_root",
    "boundary",
    "application",
    "domain",
    "infrastructure",
];
const BATCH_SIZE: usize = 128;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let exit_code = match run(args) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("{message}");
            2
        }
    };
    std::process::exit(exit_code);
}

fn run(args: Vec<String>) -> Result<i32, String> {
    let cli = Cli::parse(args)?;
    let catalog = RuleCatalog::load(&cli.common.config_dir)?;
    let policies = PolicySet::load(&cli.common.config_dir)?;
    let charters = CharterSet::load(cli.common.charter_dir.as_deref())?;

    match cli.mode {
        Mode::ScanFile { file } => {
            let bundle = scan_file(&cli.common, &catalog, &policies, &file, None)?;
            let result = finalize_scan("scan-file", &cli.common, &policies, &charters, bundle)?;
            emit_scan_result(&cli.common, &result)
        }
        Mode::ScanTree { root } => {
            let bundle = scan_tree(&cli.common, &catalog, &policies, &root)?;
            let result = finalize_scan("scan-tree", &cli.common, &policies, &charters, bundle)?;
            emit_scan_result(&cli.common, &result)
        }
        Mode::ScanHook => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|err| format!("failed to read stdin: {err}"))?;
            let hook_ctx = extract_hook_context(&input)?;
            if hook_ctx.file_path.as_os_str().is_empty() {
                let result = ScanResult::empty("scan-hook", hook_ctx.scan_root);
                if !cli.common.hook_response {
                    print_json(&result)?;
                }
                return Ok(0);
            }
            let bundle = scan_file(
                &cli.common,
                &catalog,
                &policies,
                &hook_ctx.file_path,
                Some(&hook_ctx.scan_root),
            )?;
            let result = finalize_scan("scan-hook", &cli.common, &policies, &charters, bundle)?;
            emit_scan_result(&cli.common, &result)
        }
        Mode::ScanStop => {
            let result = scan_stop(&cli.common)?;
            if cli.common.hook_response {
                if !result.clean {
                    print_json(&build_stop_hook_response(&result))?;
                }
            } else {
                print_json(&result)?;
            }
            Ok(if result.clean { 0 } else { 1 })
        }
    }
}

fn emit_scan_result(common: &CommonOptions, result: &ScanResult) -> Result<i32, String> {
    if common.hook_response {
        if !result.clean {
            print_json(&build_post_tooluse_hook_response(result))?;
        }
    } else {
        print_json(result)?;
    }
    Ok(if result.clean { 0 } else { 1 })
}

#[derive(Clone)]
struct CommonOptions {
    config_dir: PathBuf,
    ast_grep_bin: String,
    report_dir: Option<PathBuf>,
    charter_dir: Option<PathBuf>,
    hook_response: bool,
    test_globs: Vec<String>,
}

#[allow(clippy::enum_variant_names)]
enum Mode {
    ScanFile { file: PathBuf },
    ScanTree { root: PathBuf },
    ScanHook,
    ScanStop,
}

struct Cli {
    common: CommonOptions,
    mode: Mode,
}

impl Cli {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let script_dir = env::current_exe()
            .map_err(|err| format!("failed to resolve current executable: {err}"))?
            .parent()
            .ok_or("failed to resolve executable directory")?
            .to_path_buf();

        let mut common = CommonOptions {
            config_dir: find_default_config_dir(&script_dir),
            ast_grep_bin: "ast-grep".to_string(),
            report_dir: None,
            charter_dir: None,
            hook_response: false,
            test_globs: DEFAULT_TEST_GLOBS
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        };

        let mut iter: VecDeque<String> = args.into();
        let mut positional_root: Option<PathBuf> = None;
        let mut changed_only: Option<PathBuf> = None;
        let mut mode: Option<Mode> = None;

        while let Some(arg) = iter.pop_front() {
            match arg.as_str() {
                "scan-file" => {
                    let file = parse_subcommand_path(&mut iter, "--file")?;
                    mode = Some(Mode::ScanFile { file });
                }
                "scan-tree" => {
                    let root = parse_optional_subcommand_path(&mut iter, "--root")?
                        .unwrap_or_else(|| PathBuf::from("."));
                    mode = Some(Mode::ScanTree { root });
                }
                "scan-hook" => mode = Some(Mode::ScanHook),
                "scan-stop" => mode = Some(Mode::ScanStop),
                "--ast-grep-bin" => common.ast_grep_bin = next_value(&mut iter, "--ast-grep-bin")?,
                "--config-dir" => {
                    common.config_dir = PathBuf::from(next_value(&mut iter, "--config-dir")?)
                }
                "--report-dir" => {
                    common.report_dir = Some(PathBuf::from(next_value(&mut iter, "--report-dir")?))
                }
                "--charter-dir" => {
                    common.charter_dir =
                        Some(PathBuf::from(next_value(&mut iter, "--charter-dir")?))
                }
                "--test-globs" => {
                    common.test_globs = next_value(&mut iter, "--test-globs")?
                        .split(',')
                        .filter(|part| !part.trim().is_empty())
                        .map(|part| part.trim().to_string())
                        .collect();
                }
                "--hook-response" => common.hook_response = true,
                "--changed-only" => {
                    changed_only = Some(PathBuf::from(next_value(&mut iter, "--changed-only")?));
                }
                value if value.starts_with('-') => return Err(format!("unknown option: {value}")),
                value => {
                    if positional_root.is_some() {
                        return Err("only one root path can be provided".to_string());
                    }
                    positional_root = Some(PathBuf::from(value));
                }
            }
        }

        common.config_dir = common
            .config_dir
            .canonicalize()
            .map_err(|err| format!("failed to resolve config dir: {err}"))?;
        if let Some(charter_dir) = &common.charter_dir
            && charter_dir.exists()
        {
            common.charter_dir = Some(
                charter_dir
                    .canonicalize()
                    .map_err(|err| format!("failed to resolve charter dir: {err}"))?,
            );
        }

        let mode = match mode {
            Some(mode) => mode,
            None => match changed_only {
                Some(file) => Mode::ScanFile { file },
                None => Mode::ScanTree {
                    root: positional_root.unwrap_or_else(|| PathBuf::from(".")),
                },
            },
        };

        Ok(Self { common, mode })
    }
}

fn find_default_config_dir(script_dir: &Path) -> PathBuf {
    for candidate in script_dir.ancestors() {
        if candidate.join("sgconfig.yml").is_file() {
            return candidate.to_path_buf();
        }
    }
    if let Ok(cwd) = env::current_dir() {
        for candidate in cwd.ancestors() {
            if candidate.join("sgconfig.yml").is_file() {
                return candidate.to_path_buf();
            }
        }
    }
    script_dir.to_path_buf()
}

fn parse_subcommand_path(
    iter: &mut VecDeque<String>,
    expected_flag: &str,
) -> Result<PathBuf, String> {
    match iter.pop_front() {
        Some(flag) if flag == expected_flag => Ok(PathBuf::from(next_value(iter, expected_flag)?)),
        Some(flag) => Err(format!("expected {expected_flag}, got {flag}")),
        None => Err(format!("missing {expected_flag}")),
    }
}

fn parse_optional_subcommand_path(
    iter: &mut VecDeque<String>,
    expected_flag: &str,
) -> Result<Option<PathBuf>, String> {
    match iter.front() {
        Some(flag) if flag == expected_flag => {
            iter.pop_front();
            Ok(Some(PathBuf::from(next_value(iter, expected_flag)?)))
        }
        _ => Ok(None),
    }
}

fn next_value(iter: &mut VecDeque<String>, option: &str) -> Result<String, String> {
    iter.pop_front()
        .ok_or_else(|| format!("missing value for {option}"))
}

#[derive(Default, Deserialize, Clone)]
struct OwnershipPolicyFile {
    #[serde(default)]
    layers: HashMap<String, Vec<String>>,
}

#[derive(Default, Deserialize, Clone)]
struct DefaultsPolicyFile {
    #[serde(default)]
    defaults: HashMap<String, ApprovedDefault>,
}

#[derive(Clone, Default, Deserialize)]
#[allow(dead_code)]
struct ApprovedDefault {
    #[serde(default)]
    symbol: String,
    #[serde(default)]
    allowed_layers: Vec<String>,
    #[serde(default)]
    reason: String,
}

#[derive(Default, Deserialize, Clone)]
struct AdapterPolicyFile {
    #[serde(default)]
    ports: HashMap<String, PortPolicy>,
}

#[derive(Clone, Default, Deserialize)]
#[allow(dead_code)]
struct PortPolicy {
    #[serde(default)]
    allowed_runtime_adapters: Vec<String>,
    #[serde(default)]
    contract_tests: Vec<String>,
}

#[derive(Default, Deserialize, Clone)]
struct SurfacePolicyFile {
    #[serde(default)]
    public_by_default: SurfacePatterns,
    #[serde(default)]
    extension_api_patterns: Vec<String>,
    #[serde(default)]
    rules: SurfaceRules,
}

#[derive(Default, Deserialize, Clone)]
struct SurfacePatterns {
    #[serde(default)]
    concept_patterns: Vec<String>,
}

#[derive(Default, Deserialize, Clone)]
struct SurfaceRules {
    #[serde(default)]
    forbid_restricted_visibility_for_public_concepts: bool,
    #[serde(default)]
    require_explicit_export_manifest_for_new_public_symbols: bool,
}

#[derive(Default, Deserialize, Clone)]
struct ContractPolicyFile {
    #[serde(default)]
    contracts: HashMap<String, ContractPolicy>,
}

#[derive(Default, Deserialize, Clone)]
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
    compatibility: String,
    #[serde(default)]
    witnesses: Vec<String>,
}

#[derive(Default, Deserialize, Clone)]
struct ContextPolicyFile {
    #[serde(default)]
    contexts: HashMap<String, ContextPolicy>,
}

#[derive(Default, Deserialize, Clone)]
#[allow(dead_code)]
struct ContextPolicy {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    vocabulary: ContextVocabulary,
    #[serde(default)]
    may_depend_on: Vec<String>,
    #[serde(default)]
    public_entrypoints: Vec<String>,
}

#[derive(Default, Deserialize, Clone)]
struct ContextVocabulary {
    #[serde(default)]
    nouns: Vec<String>,
    #[serde(default)]
    verbs: Vec<String>,
}

#[derive(Default, Clone)]
struct PolicySet {
    ownership: OwnershipPolicyFile,
    defaults: DefaultsPolicyFile,
    adapters: AdapterPolicyFile,
    surfaces: SurfacePolicyFile,
    contracts: ContractPolicyFile,
    contexts: ContextPolicyFile,
}

impl PolicySet {
    fn load(config_dir: &Path) -> Result<Self, String> {
        Ok(Self {
            ownership: read_yaml_file(&config_dir.join("policy/ownership.yml"))?,
            defaults: read_yaml_file(&config_dir.join("policy/defaults.yml"))?,
            adapters: read_yaml_file(&config_dir.join("policy/adapters.yml"))?,
            surfaces: read_yaml_file(&config_dir.join("policy/surfaces.yml"))?,
            contracts: read_yaml_file(&config_dir.join("policy/contracts.yml"))?,
            contexts: read_yaml_file(&config_dir.join("policy/contexts.yml"))?,
        })
    }

    fn registered_approval(&self, id: &str) -> Option<&ApprovedDefault> {
        self.defaults.defaults.get(id)
    }

    fn registered_adapters(&self) -> BTreeSet<String> {
        self.adapters
            .ports
            .values()
            .flat_map(|port| port.allowed_runtime_adapters.iter().cloned())
            .collect()
    }

    fn all_contract_ids(&self) -> BTreeSet<String> {
        self.contracts.contracts.keys().cloned().collect()
    }
}

fn read_yaml_file<T>(path: &Path) -> Result<T, String>
where
    T: Default + for<'de> Deserialize<'de>,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_yml::from_str(&text).map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

#[derive(Default, Deserialize, Clone)]
#[allow(dead_code)]
struct CharterFile {
    version: u8,
    #[serde(default)]
    change_id: String,
    #[serde(default)]
    constitution_mode: String,
    #[serde(default)]
    source: CharterSource,
    #[serde(default)]
    surfaces: CharterSurfaces,
    #[serde(default)]
    contexts: CharterContexts,
    #[serde(default)]
    contracts: CharterContracts,
    #[serde(default)]
    defaults: CharterDefaults,
    #[serde(default)]
    adapters: CharterAdapters,
    #[serde(default)]
    holes: Vec<CharterHole>,
}

#[derive(Default, Deserialize, Clone)]
#[allow(dead_code)]
struct CharterSource {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    ref_name: String,
}

#[derive(Default, Deserialize, Clone)]
struct CharterSurfaces {
    #[serde(default)]
    public_symbols: BTreeMap<String, BTreeMap<String, String>>,
}

#[derive(Default, Deserialize, Clone)]
struct CharterContexts {
    #[serde(default)]
    assignments: BTreeMap<String, String>,
}

#[derive(Default, Deserialize, Clone)]
struct CharterContracts {
    #[serde(default)]
    add: Vec<CharterContract>,
}

#[derive(Default, Deserialize, Clone)]
struct CharterContract {
    #[serde(default)]
    id: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    compatibility: String,
}

#[derive(Default, Deserialize, Clone)]
struct CharterDefaults {
    #[serde(default)]
    approvals: Vec<String>,
}

#[derive(Default, Deserialize, Clone)]
struct CharterAdapters {
    #[serde(default)]
    add: Vec<String>,
}

#[derive(Default, Deserialize, Clone)]
struct CharterHole {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    question: String,
}

#[derive(Clone)]
struct LoadedCharter {
    path: PathBuf,
    relative_path: String,
    charter: CharterFile,
}

#[derive(Default, Clone)]
struct CharterSet {
    items: Vec<LoadedCharter>,
}

impl CharterSet {
    fn load(charter_dir: Option<&Path>) -> Result<Self, String> {
        let Some(charter_dir) = charter_dir else {
            return Ok(Self::default());
        };
        if !charter_dir.is_dir() {
            return Ok(Self::default());
        }
        let mut items = Vec::new();
        for entry in fs::read_dir(charter_dir)
            .map_err(|err| format!("failed to read {}: {err}", charter_dir.display()))?
        {
            let path = entry
                .map_err(|err| format!("failed to read charter entry: {err}"))?
                .path();
            let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };
            if !matches!(ext, "yml" | "yaml" | "json") {
                continue;
            }
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
            let charter: CharterFile = serde_yml::from_str(&text)
                .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
            items.push(LoadedCharter {
                relative_path: path
                    .strip_prefix(charter_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
                path,
                charter,
            });
        }
        items.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(Self { items })
    }

    fn paths(&self) -> Vec<PathBuf> {
        self.items.iter().map(|item| item.path.clone()).collect()
    }

    fn charter_ref(&self) -> Option<String> {
        let ids: Vec<_> = self
            .items
            .iter()
            .filter_map(|item| {
                if item.charter.change_id.trim().is_empty() {
                    None
                } else {
                    Some(item.charter.change_id.clone())
                }
            })
            .collect();
        if ids.is_empty() {
            None
        } else {
            Some(ids.join(","))
        }
    }

    fn public_symbol_decision(&self, file_key: &str, symbol: &str) -> Option<String> {
        self.items.iter().find_map(|item| {
            item.charter
                .surfaces
                .public_symbols
                .get(file_key)
                .and_then(|symbols| symbols.get(symbol))
                .cloned()
        })
    }

    fn context_assignment(&self, file_key: &str) -> Option<String> {
        self.items
            .iter()
            .find_map(|item| item.charter.contexts.assignments.get(file_key).cloned())
    }
}

#[derive(Clone)]
struct RuleInfo {
    path: PathBuf,
    metadata: HashMap<String, String>,
}

struct RuleCatalog {
    by_id: HashMap<String, RuleInfo>,
}

impl RuleCatalog {
    fn load(config_dir: &Path) -> Result<Self, String> {
        let sgconfig = config_dir.join("sgconfig.yml");
        let mut rule_dirs = vec!["rules".to_string()];

        if sgconfig.exists() {
            let doc = fs::read_to_string(&sgconfig)
                .map_err(|err| format!("failed to read {}: {err}", sgconfig.display()))?;
            let parsed = parse_rule_dirs(&doc);
            if !parsed.is_empty() {
                rule_dirs = parsed;
            }
        }

        let mut by_id = HashMap::new();
        for rule_dir in rule_dirs {
            let dir = config_dir.join(&rule_dir);
            if !dir.is_dir() {
                continue;
            }
            let entries = fs::read_dir(&dir)
                .map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
            for entry in entries {
                let path = entry
                    .map_err(|err| {
                        format!("failed to read rule entry in {}: {err}", dir.display())
                    })?
                    .path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("yml") {
                    continue;
                }
                let text = fs::read_to_string(&path)
                    .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
                let Some((rule_id, metadata)) = parse_rule_file(&text) else {
                    continue;
                };
                by_id.insert(
                    rule_id,
                    RuleInfo {
                        path: path.canonicalize().unwrap_or(path),
                        metadata,
                    },
                );
            }
        }

        Ok(Self { by_id })
    }

    fn rule_paths<'a>(&'a self, ids: impl IntoIterator<Item = &'a str>) -> Vec<PathBuf> {
        ids.into_iter()
            .filter_map(|id| self.by_id.get(id).map(|rule| rule.path.clone()))
            .collect()
    }

    fn metadata_for(&self, rule_id: &str) -> HashMap<String, String> {
        self.by_id
            .get(rule_id)
            .map(|rule| rule.metadata.clone())
            .unwrap_or_default()
    }
}

fn parse_rule_dirs(text: &str) -> Vec<String> {
    let mut rule_dirs = Vec::new();
    let mut in_rule_dirs = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "ruleDirs:" {
            in_rule_dirs = true;
            continue;
        }
        if in_rule_dirs {
            if let Some(item) = trimmed.strip_prefix("- ") {
                rule_dirs.push(strip_yaml_scalar(item));
                continue;
            }
            break;
        }
    }
    rule_dirs
}

fn parse_rule_file(text: &str) -> Option<(String, HashMap<String, String>)> {
    let mut rule_id: Option<String> = None;
    let mut metadata = HashMap::new();
    let mut in_metadata = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if !line.starts_with(' ') && !line.starts_with('\t') {
            in_metadata = trimmed == "metadata:";
            if let Some(value) = trimmed.strip_prefix("id:") {
                rule_id = Some(strip_yaml_scalar(value));
            }
            continue;
        }

        if in_metadata && let Some((key, value)) = trimmed.split_once(':') {
            metadata.insert(key.trim().to_string(), strip_yaml_scalar(value));
        }
    }

    rule_id.map(|rule_id| (rule_id, metadata))
}

fn strip_yaml_scalar(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

#[derive(Clone)]
struct RawFinding {
    display_file: String,
    canonical_file: PathBuf,
    line0: usize,
    rule_id: String,
    message: String,
    text: String,
    metadata: HashMap<String, String>,
}

impl RawFinding {
    fn snippet(&self) -> String {
        self.text
            .trim()
            .replace('\n', " ")
            .chars()
            .take(240)
            .collect()
    }
}

#[derive(Deserialize)]
struct AstGrepFinding {
    #[serde(rename = "file")]
    file_path: String,
    #[serde(rename = "range")]
    range: AstRange,
    #[serde(rename = "ruleId")]
    rule_id: Option<String>,
    message: Option<String>,
    text: Option<String>,
    metadata: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct AstRange {
    start: AstPosition,
}

#[derive(Deserialize)]
struct AstPosition {
    line: usize,
}

#[derive(Clone)]
struct ScanBundle {
    scan_root: PathBuf,
    scanned_files: Vec<PathBuf>,
    contents: HashMap<PathBuf, String>,
    findings: Vec<RawFinding>,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FindingKind {
    Violation,
    Hole,
    Drift,
    Obligation,
}

impl FindingKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Violation => "violation",
            Self::Hole => "hole",
            Self::Drift => "drift",
            Self::Obligation => "obligation",
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct ScanSummary {
    files_scanned: usize,
    violations: usize,
    holes: usize,
    drift: usize,
    obligations: usize,
    by_kind: BTreeMap<String, usize>,
    by_file: BTreeMap<String, usize>,
}

#[derive(Clone, Serialize, Deserialize)]
struct FindingRecord {
    kind: FindingKind,
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rule_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    violation_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_layer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    surface_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility: Option<String>,
    snippet: String,
    message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    required_judgements: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    remedy_candidates: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    proof_options: Vec<String>,
}

#[derive(Clone, Serialize)]
struct ScanResult {
    version: u8,
    mode: String,
    clean: bool,
    root: String,
    scanned_files: Vec<String>,
    summary: ScanSummary,
    capsule: Option<String>,
    report_paths: Vec<String>,
    findings: Vec<FindingRecord>,
}

impl ScanResult {
    fn empty(mode: &str, root: PathBuf) -> Self {
        Self {
            version: REPORT_VERSION,
            mode: mode.to_string(),
            clean: true,
            root: root.to_string_lossy().to_string(),
            scanned_files: Vec::new(),
            summary: ScanSummary::default(),
            capsule: None,
            report_paths: Vec::new(),
            findings: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReportStatus {
    Pending,
    Resolved,
    NeedsCharterDecision,
}

#[derive(Clone, Serialize, Deserialize)]
struct PendingReport {
    version: u8,
    report_id: String,
    created_at: String,
    status: ReportStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    charter_ref: Option<String>,
    file: String,
    canonical_file: String,
    summary: ScanSummary,
    findings: Vec<FindingRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PendingReportSummary {
    report_id: String,
    file: String,
    canonical_file: String,
    status: ReportStatus,
    summary: ScanSummary,
    capsule: String,
}

#[derive(Serialize)]
struct StopScanResult {
    version: u8,
    clean: bool,
    report_dir: String,
    pending_count: usize,
    summary: ScanSummary,
    capsule: Option<String>,
    pending_reports: Vec<PendingReportSummary>,
}

fn approval_id_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)policy-approved:\s*([A-Z][A-Z0-9_-]*-[A-Za-z0-9._-]+)").unwrap()
    })
}

fn py_test_support_import_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?m)^\s*(from|import)\s+.*(?:tests?|fixtures|mocks?|stubs?|fakes?)").unwrap()
    })
}

fn ts_test_support_import_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?m)^\s*import\s+.*from\s+['"][^'"]*(?:tests?|fixtures|__mocks__|mocks?|stubs?|fakes?)[^'"]*['"]"#,
        )
        .unwrap()
    })
}

fn python_or_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)\bor\b").unwrap())
}

fn py_symbol_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^(class|def)\s+([A-Za-z_][A-Za-z0-9_]*)").unwrap())
}

fn py_all_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)__all__\s*=\s*\[(.*?)\]").unwrap())
}

fn quote_value_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"['"]([A-Za-z_][A-Za-z0-9_]*)['"]"#).unwrap())
}

fn ts_symbol_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?m)^(export\s+)?(class|function|const|type|interface)\s+([A-Za-z_][A-Za-z0-9_]*)",
        )
        .unwrap()
    })
}

const GLOB_OPTS: MatchOptions = MatchOptions {
    require_literal_separator: true,
    require_literal_leading_dot: false,
    case_sensitive: true,
};

fn scan_file(
    common: &CommonOptions,
    catalog: &RuleCatalog,
    policies: &PolicySet,
    file: &Path,
    scan_root_override: Option<&Path>,
) -> Result<ScanBundle, String> {
    let skip_matcher = build_skip_matcher(&common.test_globs)?;
    let scan_root = match scan_root_override {
        Some(root) => root
            .canonicalize()
            .or_else(|_| Ok(root.to_path_buf()))
            .map_err(|err: io::Error| format!("failed to resolve {}: {err}", root.display()))?,
        None => env::current_dir().map_err(|err| format!("failed to read cwd: {err}"))?,
    };

    let joined_file = if file.is_absolute() {
        file.to_path_buf()
    } else {
        scan_root.join(file)
    };
    let canonical_file = joined_file
        .canonicalize()
        .map_err(|err| format!("failed to resolve {}: {err}", joined_file.display()))?;
    let scanned_files = vec![canonical_file.clone()];

    if matches_skip_globs(&skip_matcher, &canonical_file, &scan_root) {
        return Ok(ScanBundle {
            scan_root,
            scanned_files,
            contents: HashMap::new(),
            findings: Vec::new(),
        });
    }

    let content = fs::read_to_string(&canonical_file)
        .map_err(|err| format!("failed to read {}: {err}", canonical_file.display()))?;
    let selected_ids = detect_rule_ids(&canonical_file, &content);
    let mut findings = supplemental_findings(&canonical_file, &content, policies, &scan_root);

    if !selected_ids.is_empty() {
        let rule_paths = catalog.rule_paths(selected_ids.iter().map(String::as_str));
        let inline_rules = read_inline_rules(&rule_paths)?;
        findings.extend(run_ast_grep(
            common,
            catalog,
            &scan_root,
            std::slice::from_ref(&canonical_file),
            &selected_ids,
            &inline_rules,
        )?);
    }

    dedupe_raw_findings(&mut findings);

    Ok(ScanBundle {
        scan_root,
        scanned_files,
        contents: HashMap::from([(canonical_file, content)]),
        findings,
    })
}

fn scan_tree(
    common: &CommonOptions,
    catalog: &RuleCatalog,
    policies: &PolicySet,
    root: &Path,
) -> Result<ScanBundle, String> {
    let scan_root = root
        .canonicalize()
        .map_err(|err| format!("failed to resolve {}: {err}", root.display()))?;
    let skip_matcher = build_skip_matcher(&common.test_globs)?;
    let mut grouped_files: HashMap<Vec<String>, Vec<PathBuf>> = HashMap::new();
    let mut scanned_files = Vec::new();
    let mut contents = HashMap::new();
    let mut supplemental = Vec::new();

    for path in ripgrep_source_files(&scan_root)? {
        if matches_skip_globs(&skip_matcher, &path, &scan_root) {
            continue;
        }
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        scanned_files.push(path.clone());
        contents.insert(path.clone(), content.clone());
        let selected_ids = detect_rule_ids(&path, &content);
        if !selected_ids.is_empty() {
            grouped_files
                .entry(selected_ids)
                .or_default()
                .push(path.clone());
        }
        supplemental.extend(supplemental_findings(&path, &content, policies, &scan_root));
    }

    let mut findings = supplemental;
    for (rule_ids, files) in grouped_files {
        let rule_paths = catalog.rule_paths(rule_ids.iter().map(String::as_str));
        let inline_rules = read_inline_rules(&rule_paths)?;
        for chunk in files.chunks(BATCH_SIZE) {
            findings.extend(run_ast_grep(
                common,
                catalog,
                &scan_root,
                chunk,
                &rule_ids,
                &inline_rules,
            )?);
        }
    }

    dedupe_raw_findings(&mut findings);

    Ok(ScanBundle {
        scan_root,
        scanned_files,
        contents,
        findings,
    })
}

fn detect_rule_ids(path: &Path, content: &str) -> Vec<String> {
    let mut ids = BTreeSet::new();
    let lower = content.to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();

    match extension {
        "py" => {
            if lower.contains(".get(") {
                ids.insert("py-no-fallback-get-default".to_string());
            }
            if lower.contains('=') && python_or_regex().is_match(&lower) {
                ids.insert("py-no-fallback-bool-or".to_string());
            }
            if lower.contains(" if ") && lower.contains(" else ") && lower.contains(" in ") {
                ids.insert("py-no-fallback-conditional-membership-default".to_string());
            }
            if lower.contains(" if ") && lower.contains(" else ") && lower.contains("none") {
                ids.insert("py-no-fallback-conditional-none-default".to_string());
            }
            if lower.contains("getattr(") {
                ids.insert("py-no-fallback-getattr-default".to_string());
            }
            if lower.contains("next(") {
                ids.insert("py-no-fallback-next-default".to_string());
            }
            if lower.contains("getenv(") || lower.contains("os.environ.get(") {
                ids.insert("py-no-fallback-os-getenv-default".to_string());
            }
            if lower.contains("except") && lower.contains("pass") {
                ids.insert("py-no-swallowing-except-pass".to_string());
            }
            if lower.contains("except") && lower.contains("return") {
                ids.insert("py-no-fallback-except-return-default".to_string());
            }
            if lower.contains("contextlib.suppress") {
                ids.insert("py-no-fallback-contextlib-suppress".to_string());
            }
            if contains_any(&lower, &["mock", "stub", "fake"]) {
                ids.insert("py-no-test-double-identifier".to_string());
            }
            if lower.contains("unittest.mock") {
                ids.insert("py-no-test-double-unittest-mock".to_string());
            }
        }
        "ts" | "cts" | "mts" => {
            if lower.contains("??=") {
                ids.insert("ts-no-fallback-nullish-assign".to_string());
            }
            if lower.contains("||=") {
                ids.insert("ts-no-fallback-or-assign".to_string());
            }
            if lower.contains("??") {
                ids.insert("ts-no-fallback-nullish".to_string());
            }
            if lower.contains("||") {
                ids.insert("ts-no-fallback-or".to_string());
            }
            if lower.contains('?') && lower.contains(':') && lower.contains(" in ") {
                ids.insert("ts-no-fallback-lookup-else-default".to_string());
            }
            if lower.contains('?')
                && lower.contains(':')
                && contains_any(&lower, &["!== undefined", "!== null", "!= null"])
            {
                ids.insert("ts-no-fallback-ternary-default".to_string());
            }
            if lower.contains("catch") {
                ids.insert("ts-no-catch-return-default".to_string());
                ids.insert("ts-no-empty-catch".to_string());
                ids.insert("ts-no-promise-catch-default".to_string());
            }
            if contains_any(&lower, &["mock", "stub", "fake"]) {
                ids.insert("ts-no-test-double-identifier".to_string());
            }
            if contains_any(&lower, &["sinon", "ts-auto-mock", "jest-mock"]) {
                ids.insert("ts-no-test-double-import".to_string());
            }
        }
        _ => {}
    }

    ids.into_iter().collect()
}

fn contains_any(content: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| content.contains(needle))
}

fn ripgrep_source_files(scan_root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("rg")
        .args([
            "--files", "-g", "*.py", "-g", "*.ts", "-g", "*.cts", "-g", "*.mts", ".",
        ])
        .current_dir(scan_root)
        .output()
        .map_err(|err| format!("failed to execute ripgrep: {err}"))?;

    check_exit_ok(&output, "ripgrep source scan failed")?;

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            scan_root
                .join(line)
                .canonicalize()
                .unwrap_or_else(|_| scan_root.join(line))
        })
        .filter(|path| is_supported_source(path))
        .collect())
}

fn run_ast_grep(
    common: &CommonOptions,
    catalog: &RuleCatalog,
    scan_root: &Path,
    targets: &[PathBuf],
    rule_ids: &[String],
    inline_rules: &str,
) -> Result<Vec<RawFinding>, String> {
    let mut command = Command::new(&common.ast_grep_bin);
    command.arg("scan").arg("--json=stream");
    if !inline_rules.is_empty() {
        command.arg("--inline-rules").arg(inline_rules);
    }
    for target in targets {
        command.arg(target);
    }
    command.current_dir(&common.config_dir);
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return internal_ast_grep(catalog, scan_root, targets, rule_ids);
        }
        Err(err) => {
            return Err(format!(
                "could not execute {:?}: {err}",
                common.ast_grep_bin
            ));
        }
    };
    let stdout = child
        .stdout
        .take()
        .ok_or("failed to capture ast-grep stdout".to_string())?;
    let reader = BufReader::new(stdout);
    let mut findings = Vec::new();

    for line in reader.lines() {
        let line = line.map_err(|err| format!("failed to read ast-grep output: {err}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let raw: AstGrepFinding = serde_json::from_str(&line)
            .map_err(|err| format!("failed to parse ast-grep JSON: {err}"))?;
        findings.push(to_raw_finding(raw, catalog, &common.config_dir, scan_root));
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to wait for ast-grep: {err}"))?;
    check_exit_ok(&output, "ast-grep scan failed")?;
    Ok(findings)
}

fn internal_ast_grep(
    catalog: &RuleCatalog,
    scan_root: &Path,
    targets: &[PathBuf],
    rule_ids: &[String],
) -> Result<Vec<RawFinding>, String> {
    let enabled: BTreeSet<&str> = rule_ids.iter().map(String::as_str).collect();
    let mut findings = Vec::new();
    for target in targets {
        let content = fs::read_to_string(target)
            .map_err(|err| format!("failed to read {}: {err}", target.display()))?;
        let display_file = resolve_display_path(target, scan_root, &target.to_string_lossy());
        let lines: Vec<&str> = content.lines().collect();
        let extension = target
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();
        for (index, line) in lines.iter().enumerate() {
            let previous = index
                .checked_sub(1)
                .and_then(|value| lines.get(value))
                .copied()
                .unwrap_or("");
            for rule_id in supported_rule_ids(extension) {
                if !enabled.contains(rule_id) {
                    continue;
                }
                if internal_rule_matches(rule_id, line, previous) {
                    findings.push(RawFinding {
                        display_file: display_file.clone(),
                        canonical_file: target.clone(),
                        line0: index,
                        rule_id: rule_id.to_string(),
                        message: internal_rule_message(rule_id).to_string(),
                        text: (*line).to_string(),
                        metadata: catalog.metadata_for(rule_id),
                    });
                }
            }
        }
    }
    Ok(findings)
}

fn supported_rule_ids(extension: &str) -> &'static [&'static str] {
    match extension {
        "py" => &[
            "py-no-fallback-get-default",
            "py-no-fallback-bool-or",
            "py-no-fallback-conditional-membership-default",
            "py-no-fallback-conditional-none-default",
            "py-no-fallback-getattr-default",
            "py-no-fallback-next-default",
            "py-no-fallback-os-getenv-default",
            "py-no-swallowing-except-pass",
            "py-no-fallback-except-return-default",
            "py-no-fallback-contextlib-suppress",
            "py-no-test-double-identifier",
            "py-no-test-double-unittest-mock",
        ],
        "ts" | "cts" | "mts" => &[
            "ts-no-fallback-nullish-assign",
            "ts-no-fallback-or-assign",
            "ts-no-fallback-nullish",
            "ts-no-fallback-or",
            "ts-no-fallback-lookup-else-default",
            "ts-no-fallback-ternary-default",
            "ts-no-catch-return-default",
            "ts-no-empty-catch",
            "ts-no-promise-catch-default",
            "ts-no-test-double-identifier",
            "ts-no-test-double-import",
        ],
        _ => &[],
    }
}

fn internal_rule_matches(rule_id: &str, line: &str, previous_line: &str) -> bool {
    let trimmed = line.trim();
    match rule_id {
        "py-no-fallback-get-default" => line.contains(".get(") && line.contains(','),
        "py-no-fallback-bool-or" => line.contains(" = ") && line.contains(" or "),
        "py-no-fallback-conditional-membership-default" => {
            line.contains(" if ") && line.contains(" else ") && line.contains(" in ")
        }
        "py-no-fallback-conditional-none-default" => {
            line.contains(" if ") && line.contains(" else ") && line.contains("None")
        }
        "py-no-fallback-getattr-default" => {
            line.contains("getattr(") && line.matches(',').count() >= 2
        }
        "py-no-fallback-next-default" => line.contains("next(") && line.contains(','),
        "py-no-fallback-os-getenv-default" => {
            (line.contains("os.getenv(") || line.contains("os.environ.get(")) && line.contains(',')
        }
        "py-no-swallowing-except-pass" => {
            trimmed == "pass" && previous_line.trim_start().starts_with("except")
        }
        "py-no-fallback-except-return-default" => {
            trimmed.starts_with("return ") && previous_line.trim_start().starts_with("except")
        }
        "py-no-fallback-contextlib-suppress" => line.contains("contextlib.suppress"),
        "py-no-test-double-identifier" => {
            contains_any(&line.to_ascii_lowercase(), &["mock", "stub", "fake"])
        }
        "py-no-test-double-unittest-mock" => line.contains("unittest.mock"),
        "ts-no-fallback-nullish-assign" => line.contains("??="),
        "ts-no-fallback-or-assign" => line.contains("||="),
        "ts-no-fallback-nullish" => line.contains("??") && !line.contains("??="),
        "ts-no-fallback-or" => line.contains("||") && !line.contains("||="),
        "ts-no-fallback-lookup-else-default" => {
            line.contains('?') && line.contains(':') && line.contains(" in ")
        }
        "ts-no-fallback-ternary-default" => {
            line.contains('?')
                && line.contains(':')
                && contains_any(line, &["!== undefined", "!== null", "!= null"])
        }
        "ts-no-catch-return-default" => {
            trimmed.starts_with("catch") && previous_line.contains("return")
        }
        "ts-no-empty-catch" => trimmed.starts_with("catch") && trimmed.ends_with("{}"),
        "ts-no-promise-catch-default" => line.contains(".catch("),
        "ts-no-test-double-identifier" => {
            contains_any(&line.to_ascii_lowercase(), &["mock", "stub", "fake"])
        }
        "ts-no-test-double-import" => contains_any(
            &line.to_ascii_lowercase(),
            &["sinon", "ts-auto-mock", "jest-mock"],
        ),
        _ => false,
    }
}

fn internal_rule_message(rule_id: &str) -> &'static str {
    match rule_id {
        "py-no-fallback-get-default" => "Dictionary get default owns policy at the wrong layer",
        "py-no-fallback-bool-or" => "Truthiness-based fallback owns policy at the wrong layer",
        "py-no-fallback-conditional-membership-default" => {
            "Conditional membership rewrite is still an inline fallback"
        }
        "py-no-fallback-conditional-none-default" => {
            "Conditional None rewrite is still an inline fallback"
        }
        "py-no-fallback-getattr-default" => "getattr default owns policy at the wrong layer",
        "py-no-fallback-next-default" => "next default owns policy at the wrong layer",
        "py-no-fallback-os-getenv-default" => "Environment fallback owns policy at the wrong layer",
        "py-no-swallowing-except-pass" => {
            "Swallowed exception removes failure without a lawful owner or witness"
        }
        "py-no-fallback-except-return-default" => {
            "Returning a default from except removes failure without a lawful owner or witness"
        }
        "py-no-fallback-contextlib-suppress" => {
            "Suppressed exception removes failure without a lawful owner or witness"
        }
        "py-no-test-double-identifier" => {
            "Runtime Python code must not use test-double identifiers"
        }
        "py-no-test-double-unittest-mock" => "Runtime Python code must not import unittest.mock",
        "ts-no-fallback-nullish-assign" => {
            "In-place nullish fallback owns policy at the wrong layer"
        }
        "ts-no-fallback-or-assign" => "In-place or fallback owns policy at the wrong layer",
        "ts-no-fallback-nullish" => "Nullish fallback owns policy at the wrong layer",
        "ts-no-fallback-or" => "Truthy fallback owns policy at the wrong layer",
        "ts-no-fallback-lookup-else-default" => "Lookup fallback owns policy at the wrong layer",
        "ts-no-fallback-ternary-default" => "Ternary fallback owns policy at the wrong layer",
        "ts-no-catch-return-default" => {
            "Returning a default from catch removes failure without a lawful owner or witness"
        }
        "ts-no-empty-catch" => "Empty catch removes failure without a lawful owner or witness",
        "ts-no-promise-catch-default" => {
            "Promise catch default removes failure without a lawful owner or witness"
        }
        "ts-no-test-double-identifier" => {
            "Runtime TypeScript code must not use test-double identifiers"
        }
        "ts-no-test-double-import" => "Runtime TypeScript code must not import test-double tooling",
        _ => "Witness rule violation",
    }
}

fn to_raw_finding(
    raw: AstGrepFinding,
    catalog: &RuleCatalog,
    config_dir: &Path,
    scan_root: &Path,
) -> RawFinding {
    let rule_id = raw.rule_id.unwrap_or_else(|| "<unknown-rule>".to_string());
    let mut metadata = catalog.metadata_for(&rule_id);
    if let Some(extra) = raw.metadata {
        metadata.extend(extra);
    }

    let joined = if Path::new(&raw.file_path).is_absolute() {
        PathBuf::from(&raw.file_path)
    } else {
        config_dir.join(&raw.file_path)
    };
    let canonical_file = joined.canonicalize().unwrap_or(joined);
    let display_file = resolve_display_path(&canonical_file, scan_root, &raw.file_path);

    RawFinding {
        display_file,
        canonical_file,
        line0: raw.range.start.line,
        rule_id,
        message: raw.message.unwrap_or_default(),
        text: raw.text.unwrap_or_default(),
        metadata,
    }
}

fn check_exit_ok(output: &std::process::Output, fallback_msg: &str) -> Result<(), String> {
    match output.status.code() {
        Some(0) | Some(1) => Ok(()),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if stderr.is_empty() {
                fallback_msg.to_string()
            } else {
                stderr
            })
        }
    }
}

fn read_inline_rules(rule_paths: &[PathBuf]) -> Result<String, String> {
    let mut parts = Vec::new();
    for rule_path in rule_paths {
        let raw = fs::read_to_string(rule_path)
            .map_err(|err| format!("failed to read {}: {err}", rule_path.display()))?;
        parts.push(strip_ast_grep_filters(&raw));
    }
    Ok(parts.join("\n---\n"))
}

fn strip_ast_grep_filters(rule_text: &str) -> String {
    let mut result = Vec::new();
    let mut skip_block = false;
    for line in rule_text.lines() {
        if line.starts_with("files:") || line.starts_with("ignores:") {
            skip_block = true;
            continue;
        }
        if skip_block {
            if line.starts_with("  - ") || line.starts_with("  '") || line.starts_with("  \"") {
                continue;
            }
            skip_block = false;
        }
        result.push(line);
    }
    result.join("\n")
}

fn supplemental_findings(
    canonical_file: &Path,
    content: &str,
    policies: &PolicySet,
    scan_root: &Path,
) -> Vec<RawFinding> {
    let owner = guess_owner_layer(canonical_file, scan_root, policies);
    let extension = canonical_file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default();
    let mut findings = Vec::new();
    let display_file =
        resolve_display_path(canonical_file, scan_root, &canonical_file.to_string_lossy());
    let registered_adapters = policies.registered_adapters();

    for (line_index, line) in content.lines().enumerate() {
        match extension {
            "py" => {
                if py_test_support_import_regex().is_match(line) {
                    findings.push(RawFinding {
                        display_file: display_file.clone(),
                        canonical_file: canonical_file.to_path_buf(),
                        line0: line_index,
                        rule_id: "py-no-test-support-import".to_string(),
                        message: "Runtime Python code must not import test support paths"
                            .to_string(),
                        text: line.to_string(),
                        metadata: HashMap::from([
                            ("policy_group".to_string(), "test-double".to_string()),
                            (
                                "violation_class".to_string(),
                                "runtime_double_in_graph".to_string(),
                            ),
                            ("owner_hint".to_string(), "tests".to_string()),
                            ("approval_mode".to_string(), APPROVAL_MODE_NONE.to_string()),
                        ]),
                    });
                }

                if owner != "composition_root" && owner != "tests" {
                    let trimmed = line.trim_start();
                    if !trimmed.starts_with("class ") {
                        for adapter in &registered_adapters {
                            let needle = format!("{adapter}(");
                            if trimmed.contains(&needle) {
                                findings.push(RawFinding {
                                    display_file: display_file.clone(),
                                    canonical_file: canonical_file.to_path_buf(),
                                    line0: line_index,
                                    rule_id: "py-no-adapter-choice-outside-composition-root".to_string(),
                                    message: format!(
                                        "Concrete adapter `{adapter}` is selected outside the composition root"
                                    ),
                                    text: line.to_string(),
                                    metadata: HashMap::from([
                                        ("policy_group".to_string(), "architecture".to_string()),
                                        (
                                            "violation_class".to_string(),
                                            "adapter_choice_outside_composition_root".to_string(),
                                        ),
                                        ("owner_hint".to_string(), "composition_root".to_string()),
                                        ("approval_mode".to_string(), APPROVAL_MODE_NONE.to_string()),
                                    ]),
                                });
                            }
                        }
                    }
                }
            }
            "ts" | "cts" | "mts" => {
                if ts_test_support_import_regex().is_match(line) {
                    findings.push(RawFinding {
                        display_file: display_file.clone(),
                        canonical_file: canonical_file.to_path_buf(),
                        line0: line_index,
                        rule_id: "ts-no-test-support-import".to_string(),
                        message: "Runtime TypeScript code must not import test support paths"
                            .to_string(),
                        text: line.to_string(),
                        metadata: HashMap::from([
                            ("policy_group".to_string(), "test-double".to_string()),
                            (
                                "violation_class".to_string(),
                                "runtime_double_in_graph".to_string(),
                            ),
                            ("owner_hint".to_string(), "tests".to_string()),
                            ("approval_mode".to_string(), APPROVAL_MODE_NONE.to_string()),
                        ]),
                    });
                }

                if owner != "composition_root" && owner != "tests" {
                    let trimmed = line.trim_start();
                    for adapter in &registered_adapters {
                        let needle = format!("new {adapter}(");
                        if trimmed.contains(&needle) {
                            findings.push(RawFinding {
                                display_file: display_file.clone(),
                                canonical_file: canonical_file.to_path_buf(),
                                line0: line_index,
                                rule_id: "ts-no-adapter-choice-outside-composition-root".to_string(),
                                message: format!(
                                    "Concrete adapter `{adapter}` is selected outside the composition root"
                                ),
                                text: line.to_string(),
                                metadata: HashMap::from([
                                    ("policy_group".to_string(), "architecture".to_string()),
                                    (
                                        "violation_class".to_string(),
                                        "adapter_choice_outside_composition_root".to_string(),
                                    ),
                                    ("owner_hint".to_string(), "composition_root".to_string()),
                                    ("approval_mode".to_string(), APPROVAL_MODE_NONE.to_string()),
                                ]),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    findings
}

fn dedupe_raw_findings(findings: &mut Vec<RawFinding>) {
    let mut seen = BTreeSet::new();
    findings.retain(|finding| {
        let key = format!(
            "{}:{}:{}:{}",
            finding.canonical_file.display(),
            finding.line0,
            finding.rule_id,
            finding.message
        );
        seen.insert(key)
    });
}

fn finalize_scan(
    mode: &str,
    common: &CommonOptions,
    policies: &PolicySet,
    charters: &CharterSet,
    bundle: ScanBundle,
) -> Result<ScanResult, String> {
    let mut line_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let mut findings = Vec::new();

    for raw in &bundle.findings {
        match evaluate_approval(raw, policies, &bundle.scan_root, &mut line_cache) {
            ApprovalState::Approved => {}
            ApprovalState::AllowedWithoutApproval => {
                findings.push(raw_to_violation(raw, policies, &bundle.scan_root, None))
            }
            ApprovalState::Invalid(reason) => findings.push(raw_to_violation(
                raw,
                policies,
                &bundle.scan_root,
                Some(reason),
            )),
        }
    }

    findings.extend(analyze_structure(
        policies,
        charters,
        &bundle.scan_root,
        &bundle.contents,
    )?);
    findings.extend(analyze_charter_expectations(
        policies,
        charters,
        &bundle.scan_root,
    )?);
    dedupe_findings(&mut findings);

    let summary = build_summary(&findings, bundle.scanned_files.len());
    let capsule = if findings.is_empty() {
        None
    } else {
        Some(build_capsule(&findings))
    };
    let report_paths = persist_pending_reports(
        common.report_dir.as_deref(),
        &bundle.scan_root,
        &bundle.scanned_files,
        &charters.paths(),
        &findings,
        charters.charter_ref(),
    )?;

    Ok(ScanResult {
        version: REPORT_VERSION,
        mode: mode.to_string(),
        clean: findings.is_empty(),
        root: bundle.scan_root.to_string_lossy().to_string(),
        scanned_files: bundle
            .scanned_files
            .iter()
            .map(|path| resolve_display_path(path, &bundle.scan_root, &path.to_string_lossy()))
            .collect(),
        summary,
        capsule,
        report_paths,
        findings,
    })
}

enum ApprovalState {
    Approved,
    AllowedWithoutApproval,
    Invalid(String),
}

fn evaluate_approval(
    finding: &RawFinding,
    policies: &PolicySet,
    scan_root: &Path,
    line_cache: &mut HashMap<PathBuf, Vec<String>>,
) -> ApprovalState {
    if finding
        .metadata
        .get("approval_mode")
        .map(String::as_str)
        .unwrap_or(APPROVAL_MODE_NONE)
        != APPROVAL_MODE_REGISTRY
    {
        return ApprovalState::AllowedWithoutApproval;
    }

    let lines = line_cache
        .entry(finding.canonical_file.clone())
        .or_insert_with(|| {
            fs::read_to_string(&finding.canonical_file)
                .unwrap_or_default()
                .lines()
                .map(|line| line.to_string())
                .collect()
        });

    let candidates = [
        Some(finding.line0),
        finding.line0.checked_sub(1),
        finding.line0.checked_sub(2),
    ];
    let owner = guess_owner_layer(&finding.canonical_file, scan_root, policies);

    for index in candidates.into_iter().flatten() {
        if let Some(line) = lines.get(index)
            && let Some(captures) = approval_id_regex().captures(line.trim())
        {
            let approval_id = captures
                .get(1)
                .map(|value| value.as_str().to_string())
                .unwrap_or_default();
            if approval_id.is_empty() {
                continue;
            }
            let Some(entry) = policies.registered_approval(&approval_id) else {
                return ApprovalState::Invalid(
                    "approval id is not registered in policy/defaults.yml".to_string(),
                );
            };
            if !entry.allowed_layers.is_empty() && !entry.allowed_layers.contains(&owner) {
                return ApprovalState::Invalid(format!(
                    "approval id is registered but not allowed in owner layer `{owner}`"
                ));
            }
            if !entry.symbol.is_empty() {
                let symbol_matches = lines
                    .get(finding.line0)
                    .map(|candidate| candidate.contains(&entry.symbol))
                    .unwrap_or(false)
                    || finding.text.contains(&entry.symbol);
                if !symbol_matches {
                    return ApprovalState::Invalid(format!(
                        "approval id is registered for `{}` but the blessed symbol does not match the call site",
                        entry.symbol
                    ));
                }
            }
            return ApprovalState::Approved;
        }
    }

    ApprovalState::Invalid("missing registry-backed approval comment".to_string())
}

fn raw_to_violation(
    raw: &RawFinding,
    policies: &PolicySet,
    scan_root: &Path,
    approval_reason: Option<String>,
) -> FindingRecord {
    let violation_class = raw
        .metadata
        .get("violation_class")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let spec = violation_spec(&violation_class);
    let owner_hint = raw
        .metadata
        .get("owner_hint")
        .cloned()
        .unwrap_or_else(|| spec.default_owner.to_string());
    let owner_layer = {
        let guessed = guess_owner_layer(&raw.canonical_file, scan_root, policies);
        if guessed == "unknown" {
            owner_hint
        } else {
            guessed
        }
    };
    let mut message = raw.message.clone();
    if let Some(reason) = approval_reason {
        message = format!("{message} ({reason})");
    }
    FindingRecord {
        kind: FindingKind::Violation,
        file: raw.display_file.clone(),
        canonical_file: Some(raw.canonical_file.to_string_lossy().to_string()),
        line: Some(raw.line0 + 1),
        rule_id: Some(raw.rule_id.clone()),
        violation_class: Some(violation_class),
        owner_layer: Some(owner_layer),
        context_hint: None,
        surface_hint: None,
        contract_kind: None,
        compatibility: None,
        snippet: raw.snippet(),
        message,
        required_judgements: spec
            .required_judgements
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        remedy_candidates: spec
            .legal_remedies
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        proof_options: spec
            .proof_options
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    }
}

struct ViolationSpec {
    default_owner: &'static str,
    required_judgements: &'static [&'static str],
    legal_remedies: &'static [&'static str],
    proof_options: &'static [&'static str],
}

fn violation_spec(class: &str) -> ViolationSpec {
    match class {
        "fallback_unowned_default" => ViolationSpec {
            default_owner: "boundary",
            required_judgements: &["owner", "default_or_optionality"],
            legal_remedies: &[
                "approved_policy_api",
                "boundary_parser",
                "typed_exception",
                "optional_exhaustive_handling",
            ],
            proof_options: &[
                "registered approval id",
                "parser/schema validation",
                "typed exception test",
            ],
        },
        "fallback_unowned_handler" => ViolationSpec {
            default_owner: "application",
            required_judgements: &["owner", "default_or_optionality"],
            legal_remedies: &[
                "typed_exception",
                "resilience_adapter",
                "optional_exhaustive_handling",
            ],
            proof_options: &["typed exception test", "contract/property test"],
        },
        "boundary_parse_missing" => ViolationSpec {
            default_owner: "boundary",
            required_judgements: &["owner", "contract"],
            legal_remedies: &["boundary_parser", "typed_exception"],
            proof_options: &["parser/schema validation", "contract witness"],
        },
        "runtime_double_in_graph" => ViolationSpec {
            default_owner: "tests",
            required_judgements: &["owner", "adapter"],
            legal_remedies: &["move_double_to_tests", "promote_to_first_class_adapter"],
            proof_options: &["import guard", "contract test"],
        },
        "adapter_choice_outside_composition_root" => ViolationSpec {
            default_owner: "composition_root",
            required_judgements: &["owner", "adapter"],
            legal_remedies: &[
                "promote_to_first_class_adapter",
                "move_choice_to_composition_root",
            ],
            proof_options: &["composition root wiring", "contract test"],
        },
        "surface_hidden_owner_concept" => ViolationSpec {
            default_owner: "boundary",
            required_judgements: &["surface"],
            legal_remedies: &["challenge_interface"],
            proof_options: &["explicit export manifest"],
        },
        _ => ViolationSpec {
            default_owner: "boundary",
            required_judgements: &["owner"],
            legal_remedies: &["typed_exception"],
            proof_options: &["machine-checkable witness"],
        },
    }
}

fn analyze_structure(
    policies: &PolicySet,
    charters: &CharterSet,
    scan_root: &Path,
    contents: &HashMap<PathBuf, String>,
) -> Result<Vec<FindingRecord>, String> {
    let mut findings = Vec::new();
    for (path, content) in contents {
        let display_file = resolve_display_path(path, scan_root, &path.to_string_lossy());
        let owner_layer = guess_owner_layer(path, scan_root, policies);
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or_default();
        let symbols = extract_symbols(extension, content);
        let path_contexts = matched_contexts(&display_file, policies);
        let path_context = if path_contexts.len() == 1 {
            Some(path_contexts[0].clone())
        } else {
            None
        };
        let charter_assignment = charters.context_assignment(&display_file);

        if path_contexts.len() > 1 {
            findings.push(FindingRecord {
                kind: FindingKind::Hole,
                file: display_file.clone(),
                canonical_file: Some(path.to_string_lossy().to_string()),
                line: None,
                rule_id: None,
                violation_class: None,
                owner_layer: Some(owner_layer.clone()),
                context_hint: None,
                surface_hint: None,
                contract_kind: None,
                compatibility: None,
                snippet: String::new(),
                message: format!(
                    "Multiple bounded contexts match `{display_file}`: {}",
                    path_contexts.join(", ")
                ),
                required_judgements: vec!["context".to_string()],
                remedy_candidates: vec!["/witness:charter".to_string()],
                proof_options: vec!["contexts.yml assignment".to_string()],
            });
        }

        if let (Some(path_ctx), Some(charter_ctx)) = (&path_context, &charter_assignment)
            && path_ctx != charter_ctx
        {
            findings.push(FindingRecord {
                kind: FindingKind::Drift,
                file: display_file.clone(),
                canonical_file: Some(path.to_string_lossy().to_string()),
                line: None,
                rule_id: None,
                violation_class: None,
                owner_layer: Some(owner_layer.clone()),
                context_hint: Some(charter_ctx.clone()),
                surface_hint: None,
                contract_kind: None,
                compatibility: None,
                snippet: String::new(),
                message: format!(
                    "Charter assigns `{display_file}` to `{charter_ctx}` but path policy resolves to `{path_ctx}`"
                ),
                required_judgements: vec!["context".to_string()],
                remedy_candidates: vec!["align charter and contexts".to_string()],
                proof_options: vec!["contexts.yml".to_string(), "charter assignment".to_string()],
            });
        }

        for symbol in &symbols {
            let decision = classify_surface(
                &symbol.name,
                policies,
                charters.public_symbol_decision(&display_file, &symbol.name),
            );
            if !decision.is_public_like() {
                continue;
            }

            if policies
                .surfaces
                .rules
                .forbid_restricted_visibility_for_public_concepts
                && symbol.restricted
            {
                findings.push(FindingRecord {
                    kind: FindingKind::Violation,
                    file: display_file.clone(),
                    canonical_file: Some(path.to_string_lossy().to_string()),
                    line: Some(symbol.line),
                    rule_id: Some(match extension {
                        "py" => "py-no-hidden-owner-concept".to_string(),
                        _ => "ts-no-hidden-owner-concept".to_string(),
                    }),
                    violation_class: Some("surface_hidden_owner_concept".to_string()),
                    owner_layer: Some(owner_layer.clone()),
                    context_hint: path_context.clone(),
                    surface_hint: Some(decision.label().to_string()),
                    contract_kind: None,
                    compatibility: None,
                    snippet: symbol.snippet.clone(),
                    message: format!(
                        "Public owner-layer concept `{}` is hidden behind restricted visibility",
                        symbol.name
                    ),
                    required_judgements: vec!["surface".to_string()],
                    remedy_candidates: vec!["challenge_interface".to_string()],
                    proof_options: vec!["explicit export manifest".to_string()],
                });
            }

            if policies
                .surfaces
                .rules
                .require_explicit_export_manifest_for_new_public_symbols
                && !symbol.exported
            {
                let kind = if charters
                    .public_symbol_decision(&display_file, &symbol.name)
                    .is_some()
                {
                    FindingKind::Obligation
                } else {
                    FindingKind::Drift
                };
                findings.push(FindingRecord {
                    kind,
                    file: display_file.clone(),
                    canonical_file: Some(path.to_string_lossy().to_string()),
                    line: Some(symbol.line),
                    rule_id: None,
                    violation_class: None,
                    owner_layer: Some(owner_layer.clone()),
                    context_hint: path_context.clone(),
                    surface_hint: Some(decision.label().to_string()),
                    contract_kind: None,
                    compatibility: None,
                    snippet: symbol.snippet.clone(),
                    message: format!(
                        "Public symbol `{}` is missing an explicit export witness",
                        symbol.name
                    ),
                    required_judgements: vec!["surface".to_string()],
                    remedy_candidates: vec!["add export manifest".to_string()],
                    proof_options: vec!["__all__/named export/pub".to_string()],
                });
            }

            if should_check_contexts(&display_file, policies, &charter_assignment) {
                let vocab_matches = vocabulary_contexts(&symbol.name, policies);
                if path_context.is_none()
                    && charter_assignment.is_none()
                    && vocab_matches.len() != 1
                {
                    findings.push(FindingRecord {
                        kind: FindingKind::Hole,
                        file: display_file.clone(),
                        canonical_file: Some(path.to_string_lossy().to_string()),
                        line: Some(symbol.line),
                        rule_id: None,
                        violation_class: None,
                        owner_layer: Some(owner_layer.clone()),
                        context_hint: None,
                        surface_hint: Some(decision.label().to_string()),
                        contract_kind: None,
                        compatibility: None,
                        snippet: symbol.snippet.clone(),
                        message: format!(
                            "Public symbol `{}` needs a bounded context assignment",
                            symbol.name
                        ),
                        required_judgements: vec!["context".to_string()],
                        remedy_candidates: vec!["/witness:charter".to_string()],
                        proof_options: vec!["contexts.yml assignment".to_string()],
                    });
                }
                if let (Some(path_ctx), false) = (&path_context, vocab_matches.is_empty()) {
                    findings.push(FindingRecord {
                        kind: FindingKind::Drift,
                        file: display_file.clone(),
                        canonical_file: Some(path.to_string_lossy().to_string()),
                        line: Some(symbol.line),
                        rule_id: None,
                        violation_class: None,
                        owner_layer: Some(owner_layer.clone()),
                        context_hint: Some(path_ctx.clone()),
                        surface_hint: Some(decision.label().to_string()),
                        contract_kind: None,
                        compatibility: None,
                        snippet: symbol.snippet.clone(),
                        message: format!(
                            "Public symbol `{}` does not match the vocabulary of context `{}`",
                            symbol.name, path_ctx
                        ),
                        required_judgements: vec!["context".to_string()],
                        remedy_candidates: vec!["rename or reassign context".to_string()],
                        proof_options: vec!["contexts.yml vocabulary".to_string()],
                    });
                }
            }
        }

        if owner_layer == "boundary" && has_boundary_signal(content, &symbols) {
            let current_context = charter_assignment.clone().or(path_context.clone());
            let has_contract = current_context
                .as_ref()
                .map(|context| {
                    policies.contracts.contracts.values().any(|contract| {
                        contract.context == *context
                            && contract.owner_layer == "boundary"
                            && !contract.kind.trim().is_empty()
                    })
                })
                .unwrap_or(false);

            if !has_contract {
                findings.push(FindingRecord {
                    kind: FindingKind::Hole,
                    file: display_file.clone(),
                    canonical_file: Some(path.to_string_lossy().to_string()),
                    line: None,
                    rule_id: None,
                    violation_class: None,
                    owner_layer: Some(owner_layer.clone()),
                    context_hint: current_context,
                    surface_hint: None,
                    contract_kind: Some("shape".to_string()),
                    compatibility: None,
                    snippet: String::new(),
                    message: format!(
                        "Boundary parser or DTO work exists in `{display_file}` but no contract witness is defined"
                    ),
                    required_judgements: vec!["contract".to_string()],
                    remedy_candidates: vec!["/witness:charter".to_string()],
                    proof_options: vec!["policy/contracts.yml".to_string()],
                });
            }
        }
    }
    Ok(findings)
}

fn analyze_charter_expectations(
    policies: &PolicySet,
    charters: &CharterSet,
    scan_root: &Path,
) -> Result<Vec<FindingRecord>, String> {
    let mut findings = Vec::new();
    let registered_contracts = policies.all_contract_ids();
    let registered_adapters = policies.registered_adapters();

    for item in &charters.items {
        let charter_file = item.path.to_string_lossy().to_string();
        let charter_display = resolve_display_path(&item.path, scan_root, &charter_file);

        for hole in &item.charter.holes {
            findings.push(FindingRecord {
                kind: FindingKind::Hole,
                file: charter_display.clone(),
                canonical_file: Some(charter_file.clone()),
                line: None,
                rule_id: None,
                violation_class: None,
                owner_layer: None,
                context_hint: None,
                surface_hint: None,
                contract_kind: None,
                compatibility: None,
                snippet: String::new(),
                message: format!("Unresolved charter hole [{}]: {}", hole.kind, hole.question),
                required_judgements: vec![hole.kind.clone()],
                remedy_candidates: vec!["/witness:charter".to_string()],
                proof_options: vec!["resolve charter hole".to_string()],
            });
        }

        for contract in &item.charter.contracts.add {
            if !registered_contracts.contains(&contract.id) {
                findings.push(FindingRecord {
                    kind: FindingKind::Obligation,
                    file: charter_display.clone(),
                    canonical_file: Some(charter_file.clone()),
                    line: None,
                    rule_id: None,
                    violation_class: None,
                    owner_layer: None,
                    context_hint: None,
                    surface_hint: None,
                    contract_kind: Some(contract.kind.clone()),
                    compatibility: Some(contract.compatibility.clone()),
                    snippet: String::new(),
                    message: format!(
                        "Charter declares contract `{}` but policy/contracts.yml has not been updated",
                        contract.id
                    ),
                    required_judgements: vec!["contract".to_string()],
                    remedy_candidates: vec!["compile constitution".to_string()],
                    proof_options: vec!["policy/contracts.yml".to_string()],
                });
                continue;
            }

            if let Some(policy_contract) = policies.contracts.contracts.get(&contract.id) {
                if !policy_contract.schema.is_empty() {
                    let schema_path = scan_root.join(&policy_contract.schema);
                    if !schema_path.exists() {
                        findings.push(FindingRecord {
                            kind: FindingKind::Obligation,
                            file: charter_display.clone(),
                            canonical_file: Some(charter_file.clone()),
                            line: None,
                            rule_id: None,
                            violation_class: None,
                            owner_layer: None,
                            context_hint: Some(policy_contract.context.clone()),
                            surface_hint: None,
                            contract_kind: Some(policy_contract.kind.clone()),
                            compatibility: Some(policy_contract.compatibility.clone()),
                            snippet: String::new(),
                            message: format!(
                                "Contract `{}` requires schema `{}` but the file does not exist",
                                contract.id, policy_contract.schema
                            ),
                            required_judgements: vec!["contract".to_string()],
                            remedy_candidates: vec!["add schema witness".to_string()],
                            proof_options: vec!["schema file".to_string()],
                        });
                    }
                }
                for witness in &policy_contract.witnesses {
                    if !scan_root.join(witness).exists() {
                        findings.push(FindingRecord {
                            kind: FindingKind::Obligation,
                            file: charter_display.clone(),
                            canonical_file: Some(charter_file.clone()),
                            line: None,
                            rule_id: None,
                            violation_class: None,
                            owner_layer: None,
                            context_hint: Some(policy_contract.context.clone()),
                            surface_hint: None,
                            contract_kind: Some(policy_contract.kind.clone()),
                            compatibility: Some(policy_contract.compatibility.clone()),
                            snippet: String::new(),
                            message: format!(
                                "Contract `{}` requires witness `{}` but the file does not exist",
                                contract.id, witness
                            ),
                            required_judgements: vec!["contract".to_string()],
                            remedy_candidates: vec!["add contract witness".to_string()],
                            proof_options: vec!["contract test".to_string()],
                        });
                    }
                }
            }
        }

        for approval in &item.charter.defaults.approvals {
            if !policies.defaults.defaults.contains_key(approval) {
                findings.push(FindingRecord {
                    kind: FindingKind::Obligation,
                    file: charter_display.clone(),
                    canonical_file: Some(charter_file.clone()),
                    line: None,
                    rule_id: None,
                    violation_class: None,
                    owner_layer: None,
                    context_hint: None,
                    surface_hint: None,
                    contract_kind: None,
                    compatibility: None,
                    snippet: String::new(),
                    message: format!(
                        "Charter references approval `{approval}` but policy/defaults.yml is missing it"
                    ),
                    required_judgements: vec!["default_or_optionality".to_string()],
                    remedy_candidates: vec!["compile constitution".to_string()],
                    proof_options: vec!["policy/defaults.yml".to_string()],
                });
            }
        }

        for adapter in &item.charter.adapters.add {
            if !registered_adapters.contains(adapter) {
                findings.push(FindingRecord {
                    kind: FindingKind::Obligation,
                    file: charter_display.clone(),
                    canonical_file: Some(charter_file.clone()),
                    line: None,
                    rule_id: None,
                    violation_class: None,
                    owner_layer: None,
                    context_hint: None,
                    surface_hint: None,
                    contract_kind: None,
                    compatibility: None,
                    snippet: String::new(),
                    message: format!(
                        "Charter declares adapter `{adapter}` but policy/adapters.yml does not register it"
                    ),
                    required_judgements: vec!["adapter".to_string()],
                    remedy_candidates: vec!["compile constitution".to_string()],
                    proof_options: vec!["policy/adapters.yml".to_string()],
                });
            }
        }
    }
    Ok(findings)
}

#[derive(Clone)]
struct SymbolRecord {
    name: String,
    line: usize,
    exported: bool,
    restricted: bool,
    snippet: String,
}

fn extract_symbols(extension: &str, content: &str) -> Vec<SymbolRecord> {
    match extension {
        "py" => extract_python_symbols(content),
        "ts" | "cts" | "mts" => extract_typescript_symbols(content),
        _ => Vec::new(),
    }
}

fn extract_python_symbols(content: &str) -> Vec<SymbolRecord> {
    let exports = python_exports(content);
    py_symbol_regex()
        .captures_iter(content)
        .filter_map(|captures| {
            let name = captures.get(2)?.as_str().to_string();
            let start = captures.get(0)?.start();
            let line = content[..start].bytes().filter(|b| *b == b'\n').count() + 1;
            let snippet = content
                .lines()
                .nth(line.saturating_sub(1))
                .unwrap_or_default()
                .trim()
                .to_string();
            Some(SymbolRecord {
                exported: exports.contains(&name),
                restricted: name.starts_with('_'),
                name,
                line,
                snippet,
            })
        })
        .collect()
}

fn python_exports(content: &str) -> BTreeSet<String> {
    let mut exports = BTreeSet::new();
    if let Some(captures) = py_all_regex().captures(content)
        && let Some(values) = captures.get(1)
    {
        for capture in quote_value_regex().captures_iter(values.as_str()) {
            if let Some(symbol) = capture.get(1) {
                exports.insert(symbol.as_str().to_string());
            }
        }
    }
    exports
}

fn extract_typescript_symbols(content: &str) -> Vec<SymbolRecord> {
    ts_symbol_regex()
        .captures_iter(content)
        .filter_map(|captures| {
            let name = captures.get(3)?.as_str().to_string();
            let start = captures.get(0)?.start();
            let line = content[..start].bytes().filter(|b| *b == b'\n').count() + 1;
            let snippet = content
                .lines()
                .nth(line.saturating_sub(1))
                .unwrap_or_default()
                .trim()
                .to_string();
            Some(SymbolRecord {
                exported: captures.get(1).is_some(),
                restricted: name.starts_with('_'),
                name,
                line,
                snippet,
            })
        })
        .collect()
}

#[derive(Clone, Copy)]
enum SurfaceDecision {
    PublicConcept,
    SubclassApi,
    Internal,
}

impl SurfaceDecision {
    fn is_public_like(self) -> bool {
        !matches!(self, Self::Internal)
    }

    fn label(self) -> &'static str {
        match self {
            Self::PublicConcept => "public_concept",
            Self::SubclassApi => "subclass_api",
            Self::Internal => "internal_mechanic",
        }
    }
}

fn classify_surface(
    name: &str,
    policies: &PolicySet,
    charter_decision: Option<String>,
) -> SurfaceDecision {
    if let Some(decision) = charter_decision.as_deref() {
        return match decision {
            "public_concept" => SurfaceDecision::PublicConcept,
            "subclass_api" => SurfaceDecision::SubclassApi,
            _ => SurfaceDecision::Internal,
        };
    }
    let base = name.trim_start_matches('_');
    if policies
        .surfaces
        .extension_api_patterns
        .iter()
        .any(|pattern| glob_name_matches(base, pattern))
    {
        return SurfaceDecision::SubclassApi;
    }
    if policies
        .surfaces
        .public_by_default
        .concept_patterns
        .iter()
        .any(|pattern| glob_name_matches(base, pattern))
    {
        return SurfaceDecision::PublicConcept;
    }
    SurfaceDecision::Internal
}

fn glob_name_matches(name: &str, pattern: &str) -> bool {
    Pattern::new(pattern)
        .map(|value| value.matches(name))
        .unwrap_or(false)
}

fn matched_contexts(file_key: &str, policies: &PolicySet) -> Vec<String> {
    policies
        .contexts
        .contexts
        .iter()
        .filter_map(|(context, config)| {
            if config
                .paths
                .iter()
                .filter_map(|pattern| Pattern::new(pattern).ok())
                .any(|pattern| pattern.matches(file_key))
            {
                Some(context.clone())
            } else {
                None
            }
        })
        .collect()
}

fn vocabulary_contexts(symbol: &str, policies: &PolicySet) -> Vec<String> {
    let tokens = tokenize_symbol(symbol);
    policies
        .contexts
        .contexts
        .iter()
        .filter_map(|(context, config)| {
            let has_match = config
                .vocabulary
                .nouns
                .iter()
                .chain(config.vocabulary.verbs.iter())
                .map(|value| value.to_ascii_lowercase())
                .any(|value| tokens.contains(&value));
            if has_match {
                Some(context.clone())
            } else {
                None
            }
        })
        .collect()
}

fn tokenize_symbol(symbol: &str) -> BTreeSet<String> {
    let mut normalized = String::new();
    let mut previous_is_lower_or_digit = false;
    for ch in symbol.trim_start_matches('_').chars() {
        if (ch == '_' || ch == '-' || ch.is_whitespace()) && !normalized.ends_with('_') {
            normalized.push('_');
            previous_is_lower_or_digit = false;
            continue;
        }
        if ch.is_uppercase() && previous_is_lower_or_digit && !normalized.ends_with('_') {
            normalized.push('_');
        }
        previous_is_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        normalized.push(ch);
    }
    normalized
        .split(|ch: char| ch == '_' || ch == '-' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_ascii_lowercase())
        .collect()
}

fn should_check_contexts(
    file_key: &str,
    policies: &PolicySet,
    charter_assignment: &Option<String>,
) -> bool {
    charter_assignment.is_some() || !matched_contexts(file_key, policies).is_empty()
}

fn has_boundary_signal(content: &str, symbols: &[SymbolRecord]) -> bool {
    content.contains("BaseModel")
        || content.contains("model_validate(")
        || content.contains("z.object(")
        || content.contains(".parse(")
        || symbols.iter().any(|symbol| {
            let base = symbol.name.trim_start_matches('_');
            ["Payload", "Request", "Response", "Parser", "Settings"]
                .iter()
                .any(|suffix| base.ends_with(suffix))
        })
}

fn dedupe_findings(findings: &mut Vec<FindingRecord>) {
    let mut seen = BTreeSet::new();
    findings.retain(|finding| {
        let key = format!(
            "{}:{}:{}:{}",
            finding.file,
            finding.line.unwrap_or_default(),
            finding.kind.as_str(),
            finding.message
        );
        seen.insert(key)
    });
}

fn build_summary(findings: &[FindingRecord], files_scanned: usize) -> ScanSummary {
    let mut summary = ScanSummary {
        files_scanned,
        ..ScanSummary::default()
    };
    for finding in findings {
        match finding.kind {
            FindingKind::Violation => summary.violations += 1,
            FindingKind::Hole => summary.holes += 1,
            FindingKind::Drift => summary.drift += 1,
            FindingKind::Obligation => summary.obligations += 1,
        }
        *summary
            .by_kind
            .entry(finding.kind.as_str().to_string())
            .or_insert(0) += 1;
        *summary.by_file.entry(finding.file.clone()).or_insert(0) += 1;
    }
    summary
}

fn build_capsule(findings: &[FindingRecord]) -> String {
    let kinds = join_unique(
        findings
            .iter()
            .map(|finding| finding.kind.as_str().to_string()),
    );
    let classes = join_unique(
        findings
            .iter()
            .filter_map(|finding| finding.violation_class.clone()),
    );
    let owners = join_unique(
        findings
            .iter()
            .filter_map(|finding| finding.owner_layer.clone()),
    );
    let remedies = join_unique(
        findings
            .iter()
            .flat_map(|finding| finding.remedy_candidates.iter().cloned()),
    );
    format!(
        "witness count={} kinds={} classes={} owners={} remedies={}",
        findings.len(),
        kinds,
        classes,
        owners,
        remedies
    )
}

fn join_unique(values: impl IntoIterator<Item = String>) -> String {
    let mut set = BTreeSet::new();
    for value in values {
        if !value.trim().is_empty() {
            set.insert(value);
        }
    }
    set.into_iter().collect::<Vec<_>>().join("|")
}

fn persist_pending_reports(
    report_dir: Option<&Path>,
    scan_root: &Path,
    scanned_files: &[PathBuf],
    charter_files: &[PathBuf],
    findings: &[FindingRecord],
    charter_ref: Option<String>,
) -> Result<Vec<String>, String> {
    let Some(report_dir) = report_dir else {
        return Ok(Vec::new());
    };
    let pending_dir = report_dir.join("pending");
    let history_dir = report_dir.join("history");
    fs::create_dir_all(&pending_dir)
        .map_err(|err| format!("failed to create {}: {err}", pending_dir.display()))?;
    fs::create_dir_all(&history_dir)
        .map_err(|err| format!("failed to create {}: {err}", history_dir.display()))?;

    for path in scanned_files.iter().chain(charter_files.iter()) {
        let pending_path =
            pending_dir.join(format!("{}.json", stable_key(&path.to_string_lossy())));
        let _ = fs::remove_file(pending_path);
    }

    if findings.is_empty() {
        return Ok(Vec::new());
    }

    let mut grouped: BTreeMap<String, Vec<FindingRecord>> = BTreeMap::new();
    for finding in findings {
        let canonical = finding
            .canonical_file
            .clone()
            .unwrap_or_else(|| finding.file.clone());
        grouped.entry(canonical).or_default().push(finding.clone());
    }

    let created_at = timestamp_rfc3339();
    let mut report_paths = Vec::new();
    let mut index = 0usize;

    for (canonical_file, group) in grouped {
        index += 1;
        let file_path = PathBuf::from(&canonical_file);
        let display_file = resolve_display_path(&file_path, scan_root, &canonical_file);
        let report = PendingReport {
            version: REPORT_VERSION,
            report_id: format!(
                "wg-{}-{index:04}-{}",
                report_timestamp_token(),
                stable_key(&canonical_file)
            ),
            created_at: created_at.clone(),
            status: if group
                .iter()
                .any(|finding| finding.kind == FindingKind::Hole)
            {
                ReportStatus::NeedsCharterDecision
            } else {
                ReportStatus::Pending
            },
            charter_ref: charter_ref.clone(),
            file: display_file,
            canonical_file: canonical_file.clone(),
            summary: build_summary(&group, 1),
            findings: group,
        };
        let pending_path = pending_dir.join(format!("{}.json", stable_key(&canonical_file)));
        let history_path = history_dir.join(format!("{}.json", report.report_id));
        write_pretty_json(&pending_path, &report)?;
        write_pretty_json(&history_path, &report)?;
        report_paths.push(pending_path.to_string_lossy().to_string());
    }

    Ok(report_paths)
}

fn scan_stop(common: &CommonOptions) -> Result<StopScanResult, String> {
    let report_dir = common
        .report_dir
        .clone()
        .unwrap_or_else(|| common.config_dir.join(".witness-data/reports"));
    let pending_dir = report_dir.join("pending");
    if !pending_dir.is_dir() {
        return Ok(StopScanResult {
            version: REPORT_VERSION,
            clean: true,
            report_dir: report_dir.to_string_lossy().to_string(),
            pending_count: 0,
            summary: ScanSummary::default(),
            capsule: None,
            pending_reports: Vec::new(),
        });
    }

    let mut reports = Vec::new();
    for entry in fs::read_dir(&pending_dir)
        .map_err(|err| format!("failed to read {}: {err}", pending_dir.display()))?
    {
        let path = entry
            .map_err(|err| format!("failed to read pending report entry: {err}"))?
            .path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let report = read_pending_report(&path)?;
        if !Path::new(&report.canonical_file).exists() {
            let _ = fs::remove_file(&path);
            continue;
        }
        reports.push(report);
    }

    let findings: Vec<FindingRecord> = reports
        .iter()
        .flat_map(|report| report.findings.clone())
        .collect();
    let capsule = if findings.is_empty() {
        None
    } else {
        Some(build_capsule(&findings))
    };
    let summary = build_summary(&findings, reports.len());

    Ok(StopScanResult {
        version: REPORT_VERSION,
        clean: reports.is_empty(),
        report_dir: report_dir.to_string_lossy().to_string(),
        pending_count: reports.len(),
        summary,
        capsule,
        pending_reports: reports
            .into_iter()
            .map(|report| PendingReportSummary {
                report_id: report.report_id,
                file: report.file.clone(),
                canonical_file: report.canonical_file,
                status: report.status,
                summary: report.summary,
                capsule: build_capsule(&report.findings),
            })
            .collect(),
    })
}

fn resolve_display_path(canonical: &Path, scan_root: &Path, raw_fallback: &str) -> String {
    canonical
        .strip_prefix(scan_root)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| raw_fallback.to_string())
}

fn guess_owner_layer(path: &Path, scan_root: &Path, policies: &PolicySet) -> String {
    let relative = match path.strip_prefix(scan_root) {
        Ok(value) => value,
        Err(_) => return "unknown".to_string(),
    };
    for layer in OWNER_PRECEDENCE {
        let Some(globs) = policies.ownership.layers.get(*layer) else {
            continue;
        };
        for glob in globs {
            if let Ok(pattern) = Pattern::new(glob)
                && pattern.matches_path_with(relative, GLOB_OPTS)
            {
                return (*layer).to_string();
            }
        }
    }
    "unknown".to_string()
}

fn build_patterns(patterns: &[String]) -> Result<Vec<Pattern>, String> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).map_err(|err| format!("invalid glob {pattern}: {err}"))
        })
        .collect()
}

fn build_skip_matcher(test_globs: &[String]) -> Result<Vec<Pattern>, String> {
    let mut all = test_globs.to_vec();
    all.extend(DEFAULT_SKIP_GLOBS.iter().map(|value| (*value).to_string()));
    build_patterns(&all)
}

fn matches_skip_globs(matcher: &[Pattern], path: &Path, scan_root: &Path) -> bool {
    let relative = path.strip_prefix(scan_root).unwrap_or(path);
    matcher
        .iter()
        .any(|pattern| pattern.matches_path_with(relative, GLOB_OPTS))
}

fn is_supported_source(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("py" | "ts" | "cts" | "mts")
    )
}

fn build_post_tooluse_hook_response(result: &ScanResult) -> serde_json::Value {
    let report_path = result
        .report_paths
        .first()
        .cloned()
        .unwrap_or_else(|| "<pending-report>".to_string());
    json!({
        "decision": "block",
        "reason": format!(
            "Witness: {} finding(s) remain (violations={}, holes={}, drift={}, obligations={}). Resolve them before continuing.",
            result.findings.len(),
            result.summary.violations,
            result.summary.holes,
            result.summary.drift,
            result.summary.obligations
        ),
        "systemMessage": format!("Detailed report saved to {report_path}"),
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse",
            "additionalContext": result.capsule.clone().unwrap_or_default()
        }
    })
}

fn build_stop_hook_response(result: &StopScanResult) -> serde_json::Value {
    let first_report = result
        .pending_reports
        .first()
        .map(|report| report.file.clone())
        .unwrap_or_else(|| "<unknown>".to_string());
    json!({
        "decision": "block",
        "reason": format!(
            "Witness: {} unresolved pending report(s) remain (violations={}, holes={}, drift={}, obligations={}). Fix or charter them before stopping. First pending file: {}.",
            result.pending_count,
            result.summary.violations,
            result.summary.holes,
            result.summary.drift,
            result.summary.obligations,
            first_report
        ),
        "systemMessage": format!("Unresolved witness reports remain under {}/pending", result.report_dir)
    })
}

fn write_pretty_json<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let text = serde_json::to_string_pretty(value)
        .map_err(|err| format!("failed to serialize {}: {err}", path.display()))?;
    fs::write(path, format!("{text}\n"))
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn print_json<T>(value: &T) -> Result<(), String>
where
    T: Serialize,
{
    let payload = serde_json::to_string_pretty(value)
        .map_err(|err| format!("failed to serialize JSON output: {err}"))?;
    let mut out = io::stdout().lock();
    writeln!(out, "{payload}").map_err(|err| format!("failed to write output: {err}"))
}

fn stable_key(input: &str) -> String {
    let mut hash: u64 = 14695981039346656037;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{hash:016x}")
}

fn report_timestamp_token() -> String {
    format!("{:020}", OffsetDateTime::now_utc().unix_timestamp_nanos())
}

fn timestamp_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn read_pending_report(path: &Path) -> Result<PendingReport, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    let version = value.get("version").and_then(|field| field.as_u64());
    if version != Some(u64::from(REPORT_VERSION)) {
        return Err(format!(
            "unsupported pending report schema in {}: expected version {}",
            path.display(),
            REPORT_VERSION
        ));
    }
    serde_json::from_value(value)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

struct HookContext {
    scan_root: PathBuf,
    file_path: PathBuf,
}

fn extract_hook_context(input: &str) -> Result<HookContext, String> {
    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| format!("failed to parse stdin JSON: {err}"))?;
    let scan_root = value["cwd"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let file_path = value["tool_input"]["file_path"]
        .as_str()
        .or_else(|| value["tool_input"]["filePath"].as_str())
        .or_else(|| value["tool_input"]["path"].as_str())
        .or_else(|| value["tool_response"]["filePath"].as_str())
        .or_else(|| value["tool_response"]["file_path"].as_str())
        .map(PathBuf::from)
        .unwrap_or_default();
    Ok(HookContext {
        scan_root,
        file_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_policies() -> PolicySet {
        let mut ownership = OwnershipPolicyFile::default();
        ownership
            .layers
            .insert("boundary".to_string(), vec!["src/api/**".to_string()]);
        ownership.layers.insert(
            "composition_root".to_string(),
            vec!["src/bootstrap.py".to_string()],
        );
        ownership
            .layers
            .insert("tests".to_string(), vec!["tests/**".to_string()]);

        let mut defaults = DefaultsPolicyFile::default();
        defaults.defaults.insert(
            "REQ-123".to_string(),
            ApprovedDefault {
                symbol: "LocalePolicy.default_locale".to_string(),
                allowed_layers: vec!["boundary".to_string()],
                reason: "locale".to_string(),
            },
        );

        let mut adapters = AdapterPolicyFile::default();
        adapters.ports.insert(
            "UserRepository".to_string(),
            PortPolicy {
                allowed_runtime_adapters: vec!["SqlUserRepository".to_string()],
                contract_tests: vec![
                    "tests/contracts/test_user_repository_contract.py".to_string(),
                ],
            },
        );

        let surfaces = SurfacePolicyFile {
            public_by_default: SurfacePatterns {
                concept_patterns: vec![
                    "*Payload".to_string(),
                    "*Policy".to_string(),
                    "*Adapter".to_string(),
                ],
            },
            extension_api_patterns: vec!["*Base".to_string()],
            rules: SurfaceRules {
                forbid_restricted_visibility_for_public_concepts: true,
                require_explicit_export_manifest_for_new_public_symbols: true,
            },
        };

        let mut contexts = ContextPolicyFile::default();
        contexts.contexts.insert(
            "api_boundary".to_string(),
            ContextPolicy {
                paths: vec!["src/api/**".to_string()],
                vocabulary: ContextVocabulary {
                    nouns: vec!["Payload".to_string(), "Request".to_string()],
                    verbs: vec!["parse".to_string()],
                },
                may_depend_on: Vec::new(),
                public_entrypoints: vec!["src/api/__init__.py".to_string()],
            },
        );

        PolicySet {
            ownership,
            defaults,
            adapters,
            surfaces,
            contracts: ContractPolicyFile::default(),
            contexts,
        }
    }

    #[test]
    fn detect_rule_ids_catches_membership_rewrite() {
        let ids = detect_rule_ids(
            Path::new("sample.py"),
            "tool_use_id = tool_use[\"toolUseId\"] if \"toolUseId\" in tool_use else \"tool\"\n",
        );
        assert!(
            ids.iter()
                .any(|id| id == "py-no-fallback-conditional-membership-default")
        );
    }

    #[test]
    fn guess_owner_layer_uses_policy_globs() {
        let policies = sample_policies();
        let root = PathBuf::from("/repo");
        let file = PathBuf::from("/repo/src/api/handler.py");
        assert_eq!(guess_owner_layer(&file, &root, &policies), "boundary");
    }

    #[test]
    fn build_capsule_collects_unique_values() {
        let findings = vec![FindingRecord {
            kind: FindingKind::Violation,
            file: "src/api/tool_use.py".to_string(),
            canonical_file: None,
            line: Some(3),
            rule_id: Some("py-no-fallback-get-default".to_string()),
            violation_class: Some("fallback_unowned_default".to_string()),
            owner_layer: Some("boundary".to_string()),
            context_hint: None,
            surface_hint: None,
            contract_kind: None,
            compatibility: None,
            snippet: "x = y.get(\"k\", 1)".to_string(),
            message: "bad".to_string(),
            required_judgements: vec!["owner".to_string()],
            remedy_candidates: vec!["boundary_parser".to_string()],
            proof_options: vec!["parser/schema validation".to_string()],
        }];
        let capsule = build_capsule(&findings);
        assert!(capsule.contains("violation"));
        assert!(capsule.contains("fallback_unowned_default"));
        assert!(capsule.contains("boundary_parser"));
    }

    #[test]
    fn stable_key_is_deterministic() {
        assert_eq!(stable_key("abc"), stable_key("abc"));
        assert_ne!(stable_key("abc"), stable_key("abcd"));
    }

    #[test]
    fn tokenize_symbol_splits_camel_case() {
        let tokens = tokenize_symbol("_ToolUsePayload");
        assert!(tokens.contains("tool"));
        assert!(tokens.contains("use"));
        assert!(tokens.contains("payload"));
    }
}
