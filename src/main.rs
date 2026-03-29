use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, BufReader, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use glob::{MatchOptions, Pattern};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;

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
const OWNER_PRECEDENCE: &[&str] = &[
    "tests",
    "composition_root",
    "boundary",
    "application",
    "domain",
    "infrastructure",
];
const BATCH_SIZE: usize = 128;
const RG_CANDIDATE_PATTERN: &str = r"mock|stub|fake|unittest\.mock|jest-mock|sinon|ts-auto-mock|= .* or |\?\?|\|\||except.*pass|catch|contextlib\.suppress|\.get\(|getattr\(|getenv\(|os\.environ\.get\(|next\(|\? .*:| if .* else |tests?/|fixtures/|__mocks__";

fn approval_id_regex() -> &'static Regex {
    static APPROVAL_RE: OnceLock<Regex> = OnceLock::new();
    APPROVAL_RE.get_or_init(|| {
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

    match cli.mode {
        Mode::ScanFile { file } => {
            let bundle = scan_file(&cli.common, &catalog, &policies, &file, None)?;
            let result = finalize_scan("scan-file", &cli.common, &policies, bundle)?;
            if cli.common.hook_response {
                if !result.clean {
                    print_json(&build_post_tooluse_hook_response(&result))?;
                }
            } else {
                print_json(&result)?;
            }
            Ok(if result.clean { 0 } else { 1 })
        }
        Mode::ScanTree { root } => {
            let bundle = scan_tree(&cli.common, &catalog, &policies, &root)?;
            let result = finalize_scan("scan-tree", &cli.common, &policies, bundle)?;
            if cli.common.hook_response {
                if !result.clean {
                    print_json(&build_post_tooluse_hook_response(&result))?;
                }
            } else {
                print_json(&result)?;
            }
            Ok(if result.clean { 0 } else { 1 })
        }
        Mode::ScanHook => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .map_err(|e| format!("failed to read stdin: {e}"))?;
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
            let result = finalize_scan("scan-hook", &cli.common, &policies, bundle)?;
            if cli.common.hook_response {
                if !result.clean {
                    print_json(&build_post_tooluse_hook_response(&result))?;
                }
            } else {
                print_json(&result)?;
            }
            Ok(if result.clean { 0 } else { 1 })
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

#[derive(Clone)]
struct CommonOptions {
    config_dir: PathBuf,
    ast_grep_bin: String,
    report_dir: Option<PathBuf>,
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
                "scan-hook" => {
                    mode = Some(Mode::ScanHook);
                }
                "scan-stop" => {
                    mode = Some(Mode::ScanStop);
                }
                "--ast-grep-bin" => {
                    common.ast_grep_bin = next_value(&mut iter, "--ast-grep-bin")?;
                }
                "--config-dir" => {
                    common.config_dir = PathBuf::from(next_value(&mut iter, "--config-dir")?);
                }
                "--report-dir" => {
                    common.report_dir = Some(PathBuf::from(next_value(&mut iter, "--report-dir")?));
                }
                "--test-globs" => {
                    common.test_globs = next_value(&mut iter, "--test-globs")?
                        .split(',')
                        .filter(|part| !part.trim().is_empty())
                        .map(|part| part.trim().to_string())
                        .collect();
                }
                "--hook-response" => {
                    common.hook_response = true;
                }
                "--changed-only" => {
                    changed_only = Some(PathBuf::from(next_value(&mut iter, "--changed-only")?));
                }
                value if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}"));
                }
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
        if let Some(report_dir) = &common.report_dir {
            common.report_dir = Some(report_dir.clone());
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
        Some(_) | None => Ok(None),
    }
}

fn next_value(iter: &mut VecDeque<String>, option: &str) -> Result<String, String> {
    iter.pop_front()
        .ok_or_else(|| format!("missing value for {option}"))
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

#[derive(Default, Deserialize)]
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

#[derive(Default)]
struct PolicySet {
    ownership: OwnershipPolicyFile,
    defaults: DefaultsPolicyFile,
    adapters: AdapterPolicyFile,
}

impl PolicySet {
    fn load(config_dir: &Path) -> Result<Self, String> {
        let ownership =
            read_yaml_file::<OwnershipPolicyFile>(&config_dir.join("policy/ownership.yml"))?;
        let defaults =
            read_yaml_file::<DefaultsPolicyFile>(&config_dir.join("policy/defaults.yml"))?;
        let adapters =
            read_yaml_file::<AdapterPolicyFile>(&config_dir.join("policy/adapters.yml"))?;
        Ok(Self {
            ownership,
            defaults,
            adapters,
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
struct Finding {
    display_file: String,
    canonical_file: PathBuf,
    line0: usize,
    rule_id: String,
    message: String,
    text: String,
    metadata: HashMap<String, String>,
}

impl Finding {
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
    findings: Vec<Finding>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct ScanSummary {
    files_scanned: usize,
    violation_count: usize,
    classes: BTreeMap<String, usize>,
    owners: BTreeMap<String, usize>,
    by_file: BTreeMap<String, usize>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Violation {
    file: String,
    canonical_file: String,
    line: usize,
    rule_id: String,
    policy_group: String,
    violation_class: String,
    owner_guess: String,
    owner_hint: String,
    message: String,
    code: String,
    legal_remedies: Vec<String>,
    forbidden_moves: Vec<String>,
    approval_status: String,
    approval_id: Option<String>,
    approval_reason: Option<String>,
}

#[derive(Clone, Serialize)]
struct ScanResult {
    mode: String,
    clean: bool,
    root: String,
    scanned_files: Vec<String>,
    summary: ScanSummary,
    capsule: Option<String>,
    report_paths: Vec<String>,
    violations: Vec<Violation>,
}

impl ScanResult {
    fn empty(mode: &str, root: PathBuf) -> Self {
        Self {
            mode: mode.to_string(),
            clean: true,
            root: root.to_string_lossy().to_string(),
            scanned_files: Vec::new(),
            summary: ScanSummary::default(),
            capsule: None,
            report_paths: Vec::new(),
            violations: Vec::new(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct PendingReport {
    schema_version: u8,
    report_id: String,
    created_at_ms: u64,
    file: String,
    canonical_file: String,
    capsule: String,
    summary: ScanSummary,
    violations: Vec<Violation>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PendingReportSummary {
    report_id: String,
    file: String,
    canonical_file: String,
    capsule: String,
    summary: ScanSummary,
}

#[derive(Serialize)]
struct StopScanResult {
    clean: bool,
    report_dir: String,
    pending_count: usize,
    summary: ScanSummary,
    capsule: Option<String>,
    pending_reports: Vec<PendingReportSummary>,
}

fn scan_file(
    common: &CommonOptions,
    catalog: &RuleCatalog,
    policies: &PolicySet,
    file: &Path,
    scan_root_override: Option<&Path>,
) -> Result<ScanBundle, String> {
    let test_matcher = build_patterns(&common.test_globs)?;
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
    if matches_test_globs(&test_matcher, &canonical_file, &scan_root) {
        return Ok(ScanBundle {
            scan_root,
            scanned_files,
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
            &inline_rules,
        )?);
    }

    dedupe_findings(&mut findings);

    Ok(ScanBundle {
        scan_root,
        scanned_files,
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
    let test_matcher = build_patterns(&common.test_globs)?;
    let mut grouped_files: HashMap<Vec<String>, Vec<PathBuf>> = HashMap::new();
    let mut scanned_files: Vec<PathBuf> = Vec::new();
    let mut supplemental = Vec::new();

    for path in ripgrep_candidate_files(&scan_root)? {
        if !is_supported_source(&path) || matches_test_globs(&test_matcher, &path, &scan_root) {
            continue;
        }

        scanned_files.push(path.clone());

        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
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
                &inline_rules,
            )?);
        }
    }

    dedupe_findings(&mut findings);

    Ok(ScanBundle {
        scan_root,
        scanned_files,
        findings,
    })
}

fn ripgrep_candidate_files(scan_root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("rg")
        .args([
            "--files-with-matches",
            "-e",
            RG_CANDIDATE_PATTERN,
            "-g",
            "*.py",
            "-g",
            "*.ts",
            "-g",
            "*.cts",
            "-g",
            "*.mts",
            ".",
        ])
        .current_dir(scan_root)
        .output()
        .map_err(|err| format!("failed to execute ripgrep: {err}"))?;

    check_exit_ok(&output, "ripgrep candidate scan failed")?;

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
        .collect())
}

fn build_patterns(patterns: &[String]) -> Result<Vec<Pattern>, String> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern).map_err(|err| format!("invalid glob {pattern}: {err}"))
        })
        .collect()
}

const GLOB_OPTS: MatchOptions = MatchOptions {
    require_literal_separator: true,
    require_literal_leading_dot: false,
    case_sensitive: true,
};

fn matches_test_globs(matcher: &[Pattern], path: &Path, scan_root: &Path) -> bool {
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

fn run_ast_grep(
    common: &CommonOptions,
    catalog: &RuleCatalog,
    scan_root: &Path,
    targets: &[PathBuf],
    inline_rules: &str,
) -> Result<Vec<Finding>, String> {
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

    let mut child = command
        .spawn()
        .map_err(|err| format!("could not execute {:?}: {err}", common.ast_grep_bin))?;
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
        findings.push(to_finding(raw, catalog, &common.config_dir, scan_root));
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("failed to wait for ast-grep: {err}"))?;
    check_exit_ok(&output, "ast-grep scan failed")?;
    Ok(findings)
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
        parts.push(strip_files_ignores(&raw));
    }
    Ok(parts.join("\n---\n"))
}

fn strip_files_ignores(rule_text: &str) -> String {
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

fn to_finding(
    raw: AstGrepFinding,
    catalog: &RuleCatalog,
    config_dir: &Path,
    scan_root: &Path,
) -> Finding {
    let rule_id = raw.rule_id.unwrap_or_else(|| "<unknown-rule>".to_string());
    let mut metadata = catalog.metadata_for(&rule_id);
    if let Some(extra) = raw.metadata {
        metadata.extend(extra);
    }

    let is_abs = Path::new(&raw.file_path).is_absolute();
    let joined = if is_abs {
        PathBuf::from(&raw.file_path)
    } else {
        config_dir.join(&raw.file_path)
    };
    let canonical_file = joined.canonicalize().unwrap_or(joined);
    let display_file = resolve_display_path(&canonical_file, scan_root, &raw.file_path);

    Finding {
        display_file,
        canonical_file,
        line0: raw.range.start.line,
        rule_id,
        message: raw.message.unwrap_or_default(),
        text: raw.text.unwrap_or_default(),
        metadata,
    }
}

fn resolve_display_path(canonical: &Path, scan_root: &Path, raw_fallback: &str) -> String {
    canonical
        .strip_prefix(scan_root)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| raw_fallback.to_string())
}

fn supplemental_findings(
    canonical_file: &Path,
    content: &str,
    policies: &PolicySet,
    scan_root: &Path,
) -> Vec<Finding> {
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
                    findings.push(Finding {
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
                                findings.push(Finding {
                                    display_file: display_file.clone(),
                                    canonical_file: canonical_file.to_path_buf(),
                                    line0: line_index,
                                    rule_id: "py-no-adapter-choice-outside-composition-root"
                                        .to_string(),
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
                                        (
                                            "owner_hint".to_string(),
                                            "composition_root".to_string(),
                                        ),
                                        (
                                            "approval_mode".to_string(),
                                            APPROVAL_MODE_NONE.to_string(),
                                        ),
                                    ]),
                                });
                            }
                        }
                    }
                }
            }
            "ts" | "cts" | "mts" => {
                if ts_test_support_import_regex().is_match(line) {
                    findings.push(Finding {
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
                            findings.push(Finding {
                                display_file: display_file.clone(),
                                canonical_file: canonical_file.to_path_buf(),
                                line0: line_index,
                                rule_id: "ts-no-adapter-choice-outside-composition-root"
                                    .to_string(),
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
                                    (
                                        "owner_hint".to_string(),
                                        "composition_root".to_string(),
                                    ),
                                    (
                                        "approval_mode".to_string(),
                                        APPROVAL_MODE_NONE.to_string(),
                                    ),
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

fn dedupe_findings(findings: &mut Vec<Finding>) {
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

#[allow(dead_code)]
enum ApprovalState {
    Approved { id: String, reason: String },
    Invalid { id: Option<String>, reason: String },
    Missing,
}

fn finalize_scan(
    mode: &str,
    common: &CommonOptions,
    policies: &PolicySet,
    bundle: ScanBundle,
) -> Result<ScanResult, String> {
    let mut line_cache: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let mut violations = Vec::new();

    for finding in bundle.findings {
        match evaluate_approval(&finding, policies, &bundle.scan_root, &mut line_cache) {
            ApprovalState::Approved { .. } => {}
            ApprovalState::Invalid { id, reason } => {
                violations.push(to_violation(
                    &finding,
                    policies,
                    &bundle.scan_root,
                    "invalid",
                    id,
                    Some(reason),
                ));
            }
            ApprovalState::Missing => {
                violations.push(to_violation(
                    &finding,
                    policies,
                    &bundle.scan_root,
                    "missing",
                    None,
                    None,
                ));
            }
        }
    }

    let summary = build_summary(&violations, bundle.scanned_files.len());
    let capsule = if violations.is_empty() {
        None
    } else {
        Some(build_capsule(&violations))
    };
    let report_paths = persist_pending_reports(
        common.report_dir.as_deref(),
        &bundle.scan_root,
        &bundle.scanned_files,
        &violations,
    )?;

    Ok(ScanResult {
        mode: mode.to_string(),
        clean: violations.is_empty(),
        root: bundle.scan_root.to_string_lossy().to_string(),
        scanned_files: bundle
            .scanned_files
            .iter()
            .map(|path| resolve_display_path(path, &bundle.scan_root, &path.to_string_lossy()))
            .collect(),
        summary,
        capsule,
        report_paths,
        violations,
    })
}

fn evaluate_approval(
    finding: &Finding,
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
        return ApprovalState::Missing;
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

            let owner = guess_owner_layer(&finding.canonical_file, scan_root, policies);
            if let Some(entry) = policies.registered_approval(&approval_id) {
                if entry.allowed_layers.is_empty() || entry.allowed_layers.contains(&owner) {
                    return ApprovalState::Approved {
                        id: approval_id,
                        reason: entry.reason.clone(),
                    };
                }
                return ApprovalState::Invalid {
                    id: Some(approval_id),
                    reason: format!(
                        "approval id is registered but not allowed in owner layer `{owner}`"
                    ),
                };
            }

            return ApprovalState::Invalid {
                id: Some(approval_id),
                reason: "approval id is not registered in policy/defaults.yml".to_string(),
            };
        }
    }

    ApprovalState::Missing
}

fn to_violation(
    finding: &Finding,
    policies: &PolicySet,
    scan_root: &Path,
    approval_status: &str,
    approval_id: Option<String>,
    approval_reason: Option<String>,
) -> Violation {
    let violation_class = finding
        .metadata
        .get("violation_class")
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let spec = violation_spec(&violation_class);
    let owner_hint = finding
        .metadata
        .get("owner_hint")
        .cloned()
        .unwrap_or_else(|| spec.default_owner.to_string());
    let owner_guess = {
        let guessed = guess_owner_layer(&finding.canonical_file, scan_root, policies);
        if guessed == "unknown" {
            owner_hint.clone()
        } else {
            guessed
        }
    };

    let message = if let Some(reason) = &approval_reason {
        format!("{} ({reason})", finding.message)
    } else {
        finding.message.clone()
    };

    Violation {
        file: finding.display_file.clone(),
        canonical_file: finding.canonical_file.to_string_lossy().to_string(),
        line: finding.line0 + 1,
        rule_id: finding.rule_id.clone(),
        policy_group: finding
            .metadata
            .get("policy_group")
            .cloned()
            .unwrap_or_default(),
        violation_class,
        owner_guess,
        owner_hint,
        message,
        code: finding.snippet(),
        legal_remedies: spec
            .legal_remedies
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        forbidden_moves: spec
            .forbidden_moves
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        approval_status: approval_status.to_string(),
        approval_id,
        approval_reason,
    }
}

struct ViolationSpec {
    default_owner: &'static str,
    legal_remedies: &'static [&'static str],
    forbidden_moves: &'static [&'static str],
}

fn violation_spec(class: &str) -> ViolationSpec {
    match class {
        "fallback_unowned_default" => ViolationSpec {
            default_owner: "boundary",
            legal_remedies: &[
                "approved_policy_api",
                "boundary_parser",
                "typed_exception",
                "optional_exhaustive_handling",
            ],
            forbidden_moves: &["rename", "equivalent_rewrite", "new_inline_default"],
        },
        "fallback_unowned_handler" => ViolationSpec {
            default_owner: "application",
            legal_remedies: &[
                "typed_exception",
                "resilience_adapter",
                "optional_exhaustive_handling",
            ],
            forbidden_moves: &["rename", "equivalent_rewrite", "swallow_error"],
        },
        "boundary_parse_missing" => ViolationSpec {
            default_owner: "boundary",
            legal_remedies: &["boundary_parser", "approved_policy_api", "typed_exception"],
            forbidden_moves: &["rename", "equivalent_rewrite", "scattered_env_defaults"],
        },
        "runtime_double_in_graph" => ViolationSpec {
            default_owner: "tests",
            legal_remedies: &["move_double_to_tests", "promote_to_first_class_adapter"],
            forbidden_moves: &["rename", "keep_test_support_in_runtime"],
        },
        "adapter_choice_outside_composition_root" => ViolationSpec {
            default_owner: "composition_root",
            legal_remedies: &[
                "promote_to_first_class_adapter",
                "move_choice_to_composition_root",
            ],
            forbidden_moves: &["rename", "mid_flow_adapter_selection"],
        },
        _ => ViolationSpec {
            default_owner: "boundary",
            legal_remedies: &["typed_exception"],
            forbidden_moves: &["rename"],
        },
    }
}

fn build_summary(violations: &[Violation], files_scanned: usize) -> ScanSummary {
    let mut summary = ScanSummary {
        files_scanned,
        violation_count: violations.len(),
        ..ScanSummary::default()
    };

    for violation in violations {
        *summary
            .classes
            .entry(violation.violation_class.clone())
            .or_insert(0) += 1;
        *summary
            .owners
            .entry(violation.owner_guess.clone())
            .or_insert(0) += 1;
        *summary.by_file.entry(violation.file.clone()).or_insert(0) += 1;
    }

    summary
}

fn build_capsule(violations: &[Violation]) -> String {
    let classes = join_unique(violations.iter().map(|v| v.violation_class.clone()));
    let owners = join_unique(violations.iter().map(|v| v.owner_guess.clone()));
    let remedies = join_unique(
        violations
            .iter()
            .flat_map(|v| v.legal_remedies.iter().cloned()),
    );
    let forbidden = join_unique(
        violations
            .iter()
            .flat_map(|v| v.forbidden_moves.iter().cloned()),
    );

    format!(
        "guardrail count={} classes={} owners={} remedies={} forbidden={}",
        violations.len(),
        classes,
        owners,
        remedies,
        forbidden
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
    violations: &[Violation],
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

    for file in scanned_files {
        let pending_path =
            pending_dir.join(format!("{}.json", stable_key(&file.to_string_lossy())));
        let _ = fs::remove_file(pending_path);
    }

    if violations.is_empty() {
        return Ok(Vec::new());
    }

    let mut grouped: BTreeMap<String, Vec<Violation>> = BTreeMap::new();
    for violation in violations {
        grouped
            .entry(violation.canonical_file.clone())
            .or_default()
            .push(violation.clone());
    }

    let now_ms = timestamp_millis();
    let mut index = 0usize;
    let mut report_paths = Vec::new();

    for (canonical_file, group) in grouped {
        index += 1;
        let file_path = PathBuf::from(&canonical_file);
        let report_id = format!("cg-{now_ms}-{index:04}");
        let display_file = resolve_display_path(&file_path, scan_root, &canonical_file);
        let summary = build_summary(&group, 1);
        let capsule = build_capsule(&group);
        let report = PendingReport {
            schema_version: 1,
            report_id: report_id.clone(),
            created_at_ms: now_ms,
            file: display_file,
            canonical_file: canonical_file.clone(),
            capsule: capsule.clone(),
            summary,
            violations: group,
        };

        let pending_path = pending_dir.join(format!("{}.json", stable_key(&canonical_file)));
        let history_path = history_dir.join(format!("{report_id}.json"));

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
        .unwrap_or_else(|| common.config_dir.join(".code-guardrails-data/reports"));
    let pending_dir = report_dir.join("pending");

    if !pending_dir.is_dir() {
        return Ok(StopScanResult {
            clean: true,
            report_dir: report_dir.to_string_lossy().to_string(),
            pending_count: 0,
            summary: ScanSummary::default(),
            capsule: None,
            pending_reports: Vec::new(),
        });
    }

    let mut reports = Vec::new();
    let entries = fs::read_dir(&pending_dir)
        .map_err(|err| format!("failed to read {}: {err}", pending_dir.display()))?;

    for entry in entries {
        let path = entry
            .map_err(|err| format!("failed to read pending report entry: {err}"))?
            .path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let report: PendingReport = serde_json::from_str(&text)
            .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
        if !Path::new(&report.canonical_file).exists() {
            let _ = fs::remove_file(&path);
            continue;
        }
        reports.push(report);
    }

    let violations: Vec<Violation> = reports
        .iter()
        .flat_map(|report| report.violations.clone())
        .collect();
    let capsule = if violations.is_empty() {
        None
    } else {
        Some(build_capsule(&violations))
    };
    let summary = build_summary(&violations, reports.len());

    Ok(StopScanResult {
        clean: reports.is_empty(),
        report_dir: report_dir.to_string_lossy().to_string(),
        pending_count: reports.len(),
        summary,
        capsule,
        pending_reports: reports
            .into_iter()
            .map(|report| PendingReportSummary {
                report_id: report.report_id,
                file: report.file,
                canonical_file: report.canonical_file,
                capsule: report.capsule,
                summary: report.summary,
            })
            .collect(),
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

fn build_post_tooluse_hook_response(result: &ScanResult) -> serde_json::Value {
    let report_path = result
        .report_paths
        .first()
        .cloned()
        .unwrap_or_else(|| "<pending-report>".to_string());
    let classes = result
        .summary
        .classes
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");

    json!({
        "decision": "block",
        "reason": format!("Guardrail: {} violation(s) remain ({classes}). Repair at the owner layer; do not rename or rewrite into an equivalent fallback.", result.summary.violation_count),
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
        "reason": format!("Guardrail: {} unresolved pending report(s) remain. Fix them before stopping. First pending file: {}.", result.pending_count, first_report),
        "systemMessage": format!("Unresolved guardrail reports remain under {}/pending", result.report_dir)
    })
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

fn stable_key(input: &str) -> String {
    let mut hash: u64 = 14695981039346656037;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{hash:016x}")
}

fn timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
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

struct HookContext {
    scan_root: PathBuf,
    file_path: PathBuf,
}

fn extract_hook_context(input: &str) -> Result<HookContext, String> {
    let v: serde_json::Value =
        serde_json::from_str(input).map_err(|e| format!("failed to parse stdin JSON: {e}"))?;

    let scan_root = v["cwd"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let file_path = v["tool_input"]["file_path"]
        .as_str()
        .or_else(|| v["tool_input"]["filePath"].as_str())
        .or_else(|| v["tool_input"]["path"].as_str())
        .or_else(|| v["tool_response"]["filePath"].as_str())
        .or_else(|| v["tool_response"]["file_path"].as_str())
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

        PolicySet {
            ownership,
            defaults,
            adapters,
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
        let violations = vec![Violation {
            file: "src/api/handler.py".to_string(),
            canonical_file: "/repo/src/api/handler.py".to_string(),
            line: 3,
            rule_id: "py-no-fallback-get-default".to_string(),
            policy_group: "fallback".to_string(),
            violation_class: "fallback_unowned_default".to_string(),
            owner_guess: "boundary".to_string(),
            owner_hint: "boundary".to_string(),
            message: "bad".to_string(),
            code: "x = y.get(\"k\", 1)".to_string(),
            legal_remedies: vec!["boundary_parser".to_string(), "typed_exception".to_string()],
            forbidden_moves: vec!["rename".to_string(), "equivalent_rewrite".to_string()],
            approval_status: "missing".to_string(),
            approval_id: None,
            approval_reason: None,
        }];
        let capsule = build_capsule(&violations);
        assert!(capsule.contains("fallback_unowned_default"));
        assert!(capsule.contains("boundary_parser"));
        assert!(capsule.contains("rename"));
    }

    #[test]
    fn stable_key_is_deterministic() {
        assert_eq!(stable_key("abc"), stable_key("abc"));
        assert_ne!(stable_key("abc"), stable_key("abcd"));
    }
}
