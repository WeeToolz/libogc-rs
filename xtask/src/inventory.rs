use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use syn::visit::{self, Visit};

use crate::workspace::{CrateRole, load_roles, read_csv};

const DISALLOWED_CLASSES: &[&str] = &[
    "stub",
    "synthetic",
    "host-model",
    "fake-success",
    "state-only",
    "unimplemented",
    "unverified",
];

const ALLOWED_CLASSES: &[&str] = &[
    "dolphin-partial",
    "hardware",
    "ios",
    "scheduler",
    "protocol",
    "pure",
    "compatibility-forwarder",
];

#[derive(Clone, Debug)]
struct Override {
    crate_name: String,
    path: String,
    symbol: String,
    classification: String,
    probe: String,
    limitations: String,
}

#[derive(Clone, Debug)]
struct Evidence {
    crate_name: String,
    path: String,
    symbol: String,
    line: usize,
    classification: String,
    kind: String,
    evidence_path: String,
    function: String,
    api_path: Option<String>,
    scope: Option<EvidenceScope>,
    contracts: Vec<String>,
}

#[derive(Clone, Debug)]
struct ProbeCoverage {
    source: String,
    dolphin_runner: Option<String>,
}

#[derive(Clone, Debug)]
struct IpcEvidenceContract {
    crate_name: String,
    symbol: String,
    dolphin_runner: String,
    evidence_path: String,
    function: String,
    api_path: String,
    status: String,
    limitation: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvidenceScope {
    MockDispatch,
    ClosedService,
    DisabledAdapter,
    InvalidChannel,
    InvalidArguments,
    EmptyChannel,
    NoController,
    NoControllerAndInvalidArguments,
    ActiveSearchState,
    ZeroLength,
    EmptyQueue,
    NoPendingInterrupt,
    RepeatedInitialization,
    NoEventHook,
    SynchronousScalarOutput,
    ScalarOutput,
    ScalarAndBuffer,
    DolphinCacheRegister,
    DolphinCacheInstructions,
    ZeroTagCount,
    DolphinHostCallback,
    DolphinMouseLifecycle,
    IpcPartialDolphin,
    IpcRebootGuard,
    IosLifecycleDolphin,
}

impl EvidenceScope {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "mock-dispatch" => Self::MockDispatch,
            "closed-service" => Self::ClosedService,
            "disabled-adapter" => Self::DisabledAdapter,
            "invalid-channel" => Self::InvalidChannel,
            "invalid-arguments" => Self::InvalidArguments,
            "empty-channel" => Self::EmptyChannel,
            "no-controller" => Self::NoController,
            "no-controller-and-invalid-arguments" => Self::NoControllerAndInvalidArguments,
            "active-search-state" => Self::ActiveSearchState,
            "zero-length" => Self::ZeroLength,
            "empty-queue" => Self::EmptyQueue,
            "no-pending-interrupt" => Self::NoPendingInterrupt,
            "repeated-initialization" => Self::RepeatedInitialization,
            "no-event-hook" => Self::NoEventHook,
            "synchronous-scalar-output" => Self::SynchronousScalarOutput,
            "scalar-output" => Self::ScalarOutput,
            "scalar-and-buffer" => Self::ScalarAndBuffer,
            "dolphin-cache-register" => Self::DolphinCacheRegister,
            "dolphin-cache-instructions" => Self::DolphinCacheInstructions,
            "zero-tag-count" => Self::ZeroTagCount,
            "dolphin-host-callback" => Self::DolphinHostCallback,
            "dolphin-mouse-lifecycle" => Self::DolphinMouseLifecycle,
            "ipc-partial-dolphin" => Self::IpcPartialDolphin,
            "ipc-reboot-guard" => Self::IpcRebootGuard,
            "ios-lifecycle-dolphin" => Self::IosLifecycleDolphin,
            _ => return None,
        })
    }

    fn limitation(self) -> &'static str {
        match self {
            Self::MockDispatch => {
                "Mock dispatch only in Dolphin. Production driver behavior is unverified."
            }
            Self::ClosedService => {
                "Closed service guard only in Dolphin. Successful IOS requests and their effects are unverified."
            }
            Self::DisabledAdapter => {
                "Disabled adapter behavior only in Dolphin. Active discovery and pairing are unverified."
            }
            Self::InvalidChannel => {
                "Invalid channel behavior only in Dolphin. Valid channel behavior is unverified."
            }
            Self::InvalidArguments => {
                "Invalid argument behavior only in Dolphin. Valid operation with a controller remains unverified."
            }
            Self::EmptyChannel => {
                "Valid empty-channel behavior only in Dolphin. Effects with a connected controller remain unverified."
            }
            Self::NoController => {
                "Valid empty-channel and invalid-channel behavior only in Dolphin. Behavior with a connected controller remains unverified."
            }
            Self::NoControllerAndInvalidArguments => {
                "Valid empty-channel behavior and Rust invalid-argument guards only in Dolphin. Behavior with a connected controller remains unverified."
            }
            Self::ActiveSearchState => {
                "Inquiry state changes and adapter activity only in Dolphin. A usable controller channel, discovery results, and pairing remain unverified."
            }
            Self::ZeroLength => {
                "Zero-length behavior only in Dolphin. Non-zero transfer behavior remains unverified."
            }
            Self::EmptyQueue => {
                "Empty-queue behavior only in Dolphin. Queue operations with pending work remain unverified."
            }
            Self::NoPendingInterrupt => {
                "No-pending-interrupt behavior only in Dolphin. Dispatch of a pending hardware interrupt remains unverified."
            }
            Self::RepeatedInitialization => {
                "Repeated initialization only in Dolphin. First-call initialization remains unverified."
            }
            Self::NoEventHook => {
                "Close behavior without a registered event hook only in Dolphin. Event-hook release remains unverified."
            }
            Self::SynchronousScalarOutput => {
                "Synchronous scalar output only in Dolphin. Other formats and asynchronous requests remain unverified."
            }
            Self::ScalarOutput => {
                "Synchronous and asynchronous scalar output in Dolphin. Other formats remain unverified."
            }
            Self::ScalarAndBuffer => {
                "Selected scalar, string, and buffer cases are documented by separate contracts. Other format combinations and physical cache behavior remain unverified."
            }
            Self::IpcPartialDolphin => {
                "Selected ioctl and formatter cases pass in Dolphin. Physical Wii cache behavior remains unverified."
            }
            Self::IpcRebootGuard => {
                "Invalid argument guards and the OHBC test title handoff pass in Dolphin. The System Menu handoff remains unverified."
            }
            Self::IosLifecycleDolphin => {
                "IOS lifecycle API checks pass in Dolphin. Physical Wii reload behavior remains unverified; the startup loader contract is a no-op in pinned C."
            }
            Self::DolphinCacheRegister => {
                "Dolphin HID0 transitions and restoration only. Physical cache state and timing are unverified."
            }
            Self::DolphinCacheInstructions => {
                "Dolphin instruction and register checks only. Physical cache contents and timing are unverified."
            }
            Self::ZeroTagCount => {
                "Zero-tag behavior only in Dolphin. Allocation for non-zero tag counts remains unverified."
            }
            Self::DolphinHostCallback => {
                "Synchronous host callback behavior only in Dolphin. Physical sync button behavior is unverified."
            }
            Self::DolphinMouseLifecycle => {
                "Successful worker shutdown in Dolphin without a physical USB mouse. Stop errors are covered by injected host tests."
            }
        }
    }

    fn classification(self, declared: &str) -> &str {
        match self {
            Self::MockDispatch
            | Self::ClosedService
            | Self::DisabledAdapter
            | Self::InvalidChannel
            | Self::InvalidArguments
            | Self::EmptyChannel
            | Self::NoController
            | Self::ActiveSearchState
            | Self::ZeroTagCount
            | Self::ZeroLength
            | Self::EmptyQueue
            | Self::NoPendingInterrupt => "unverified",
            Self::RepeatedInitialization
            | Self::NoEventHook
            | Self::SynchronousScalarOutput
            | Self::ScalarOutput => "unverified",
            Self::ScalarAndBuffer
            | Self::IpcPartialDolphin
            | Self::IpcRebootGuard
            | Self::IosLifecycleDolphin => "dolphin-partial",
            Self::NoControllerAndInvalidArguments => "unverified",
            _ => declared,
        }
    }
}

#[derive(Clone, Debug)]
struct ApiEntry {
    crate_name: String,
    symbol: String,
    interface_kind: String,
    implementation_file: String,
    line: usize,
    classification: String,
    declared_classification: String,
    evidence: String,
    probe: String,
    limitations: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceFinding {
    classification: Option<&'static str>,
    reason: String,
}

pub fn generate_inventory(root: &Path) -> Result<(), String> {
    RuntimeInventory::collect(root)?.write(root)
}

pub fn check_inventory(root: &Path) -> Result<(), String> {
    RuntimeInventory::collect(root)?.check(root)
}

pub struct RuntimeInventory(Vec<ApiEntry>);

impl RuntimeInventory {
    pub fn collect(root: &Path) -> Result<Self, String> {
        collect_inventory(root).map(Self)
    }

    pub fn write(&self, root: &Path) -> Result<(), String> {
        let entries = &self.0;
        let csv = render_csv(entries);
        let csv_path = root.join("docs/generated/implementation-inventory.csv");
        if let Some(parent) = csv_path.parent() {
            fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
        }
        fs::write(&csv_path, csv).map_err(|err| format!("{}: {err}", csv_path.display()))?;

        let status = render_status(entries);
        let status_path = root.join("docs/generated/implementation-status.md");
        fs::write(&status_path, status)
            .map_err(|err| format!("{}: {err}", status_path.display()))?;

        let formatter_status = render_formatter_status(&load_ipc_format_contracts(root)?);
        let formatter_status_path = root.join("docs/generated/ipc-format-status.md");
        fs::write(&formatter_status_path, formatter_status)
            .map_err(|err| format!("{}: {err}", formatter_status_path.display()))?;

        println!(
            "wrote {} API entries to {}",
            entries.len(),
            csv_path.display()
        );
        Ok(())
    }

    pub fn check(&self, root: &Path) -> Result<(), String> {
        let entries = &self.0;
        let expected = render_csv(entries);
        let path = root.join("docs/generated/implementation-inventory.csv");
        let actual = fs::read_to_string(&path)
            .map_err(|err| format!("{}: {err}; run cargo xtask inventory", path.display()))?;
        if actual != expected {
            return Err(format!(
                "{} is stale; run cargo xtask inventory",
                path.display()
            ));
        }

        let expected_status = render_status(entries);
        let status_path = root.join("docs/generated/implementation-status.md");
        let actual_status = fs::read_to_string(&status_path).map_err(|err| {
            format!(
                "{}: {err}; run cargo xtask inventory",
                status_path.display()
            )
        })?;
        if actual_status != expected_status {
            return Err(format!(
                "{} is stale; run cargo xtask inventory",
                status_path.display()
            ));
        }

        let expected_formatter_status = render_formatter_status(&load_ipc_format_contracts(root)?);
        let formatter_status_path = root.join("docs/generated/ipc-format-status.md");
        let actual_formatter_status =
            fs::read_to_string(&formatter_status_path).map_err(|err| {
                format!(
                    "{}: {err}; run cargo xtask inventory",
                    formatter_status_path.display()
                )
            })?;
        if actual_formatter_status != expected_formatter_status {
            return Err(format!(
                "{} is stale; run cargo xtask inventory",
                formatter_status_path.display()
            ));
        }

        println!(
            "implementation inventory is current ({} entries)",
            entries.len()
        );
        Ok(())
    }

    pub fn audit(&self, root: &Path, json_path: Option<&Path>) -> Result<(), String> {
        let probe_sources = load_probe_coverage(root)?;
        let probes = probe_sources
            .iter()
            .map(|(name, source)| {
                let reason = (!root.join(&source.source).is_file())
                    .then(|| format!("focused probe source does not exist: {source:?}"));
                (name.as_str(), reason)
            })
            .collect();
        let report = completeness_report(&self.0, &probes);
        if let Some(path) = json_path {
            let json = serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?;
            fs::write(path, format!("{json}\n"))
                .map_err(|err| format!("{}: {err}", path.display()))?;
        }
        if report.entries.is_empty() {
            println!("runtime inventory contains no disallowed implementation classes");
            return Ok(());
        }
        for entry in &report.entries {
            for reason in &entry.reasons {
                eprintln!(
                    "{}::{} [{}] {}:{} — {}",
                    entry.crate_name,
                    entry.symbol,
                    entry.classification,
                    entry.implementation_file,
                    entry.line,
                    reason.message
                );
            }
        }
        Err(format!(
            "runtime completeness audit found {} disallowed API implementation(s)",
            report.remaining_count
        ))
    }
}

#[derive(Serialize)]
struct CompletenessReport<'a> {
    schema_version: u32,
    inventory_count: usize,
    unverified_count: usize,
    remaining_count: usize,
    entries: Vec<RemainingEntry<'a>>,
}

#[derive(Serialize)]
struct RemainingEntry<'a> {
    #[serde(rename = "crate")]
    crate_name: &'a str,
    symbol: &'a str,
    interface_kind: &'a str,
    implementation_file: &'a str,
    line: usize,
    classification: &'a str,
    probe: &'a str,
    reasons: Vec<AuditReason>,
}

#[derive(Serialize)]
struct AuditReason {
    code: &'static str,
    message: String,
}

fn completeness_report<'a>(
    inventory: &'a [ApiEntry],
    probes: &BTreeMap<&str, Option<String>>,
) -> CompletenessReport<'a> {
    let mut entries = Vec::new();
    for entry in inventory {
        let mut reasons = Vec::new();
        let probe = entry.probe.as_str();
        if probe.trim().is_empty() || probe == "none" {
            reasons.push(AuditReason {
                code: "missing_probe",
                message: "missing focused probe".to_string(),
            });
        } else {
            match probes.get(probe) {
                Some(None) => {}
                Some(Some(message)) => reasons.push(AuditReason {
                    code: "missing_probe_source",
                    message: message.clone(),
                }),
                None => reasons.push(AuditReason {
                    code: "unmapped_probe",
                    message: format!("probe `{probe}` is not mapped in config/probe-coverage.csv"),
                }),
            }
        }
        if DISALLOWED_CLASSES.contains(&entry.classification.as_str()) {
            reasons.push(AuditReason {
                code: "disallowed_classification",
                message: entry.limitations.clone(),
            });
        }
        if !reasons.is_empty() {
            entries.push(RemainingEntry {
                crate_name: &entry.crate_name,
                symbol: &entry.symbol,
                interface_kind: &entry.interface_kind,
                implementation_file: &entry.implementation_file,
                line: entry.line,
                classification: &entry.classification,
                probe,
                reasons,
            });
        }
    }

    CompletenessReport {
        schema_version: 1,
        inventory_count: inventory.len(),
        unverified_count: inventory
            .iter()
            .filter(|entry| entry.classification == "unverified")
            .count(),
        remaining_count: entries.len(),
        entries,
    }
}

fn load_probe_coverage(root: &Path) -> Result<BTreeMap<String, ProbeCoverage>, String> {
    let path = root.join("config/probe-coverage.csv");
    let rows = crate::workspace::read_csv(&path)?;
    let mut coverage = BTreeMap::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if row.len() != 3 || row[0].trim().is_empty() || row[1].trim().is_empty() {
            return Err(format!(
                "{}:{}: expected probe and source columns",
                path.display(),
                line_no + 1
            ));
        }
        let runner = (!row[2].trim().is_empty()).then(|| row[2].clone());
        if coverage
            .insert(
                row[0].clone(),
                ProbeCoverage {
                    source: row[1].clone(),
                    dolphin_runner: runner,
                },
            )
            .is_some()
        {
            return Err(format!(
                "{}:{}: duplicate probe `{}`",
                path.display(),
                line_no + 1,
                row[0]
            ));
        }
    }
    Ok(coverage)
}

fn collect_inventory(root: &Path) -> Result<Vec<ApiEntry>, String> {
    validate_formatter_assembly_exports(root)?;
    let roles = load_roles(root)?;
    let overrides = load_overrides(root)?;
    let evidence = load_evidence(root)?;
    let coverage = load_probe_coverage(root)?;
    let contracts = load_ipc_format_contracts(root)?;
    let mut used_evidence = vec![false; evidence.len()];
    let evidence_index = evidence
        .iter()
        .enumerate()
        .map(|(index, item)| (evidence_key(item), index))
        .collect::<HashMap<_, _>>();
    let mut parsed_evidence = ParsedEvidenceCache::default();
    let mut entries = Vec::new();

    for role in roles.iter().filter(|role| role.runtime) {
        let src = root.join(&role.path).join("src");
        if !src.exists() {
            continue;
        }
        let mut files = Vec::new();
        collect_rust_files(&src, &mut files)?;
        files.sort();
        for file in files {
            let relative_to_crate = file
                .strip_prefix(root.join(&role.path))
                .map_err(|err| err.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let implementation_file = file
                .strip_prefix(root)
                .map_err(|err| err.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            for function in scan_public_functions(&file)? {
                let selected = select_override(
                    &overrides,
                    &role.package,
                    &relative_to_crate,
                    &function.name,
                );
                let (mut declared_classification, probe, mut limitations) = match selected {
                    Some(value) => (
                        value.classification.clone(),
                        value.probe.clone(),
                        value.limitations.clone(),
                    ),
                    None => (
                        role.default_classification.clone(),
                        role.probe.clone(),
                        default_limitations(role),
                    ),
                };
                let mut classification = declared_classification.clone();
                let mut evidence_summary = String::from("none");
                let source_review = function
                    .source_finding
                    .as_ref()
                    .filter(|finding| finding.classification.is_none())
                    .map(|finding| finding.reason.as_str());
                if let Some(finding) = function
                    .source_finding
                    .as_ref()
                    .filter(|finding| finding.classification.is_some())
                {
                    classification = finding.classification.expect("filtered above").to_string();
                    declared_classification = classification.clone();
                    evidence_summary = String::from("source-derived");
                    limitations = format!("source-derived: {}", finding.reason);
                } else {
                    if !ALLOWED_CLASSES.contains(&declared_classification.as_str()) {
                        return Err(format!(
                            "{}::{} has unknown implementation classification `{}`",
                            role.package, function.name, declared_classification
                        ));
                    }
                    let key = (
                        role.package.clone(),
                        relative_to_crate.clone(),
                        function.name.clone(),
                        function.line,
                    );
                    if let Some(&index) = evidence_index.get(&key) {
                        let item = &evidence[index];
                        validate_evidence_classification(item, &declared_classification)?;
                        validate_contracts(item, &probe, &coverage, &contracts)?;
                        let cached =
                            parsed_evidence.get_or_parse(&root.join(&item.evidence_path))?;
                        verify_evidence_cached(item, &declared_classification, cached)?;
                        used_evidence[index] = true;
                        evidence_summary = match &item.api_path {
                            Some(api_path) => format!(
                                "{}:{}::{} calls {} through `{api_path}`",
                                item.kind, item.evidence_path, item.function, item.symbol
                            ),
                            None => format!(
                                "{}:{}::{} calls {}",
                                item.kind, item.evidence_path, item.function, item.symbol
                            ),
                        };
                        if let Some(scope) = item.scope {
                            classification =
                                scope.classification(&declared_classification).to_string();
                            limitations =
                                format!("{limitations}; evidence scope: {}", scope.limitation());
                        }
                        if let Some(review) = source_review {
                            limitations = format!("{limitations}; source review: {review}");
                        }
                    } else {
                        classification = String::from("unverified");
                        limitations = match source_review {
                            Some(review) => format!(
                                "source review: {review}; declared `{declared_classification}` without exact API test or probe evidence"
                            ),
                            None => format!(
                                "declared `{declared_classification}` without exact API test or probe evidence"
                            ),
                        };
                    }
                }
                entries.push(ApiEntry {
                    crate_name: role.package.clone(),
                    symbol: function.name,
                    interface_kind: function.kind,
                    implementation_file: implementation_file.clone(),
                    line: function.line,
                    classification,
                    declared_classification,
                    evidence: evidence_summary,
                    probe,
                    limitations,
                });
            }
        }
    }

    for export in load_compatibility_exports(root)? {
        let role_path = role_path_for_package(root, &export.crate_name)?;
        let source_path = Path::new(&export.implementation_file);
        let relative_path = source_path
            .strip_prefix(&role_path)
            .map_err(|err| format!("{}: {err}", source_path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        let key = (
            export.crate_name.clone(),
            relative_path,
            export.symbol.clone(),
            export.line,
        );
        if let Some(&index) = evidence_index.get(&key) {
            let item = &evidence[index];
            validate_evidence_classification(item, &export.declared_classification)?;
            validate_contracts(item, &export.probe, &coverage, &contracts)?;
            let cached = parsed_evidence.get_or_parse(&root.join(&item.evidence_path))?;
            verify_evidence_cached(item, &export.declared_classification, cached)?;
            used_evidence[index] = true;
            let mut export = export;
            export.evidence = match &item.api_path {
                Some(api_path) => format!(
                    "{}:{}::{} calls {} through `{api_path}`",
                    item.kind, item.evidence_path, item.function, item.symbol
                ),
                None => format!(
                    "{}:{}::{} calls {}",
                    item.kind, item.evidence_path, item.function, item.symbol
                ),
            };
            if let Some(scope) = item.scope {
                export.classification = scope
                    .classification(&export.declared_classification)
                    .to_string();
                export.limitations = format!(
                    "{}; evidence scope: {}",
                    export.limitations,
                    scope.limitation()
                );
            }
            entries.push(export);
        } else {
            entries.push(export);
        }
    }

    for (index, item) in evidence.iter().enumerate() {
        if !used_evidence[index] {
            return Err(format!(
                "stale implementation evidence: {}:{} {}::{}",
                item.path, item.line, item.crate_name, item.symbol
            ));
        }
    }

    entries.sort_by(|left, right| {
        (
            &left.crate_name,
            &left.symbol,
            &left.implementation_file,
            left.line,
        )
            .cmp(&(
                &right.crate_name,
                &right.symbol,
                &right.implementation_file,
                right.line,
            ))
    });
    entries.dedup_by(|left, right| {
        left.crate_name == right.crate_name
            && left.symbol == right.symbol
            && left.implementation_file == right.implementation_file
            && left.line == right.line
    });
    Ok(entries)
}

fn assembly_global_symbols(source: &str) -> HashSet<String> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let (_, symbol) = line
                .split_once(".globl")
                .or_else(|| line.split_once(".global"))?;
            let symbol = symbol.trim().trim_end_matches(',').trim();
            (!symbol.is_empty()).then(|| symbol.to_string())
        })
        .collect()
}

fn missing_assembly_exports(found: &HashSet<String>, listed: &HashSet<String>) -> Vec<String> {
    let mut missing = found
        .iter()
        .filter(|symbol| symbol.starts_with("IOS_IoctlvFormat") && !listed.contains(*symbol))
        .cloned()
        .collect::<Vec<_>>();
    missing.sort();
    missing
}

fn validate_formatter_assembly_exports(root: &Path) -> Result<(), String> {
    let source_path = "crates/core/ogc-ipc/src/lib.rs";
    let source = fs::read_to_string(root.join(source_path)).map_err(|error| error.to_string())?;
    let found = assembly_global_symbols(&source);
    let rows = read_csv(&root.join("config/compatibility-exports.csv"))?;
    let listed = rows
        .iter()
        .skip(1)
        .filter(|row| row.len() == 8 && row[0] == "ogc-ipc" && row[1] == source_path)
        .map(|row| row[2].clone())
        .collect::<HashSet<_>>();
    let missing = missing_assembly_exports(&found, &listed);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{}: formatter assembly exports are missing from config/compatibility-exports.csv: {}",
            source_path,
            missing.join(", ")
        ))
    }
}

fn role_path_for_package(root: &Path, package: &str) -> Result<PathBuf, String> {
    load_roles(root)?
        .into_iter()
        .find(|role| role.package == package)
        .map(|role| role.path)
        .ok_or_else(|| format!("compatibility export uses unknown crate `{package}`"))
}

fn load_compatibility_exports(root: &Path) -> Result<Vec<ApiEntry>, String> {
    let path = root.join("config/compatibility-exports.csv");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let rows = read_csv(&path)?;
    let mut exports = Vec::new();
    let mut keys = HashSet::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if row.len() != 8 || row.iter().any(|field| field.trim().is_empty()) {
            return Err(format!(
                "{}:{}: expected crate, source, symbol, line, classification, kind, probe, and limitation",
                path.display(),
                line_no + 1
            ));
        }
        let line = row[3].parse::<usize>().map_err(|_| {
            format!(
                "{}:{}: line must be an integer",
                path.display(),
                line_no + 1
            )
        })?;
        let source = root.join(&row[1]);
        if line == 0 || !source.is_file() {
            return Err(format!(
                "{}:{}: source or line is invalid",
                path.display(),
                line_no + 1
            ));
        }
        if !keys.insert((row[0].clone(), row[1].clone(), row[2].clone(), line)) {
            return Err(format!(
                "{}:{}: duplicate compatibility export {}",
                path.display(),
                line_no + 1,
                row[2]
            ));
        }
        exports.push(ApiEntry {
            crate_name: row[0].clone(),
            symbol: row[2].clone(),
            interface_kind: row[5].clone(),
            implementation_file: row[1].clone(),
            line,
            classification: String::from("unverified"),
            declared_classification: row[4].clone(),
            evidence: String::from("none"),
            probe: row[6].clone(),
            limitations: row[7].clone(),
        });
    }
    Ok(exports)
}

fn load_evidence(root: &Path) -> Result<Vec<Evidence>, String> {
    let path = root.join("config/implementation-evidence.csv");
    let rows = read_csv(&path)?;
    let known_contracts = load_ipc_format_contracts(root)?;
    let mut evidence = Vec::new();
    let mut evidence_keys = HashSet::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if !(8..=11).contains(&row.len()) || row[..8].iter().any(|field| field.trim().is_empty()) {
            return Err(format!(
                "{}:{}: expected eight required evidence columns and optional API path, scope, and contracts",
                path.display(),
                line_no + 1
            ));
        }
        let item = Evidence {
            crate_name: row[0].clone(),
            path: row[1].clone(),
            symbol: row[2].clone(),
            line: row[3].parse().map_err(|_| {
                format!(
                    "{}:{}: evidence line must be an integer",
                    path.display(),
                    line_no + 1
                )
            })?,
            classification: row[4].clone(),
            kind: row[5].clone(),
            evidence_path: row[6].clone(),
            function: row[7].clone(),
            api_path: row.get(8).filter(|value| !value.trim().is_empty()).cloned(),
            scope: row
                .get(9)
                .filter(|value| !value.is_empty())
                .map(|value| {
                    EvidenceScope::parse(value).ok_or_else(|| {
                        format!(
                            "{}:{}: unknown evidence scope `{value}`",
                            path.display(),
                            line_no + 1
                        )
                    })
                })
                .transpose()?,
            contracts: row
                .get(10)
                .filter(|value| !value.trim().is_empty())
                .map(|value| value.split(';').map(str::to_string).collect())
                .unwrap_or_default(),
        };
        for id in &item.contracts {
            if !known_contracts.contains_key(id) {
                return Err(format!(
                    "{}:{}: unknown evidence contract `{id}`",
                    path.display(),
                    line_no + 1
                ));
            }
        }
        if !evidence_keys.insert(evidence_key(&item)) {
            return Err(format!(
                "{}:{}: duplicate evidence for {}::{}",
                path.display(),
                line_no + 1,
                item.crate_name,
                item.symbol
            ));
        }
        evidence.push(item);
    }
    Ok(evidence)
}

fn load_ipc_format_contracts(
    root: &Path,
) -> Result<HashMap<String, Vec<IpcEvidenceContract>>, String> {
    let path = root.join("config/ipc-format-contracts.csv");
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let rows = read_csv(&path)?;
    let expected_header = [
        "contract_id",
        "crate",
        "symbol",
        "probe",
        "evidence_path",
        "function",
        "api_path",
        "check_label",
        "environment",
        "scope",
        "limitation",
        "status",
    ];
    if rows
        .first()
        .map(|row| row.iter().map(String::as_str).collect::<Vec<_>>())
        != Some(expected_header.to_vec())
    {
        return Err(format!(
            "{}: invalid IPC format contract header",
            path.display()
        ));
    }
    let mut contracts = HashMap::<String, Vec<IpcEvidenceContract>>::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.len() != expected_header.len() || row.iter().any(|value| value.trim().is_empty()) {
            return Err(format!(
                "{}:{}: incomplete IPC format contract",
                path.display(),
                line_no + 1
            ));
        }
        if !["verified", "partial", "blocked", "unverified"].contains(&row[11].as_str()) {
            return Err(format!(
                "{}:{}: invalid formatter evidence status `{}`",
                path.display(),
                line_no + 1,
                row[11]
            ));
        }
        contracts
            .entry(row[0].clone())
            .or_default()
            .push(IpcEvidenceContract {
                crate_name: row[1].clone(),
                symbol: row[2].clone(),
                dolphin_runner: row[3].clone(),
                evidence_path: row[4].clone(),
                function: row[5].clone(),
                api_path: row[6].clone(),
                status: row[11].clone(),
                limitation: row[10].clone(),
            });
    }
    Ok(contracts)
}

fn validate_contracts(
    item: &Evidence,
    coverage_id: &str,
    coverage: &BTreeMap<String, ProbeCoverage>,
    contracts: &HashMap<String, Vec<IpcEvidenceContract>>,
) -> Result<(), String> {
    if item.contracts.is_empty() {
        return Ok(());
    }
    let mapping = coverage
        .get(coverage_id)
        .ok_or_else(|| format!("missing coverage mapping `{coverage_id}`"))?;
    let runner = mapping
        .dolphin_runner
        .as_deref()
        .ok_or_else(|| format!("missing Dolphin runner for `{coverage_id}`"))?;
    if !crate::dolphin::is_known_probe(runner) {
        return Err(format!("unknown Dolphin runner `{runner}`"));
    }
    for id in &item.contracts {
        let rows = contracts
            .get(id)
            .ok_or_else(|| format!("unknown contract `{id}`"))?;
        if rows.is_empty()
            || !rows.iter().all(|c| {
                c.crate_name == item.crate_name
                    && c.symbol == item.symbol
                    && c.dolphin_runner == runner
                    && c.evidence_path == mapping.source
                    && c.evidence_path == item.evidence_path
                    && c.function == item.function
                    && item.api_path.as_deref() == Some(c.api_path.as_str())
            })
        {
            return Err(format!(
                "contract `{id}` does not match coverage mapping and API evidence"
            ));
        }
    }
    Ok(())
}

type EvidenceKey = (String, String, String, usize);

fn evidence_key(evidence: &Evidence) -> EvidenceKey {
    (
        evidence.crate_name.clone(),
        evidence.path.clone(),
        evidence.symbol.clone(),
        evidence.line,
    )
}

#[derive(Default)]
struct ParsedEvidenceCache {
    files: HashMap<PathBuf, ParsedEvidence>,
}

struct ParsedEvidence {
    source: String,
    syntax: syn::File,
}

impl ParsedEvidenceCache {
    fn get_or_parse(&mut self, path: &Path) -> Result<&ParsedEvidence, String> {
        if !self.files.contains_key(path) {
            let source =
                fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
            let syntax = syn::parse_file(&source).map_err(|err| {
                format!("evidence source {} is invalid Rust: {err}", path.display())
            })?;
            self.files
                .insert(path.to_path_buf(), ParsedEvidence { source, syntax });
        }
        Ok(self.files.get(path).expect("entry was inserted above"))
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.files.len()
    }
}

#[cfg(test)]
fn verify_evidence(root: &Path, evidence: &Evidence, declared: &str) -> Result<(), String> {
    validate_evidence_classification(evidence, declared)?;
    let path = root.join(&evidence.evidence_path);
    let source = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let syntax = syn::parse_file(&source).map_err(|err| {
        format!(
            "{}::{} evidence source {} is invalid Rust: {err}",
            evidence.crate_name,
            evidence.symbol,
            path.display()
        )
    })?;
    verify_evidence_cached(evidence, declared, &ParsedEvidence { source, syntax })
}

fn verify_evidence_cached(
    evidence: &Evidence,
    declared: &str,
    cached: &ParsedEvidence,
) -> Result<(), String> {
    validate_evidence_classification(evidence, declared)?;
    let normalized_path = evidence.evidence_path.replace('\\', "/");
    match evidence.kind.as_str() {
        "hardware-probe" if !normalized_path.starts_with("probes/") => {
            return Err(format!(
                "{}::{} hardware evidence must be under probes/",
                evidence.crate_name, evidence.symbol
            ));
        }
        "host-test" if !is_host_test_source(&normalized_path, &cached.source) => {
            return Err(format!(
                "{}::{} host evidence must be a test source",
                evidence.crate_name, evidence.symbol
            ));
        }
        _ => {}
    }
    verify_evidence_file(&cached.syntax, evidence, Path::new(&evidence.evidence_path))
}

fn validate_evidence_classification(evidence: &Evidence, declared: &str) -> Result<(), String> {
    if evidence.classification != declared {
        return Err(format!(
            "{}::{} evidence verifies `{}`, but the code is declared `{declared}`",
            evidence.crate_name, evidence.symbol, evidence.classification
        ));
    }
    let accepted_kind = match declared {
        "hardware" | "ios" | "scheduler" => evidence.kind == "hardware-probe",
        "protocol" | "pure" | "compatibility-forwarder" => {
            evidence.kind == "host-test" || evidence.kind == "hardware-probe"
        }
        _ => false,
    };
    if !accepted_kind {
        return Err(format!(
            "{}::{} classification `{declared}` cannot use `{}` evidence",
            evidence.crate_name, evidence.symbol, evidence.kind
        ));
    }
    Ok(())
}

fn is_host_test_source(path: &str, source: &str) -> bool {
    path.contains("/tests/") || source.contains("#[test]")
}

#[cfg(test)]
fn verify_evidence_call(source: &str, evidence: &Evidence, path: &Path) -> Result<(), String> {
    let file = syn::parse_file(source).map_err(|err| {
        format!(
            "{}::{} evidence source {} is invalid Rust: {err}",
            evidence.crate_name,
            evidence.symbol,
            path.display()
        )
    })?;
    verify_evidence_file(&file, evidence, path)
}

fn verify_evidence_file(file: &syn::File, evidence: &Evidence, path: &Path) -> Result<(), String> {
    if evidence.crate_name == "ogc-runtime"
        && matches!(evidence.path.as_str(), "src/driver.rs" | "src/scheduler.rs")
        && evidence.api_path.is_none()
    {
        return Err(format!(
            "{}::{} runtime evidence requires a qualified API path",
            evidence.crate_name, evidence.symbol
        ));
    }
    let mut visitor = EvidenceFunctionVisitor {
        function: &evidence.function,
        symbol: &evidence.symbol,
        api_path: evidence.api_path.as_deref(),
        require_test: evidence.kind == "host-test",
        function_count: 0,
        call_found: false,
        api_path_found: false,
    };
    visitor.visit_file(file);

    if visitor.function_count == 0 {
        return Err(format!(
            "{}::{} evidence function `{}` is missing{} in {}",
            evidence.crate_name,
            evidence.symbol,
            evidence.function,
            if visitor.require_test {
                " or is not a #[test]"
            } else {
                ""
            },
            path.display()
        ));
    }
    if visitor.function_count > 1 {
        return Err(format!(
            "{}::{} evidence function `{}` is ambiguous in {}; use a unique function name",
            evidence.crate_name,
            evidence.symbol,
            evidence.function,
            path.display()
        ));
    }
    if !visitor.call_found {
        return Err(format!(
            "{}::{} evidence function `{}` does not call the API in {}",
            evidence.crate_name,
            evidence.symbol,
            evidence.function,
            path.display()
        ));
    }
    if let Some(api_path) = &evidence.api_path
        && !visitor.api_path_found
    {
        return Err(format!(
            "{}::{} evidence function `{}` does not bind or call API path `{api_path}` in {}",
            evidence.crate_name,
            evidence.symbol,
            evidence.function,
            path.display()
        ));
    }
    Ok(())
}

struct EvidenceFunctionVisitor<'a> {
    function: &'a str,
    symbol: &'a str,
    api_path: Option<&'a str>,
    require_test: bool,
    function_count: usize,
    call_found: bool,
    api_path_found: bool,
}

impl<'ast> Visit<'ast> for EvidenceFunctionVisitor<'_> {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        let is_test = function
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("test"));
        if function.sig.ident == self.function && (!self.require_test || is_test) {
            self.function_count += 1;
            let mut calls = EvidenceCallVisitor {
                symbol: self.symbol,
                api_path: self.api_path,
                found: false,
                api_path_found: false,
            };
            calls.visit_block(&function.block);
            self.call_found |= calls.found;
            self.api_path_found |= calls.api_path_found;
        }
        visit::visit_item_fn(self, function);
    }
}

struct EvidenceCallVisitor<'a> {
    symbol: &'a str,
    api_path: Option<&'a str>,
    found: bool,
    api_path_found: bool,
}

impl EvidenceCallVisitor<'_> {
    fn path_calls_symbol(&self, expression: &syn::Expr) -> bool {
        match expression {
            syn::Expr::Path(path) => path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == self.symbol),
            syn::Expr::Paren(paren) => self.path_calls_symbol(&paren.expr),
            syn::Expr::Group(group) => self.path_calls_symbol(&group.expr),
            _ => false,
        }
    }
}

impl<'ast> Visit<'ast> for EvidenceCallVisitor<'_> {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(api_path) = self.api_path
            && local
                .init
                .as_ref()
                .and_then(|initializer| expression_path(&initializer.expr))
                .as_deref()
                == Some(api_path)
        {
            let mut bindings = PatternIdentifierVisitor {
                symbol: self.symbol,
                found: false,
            };
            bindings.visit_pat(&local.pat);
            self.api_path_found |= bindings.found;
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        if self
            .api_path
            .is_some_and(|expected| expression_path(&expression.func).as_deref() == Some(expected))
        {
            self.api_path_found = true;
        }
        self.found |= self.api_path.map_or_else(
            || self.path_calls_symbol(&expression.func),
            |api_path| {
                expression_path(&expression.func).as_deref() == Some(api_path)
                    || (self.api_path_found
                        && expression_path(&expression.func).as_deref() == Some(self.symbol))
            },
        );
        visit::visit_expr_call(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        if let Some(api_path) = self.api_path
            && format!(
                "{}.{}",
                expression_path(&expression.receiver).unwrap_or_default(),
                expression.method
            ) == api_path
        {
            self.api_path_found = true;
        }
        self.found |= self.api_path.map_or_else(
            || expression.method == self.symbol,
            |api_path| {
                expression.method == self.symbol
                    && format!(
                        "{}.{}",
                        expression_path(&expression.receiver).unwrap_or_default(),
                        expression.method
                    ) == api_path
            },
        );
        visit::visit_expr_method_call(self, expression);
    }

    fn visit_item_fn(&mut self, _function: &'ast syn::ItemFn) {
        // A nested function definition does not mean the evidence function calls it.
    }

    fn visit_macro(&mut self, macro_: &'ast syn::Macro) {
        let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        if let Ok(expressions) = syn::parse::Parser::parse2(parser, macro_.tokens.clone()) {
            for expression in expressions {
                self.visit_expr(&expression);
            }
        }
    }
}

struct PatternIdentifierVisitor<'a> {
    symbol: &'a str,
    found: bool,
}

impl<'ast> Visit<'ast> for PatternIdentifierVisitor<'_> {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.found |= pattern.ident == self.symbol;
        visit::visit_pat_ident(self, pattern);
    }
}

fn expression_path(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Path(path) => Some(
            path.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        ),
        syn::Expr::Field(field) => Some(format!(
            "{}.{}",
            expression_path(&field.base)?,
            match &field.member {
                syn::Member::Named(name) => name.to_string(),
                syn::Member::Unnamed(index) => index.index.to_string(),
            }
        )),
        syn::Expr::Index(index) => Some(format!(
            "{}[{}]",
            expression_path(&index.expr)?,
            match index.index.as_ref() {
                syn::Expr::Lit(literal) => match &literal.lit {
                    syn::Lit::Int(value) => value.base10_digits().to_string(),
                    _ => return None,
                },
                expression => expression_path(expression)?,
            }
        )),
        syn::Expr::Paren(paren) => expression_path(&paren.expr),
        syn::Expr::Group(group) => expression_path(&group.expr),
        _ => None,
    }
}

fn default_limitations(role: &CrateRole) -> String {
    format!(
        "See crate contract for {} role and hardware validation status",
        role.role
    )
}

fn load_overrides(root: &Path) -> Result<Vec<Override>, String> {
    let path = root.join("config/implementation-overrides.csv");
    let rows = read_csv(&path)?;
    let mut overrides = Vec::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if row.len() != 6 {
            return Err(format!(
                "{}:{}: expected 6 columns, found {}",
                path.display(),
                line_no + 1,
                row.len()
            ));
        }
        overrides.push(Override {
            crate_name: row[0].clone(),
            path: row[1].clone(),
            symbol: row[2].clone(),
            classification: row[3].clone(),
            probe: row[4].clone(),
            limitations: row[5].clone(),
        });
    }
    Ok(overrides)
}

fn select_override<'a>(
    overrides: &'a [Override],
    crate_name: &str,
    path: &str,
    symbol: &str,
) -> Option<&'a Override> {
    overrides
        .iter()
        .filter(|value| {
            value.crate_name == crate_name
                && (value.path == "*" || value.path == path)
                && (value.symbol == "*" || value.symbol == symbol)
        })
        .max_by_key(|value| {
            let path_score = usize::from(value.path != "*");
            let symbol_score = usize::from(value.symbol != "*");
            path_score + symbol_score * 2
        })
}

fn collect_rust_files(dir: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| err.to_string())?;
        if file_type.is_dir() {
            collect_rust_files(&path, output)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            output.push(path);
        }
    }
    Ok(())
}

#[derive(Debug)]
struct ScannedFunction {
    name: String,
    kind: String,
    line: usize,
    source_finding: Option<SourceFinding>,
}

fn scan_public_functions(path: &Path) -> Result<Vec<ScannedFunction>, String> {
    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    scan_public_functions_source(&source)
}

fn scan_public_functions_source(source: &str) -> Result<Vec<ScannedFunction>, String> {
    // The formatter hook module is compiled only into the Dolphin probe. Its
    // public functions are test controls, not production crate APIs.
    if source
        .lines()
        .any(|line| line.trim() == "#![cfg(feature = \"format-probe-hooks\")]")
    {
        return Ok(Vec::new());
    }

    let source_findings = analyze_function_bodies(source)
        .map_err(|err| format!("failed to parse Rust source: {err}"))?;
    let mut functions = Vec::new();
    let mut brace_depth: i32 = 0;
    let mut public_trait_depths = Vec::new();
    let mut excluded_scope_depths = Vec::new();
    let mut mask_state = RustMaskState::default();
    let mut pending_attributes = Vec::new();
    let mut lwp_macro_depth: Option<i32> = None;

    for (index, original) in source.lines().enumerate() {
        let clean = mask_rust_line(original, &mut mask_state);
        let trimmed = clean.trim();
        let line_no = index + 1;

        // Rust allows comments and blank lines between attributes and an item.
        if trimmed.is_empty() {
            continue;
        }

        while public_trait_depths
            .last()
            .is_some_and(|depth| brace_depth < *depth)
        {
            public_trait_depths.pop();
        }
        while excluded_scope_depths
            .last()
            .is_some_and(|depth| brace_depth < *depth)
        {
            excluded_scope_depths.pop();
        }
        if lwp_macro_depth.is_some_and(|depth| brace_depth < depth) {
            lwp_macro_depth = None;
        }

        if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            pending_attributes.push(original.trim().to_string());
            brace_depth += brace_delta(&clean);
            continue;
        }

        let excluded_from_target = pending_attributes.iter().any(|attribute| {
            attribute.contains("cfg(not(target_arch = \"powerpc\"))")
                || attribute.contains("cfg(test)")
                || attribute.contains("cfg(all(test")
        });

        let opens_public_trait = is_public_trait(trimmed);
        let opens_lwp_macro = trimmed.starts_with("lwp_s32_stub!");
        let inside_public_trait = public_trait_depths.last().is_some();
        let inside_lwp_macro = lwp_macro_depth.is_some();

        let inside_excluded_scope = excluded_scope_depths.last().is_some();

        if !inside_excluded_scope && !excluded_from_target {
            let public_item = is_public_function_line(trimmed);
            let trait_item = inside_public_trait && is_trait_function_line(trimmed);
            let macro_item = inside_lwp_macro && is_trait_function_line(trimmed);
            if (public_item || trait_item || macro_item)
                && let Some(name) = function_name(trimmed)
            {
                let kind = if trait_item {
                    "trait-method"
                } else if original.contains("extern \"C\"") {
                    "extern-c"
                } else if brace_depth > 0 {
                    "rust-method-or-module-function"
                } else {
                    "rust-function"
                };
                functions.push(ScannedFunction {
                    source_finding: source_findings.get(&(name.clone(), line_no)).cloned(),
                    name,
                    kind: kind.to_string(),
                    line: line_no,
                });
            }
        }

        let delta = brace_delta(&clean);
        if opens_public_trait && delta > 0 && !excluded_from_target {
            public_trait_depths.push(brace_depth + delta);
        }
        if excluded_from_target && delta > 0 {
            excluded_scope_depths.push(brace_depth + delta);
        }
        if opens_lwp_macro && delta > 0 && !excluded_from_target {
            lwp_macro_depth = Some(brace_depth + delta);
        }
        brace_depth += delta;
        pending_attributes.clear();
    }
    Ok(functions)
}

fn analyze_function_bodies(
    source: &str,
) -> Result<BTreeMap<(String, usize), SourceFinding>, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut analyzer = FunctionAnalyzer::default();
    analyzer.visit_file(&file);
    Ok(analyzer.findings)
}

#[derive(Default)]
struct FunctionAnalyzer {
    findings: BTreeMap<(String, usize), SourceFinding>,
    public_trait_depth: usize,
}

impl FunctionAnalyzer {
    fn inspect(&mut self, signature: &syn::Signature, block: &syn::Block) {
        let mut arguments = IdentifierCollector::default();
        for input in &signature.inputs {
            if let syn::FnArg::Typed(argument) = input {
                arguments.visit_pat(&argument.pat);
            }
        }

        let mut body = BodyEvidence::default();
        body.visit_block(block);
        let ignored: Vec<_> = arguments
            .identifiers
            .keys()
            .filter(|name| !name.starts_with('_') && !body.identifiers.contains_key(*name))
            .cloned()
            .collect();

        let finding = if body.discarded_results > 0 {
            Some(SourceFinding {
                classification: None,
                reason: format!(
                    "discards {} operation result(s) with a wildcard binding",
                    body.discarded_results
                ),
            })
        } else if !ignored.is_empty() {
            Some(SourceFinding {
                classification: Some("stub"),
                reason: format!("does not use argument(s): {}", ignored.join(", ")),
            })
        } else if block_returns_constant_success(block) {
            Some(SourceFinding {
                classification: Some("fake-success"),
                reason: "returns an unconditional success value".to_string(),
            })
        } else {
            None
        };

        if let Some(finding) = finding {
            self.findings.insert(
                (
                    signature.ident.to_string(),
                    signature.ident.span().start().line,
                ),
                finding,
            );
        }
    }
}

impl<'ast> Visit<'ast> for FunctionAnalyzer {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        if matches!(function.vis, syn::Visibility::Public(_)) {
            self.inspect(&function.sig, &function.block);
        }
        visit::visit_item_fn(self, function);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        if matches!(function.vis, syn::Visibility::Public(_)) {
            self.inspect(&function.sig, &function.block);
        }
        visit::visit_impl_item_fn(self, function);
    }

    fn visit_item_trait(&mut self, trait_: &'ast syn::ItemTrait) {
        if matches!(trait_.vis, syn::Visibility::Public(_)) {
            self.public_trait_depth += 1;
            visit::visit_item_trait(self, trait_);
            self.public_trait_depth -= 1;
        }
    }

    fn visit_trait_item_fn(&mut self, function: &'ast syn::TraitItemFn) {
        if self.public_trait_depth > 0
            && let Some(block) = &function.default
        {
            self.inspect(&function.sig, block);
        }
        visit::visit_trait_item_fn(self, function);
    }
}

#[derive(Default)]
struct IdentifierCollector {
    identifiers: BTreeMap<String, ()>,
}

impl<'ast> Visit<'ast> for IdentifierCollector {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.identifiers.insert(pattern.ident.to_string(), ());
        visit::visit_pat_ident(self, pattern);
    }
}

#[derive(Default)]
struct BodyEvidence {
    identifiers: BTreeMap<String, ()>,
    discarded_results: usize,
}

impl<'ast> Visit<'ast> for BodyEvidence {
    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if path.qself.is_none() && path.path.segments.len() == 1 {
            self.identifiers
                .insert(path.path.segments[0].ident.to_string(), ());
        }
        visit::visit_expr_path(self, path);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if matches!(local.pat, syn::Pat::Wild(_))
            && let Some(initializer) = &local.init
            && is_discarded_operation(&initializer.expr)
        {
            self.discarded_results += 1;
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_if(&mut self, expression: &'ast syn::ExprIf) {
        match target_cfg_value(&expression.cond) {
            Some(true) => self.visit_block(&expression.then_branch),
            Some(false) => {
                if let Some((_, alternate)) = &expression.else_branch {
                    self.visit_expr(alternate);
                }
            }
            None => visit::visit_expr_if(self, expression),
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        for statement in &block.stmts {
            if let syn::Stmt::Expr(syn::Expr::If(expression), _) = statement
                && let Some(target_value) = target_cfg_value(&expression.cond)
            {
                if target_value {
                    self.visit_block(&expression.then_branch);
                    if block_definitely_returns(&expression.then_branch) {
                        break;
                    }
                } else if let Some((_, alternate)) = &expression.else_branch {
                    self.visit_expr(alternate);
                    if expression_definitely_returns(alternate) {
                        break;
                    }
                }
                continue;
            }
            self.visit_stmt(statement);
        }
    }

    fn visit_macro(&mut self, macro_: &'ast syn::Macro) {
        collect_token_identifiers(macro_.tokens.clone(), &mut self.identifiers);
        visit::visit_macro(self, macro_);
    }
}

fn block_returns_constant_success(block: &syn::Block) -> bool {
    if block.stmts.len() != 1 {
        return false;
    }
    let Some(syn::Stmt::Expr(expression, None)) = block.stmts.last() else {
        return false;
    };
    constant_success_expression(expression)
}

fn is_discarded_operation(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Call(call) => {
            match call.func.as_ref() {
                syn::Expr::Path(path) => !path.path.segments.last().is_some_and(|segment| {
                    segment.ident == "from_raw" || segment.ident == "size_of"
                }),
                _ => true,
            }
        }
        syn::Expr::MethodCall(_) => true,
        syn::Expr::Unsafe(unsafe_) => {
            unsafe_
                .block
                .stmts
                .last()
                .is_some_and(|statement| match statement {
                    syn::Stmt::Expr(expression, None) => is_discarded_operation(expression),
                    _ => false,
                })
        }
        syn::Expr::Paren(paren) => is_discarded_operation(&paren.expr),
        syn::Expr::Group(group) => is_discarded_operation(&group.expr),
        _ => false,
    }
}

fn target_cfg_value(expression: &syn::Expr) -> Option<bool> {
    let syn::Expr::Macro(macro_) = expression else {
        return None;
    };
    if !macro_.mac.path.is_ident("cfg") {
        return None;
    }
    let condition = macro_.mac.tokens.to_string();
    if !condition.contains("target_arch") || !condition.contains("powerpc") {
        return None;
    }
    Some(!condition.trim_start().starts_with("not"))
}

fn block_definitely_returns(block: &syn::Block) -> bool {
    block.stmts.last().is_some_and(|statement| match statement {
        syn::Stmt::Expr(expression, _) => expression_definitely_returns(expression),
        _ => false,
    })
}

fn expression_definitely_returns(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Return(_) => true,
        syn::Expr::Block(block) => block_definitely_returns(&block.block),
        syn::Expr::Unsafe(block) => block_definitely_returns(&block.block),
        _ => false,
    }
}

fn collect_token_identifiers(
    stream: proc_macro2::TokenStream,
    identifiers: &mut BTreeMap<String, ()>,
) {
    for token in stream {
        match token {
            proc_macro2::TokenTree::Ident(identifier) => {
                identifiers.insert(identifier.to_string(), ());
            }
            proc_macro2::TokenTree::Group(group) => {
                collect_token_identifiers(group.stream(), identifiers);
            }
            _ => {}
        }
    }
}

fn constant_success_expression(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Lit(literal) => match &literal.lit {
            syn::Lit::Bool(value) => value.value,
            syn::Lit::Int(value) => value.base10_parse::<u128>().is_ok_and(|value| value <= 1),
            _ => false,
        },
        syn::Expr::Path(path) => path.path.segments.last().is_some_and(|segment| {
            let name = segment.ident.to_string();
            name.ends_with("_OK") || name.ends_with("_READY") || name.ends_with("_SUCCESSFUL")
        }),
        syn::Expr::Call(call) => {
            let is_ok = match call.func.as_ref() {
                syn::Expr::Path(path) => path
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "Ok"),
                _ => false,
            };
            is_ok
                && call.args.len() == 1
                && matches!(call.args.first(), Some(syn::Expr::Tuple(tuple)) if tuple.elems.is_empty())
        }
        syn::Expr::Paren(paren) => constant_success_expression(&paren.expr),
        syn::Expr::Group(group) => constant_success_expression(&group.expr),
        syn::Expr::Block(block) => block_returns_constant_success(&block.block),
        syn::Expr::Unsafe(block) => block_returns_constant_success(&block.block),
        _ => false,
    }
}

fn is_public_trait(line: &str) -> bool {
    line.starts_with("pub trait ") || line.starts_with("pub unsafe trait ")
}

fn is_public_function_line(line: &str) -> bool {
    if !line.starts_with("pub ") {
        return false;
    }
    if line.starts_with("pub(crate)")
        || line.starts_with("pub(super)")
        || line.starts_with("pub(self)")
    {
        return false;
    }
    contains_fn_token(line)
}

fn is_trait_function_line(line: &str) -> bool {
    contains_fn_token(line)
        && !line.starts_with("type ")
        && !line.starts_with("const ")
        && !line.starts_with("//")
}

fn contains_fn_token(line: &str) -> bool {
    line == "fn" || line.starts_with("fn ") || line.contains(" fn ")
}

fn function_name(line: &str) -> Option<String> {
    let fn_index = if line.starts_with("fn ") {
        0
    } else {
        line.find(" fn ")? + 1
    };
    let rest = line.get(fn_index + 3..)?.trim_start();
    let name = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn brace_delta(line: &str) -> i32 {
    let opens = line.bytes().filter(|byte| *byte == b'{').count() as i32;
    let closes = line.bytes().filter(|byte| *byte == b'}').count() as i32;
    opens - closes
}

#[derive(Default)]
struct RustMaskState {
    block_comment_depth: u32,
    raw_string_hashes: Option<usize>,
}

fn mask_rust_line(line: &str, state: &mut RustMaskState) -> String {
    let bytes = line.as_bytes();
    let mut output = String::with_capacity(line.len());
    let mut index = 0;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    while index < bytes.len() {
        if state.block_comment_depth > 0 {
            if index + 1 < bytes.len() && bytes[index] == b'/' && bytes[index + 1] == b'*' {
                state.block_comment_depth += 1;
                output.push_str("  ");
                index += 2;
            } else if index + 1 < bytes.len() && bytes[index] == b'*' && bytes[index + 1] == b'/' {
                state.block_comment_depth -= 1;
                output.push_str("  ");
                index += 2;
            } else {
                output.push(' ');
                index += 1;
            }
            continue;
        }
        if let Some(hashes) = state.raw_string_hashes {
            if bytes[index] == b'"'
                && bytes
                    .get(index + 1..index + 1 + hashes)
                    .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
            {
                output.extend(std::iter::repeat_n(' ', hashes + 1));
                index += hashes + 1;
                state.raw_string_hashes = None;
            } else {
                output.push(' ');
                index += 1;
            }
            continue;
        }
        if !in_string
            && !in_char
            && let Some((prefix_len, hashes)) = raw_string_start(bytes, index)
        {
            output.extend(std::iter::repeat_n(' ', prefix_len + hashes + 1));
            index += prefix_len + hashes + 1;
            state.raw_string_hashes = Some(hashes);
            continue;
        }
        if !in_string && !in_char && index + 1 < bytes.len() && bytes[index] == b'/' {
            if bytes[index + 1] == b'/' {
                output.extend(std::iter::repeat_n(' ', bytes.len() - index));
                break;
            }
            if bytes[index + 1] == b'*' {
                state.block_comment_depth = 1;
                output.push_str("  ");
                index += 2;
                continue;
            }
        }
        let byte = bytes[index];
        if in_string {
            output.push(' ');
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else if in_char {
            output.push(' ');
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'\'' {
                in_char = false;
            }
        } else if byte == b'"' {
            in_string = true;
            output.push(' ');
        } else if byte == b'\'' {
            let next = bytes.get(index + 1).copied();
            let after_next = bytes.get(index + 2).copied();
            let is_lifetime = next
                .is_some_and(|value| value.is_ascii_alphabetic() || value == b'_')
                && after_next != Some(b'\'');
            if is_lifetime {
                output.push('\'');
            } else {
                in_char = true;
                output.push(' ');
            }
        } else {
            output.push(byte as char);
        }
        index += 1;
    }
    output
}

fn raw_string_start(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut index = start;
    if bytes.get(index) == Some(&b'b') || bytes.get(index) == Some(&b'c') {
        index += 1;
    }
    if bytes.get(index) != Some(&b'r') {
        return None;
    }
    index += 1;
    let hash_start = index;
    while bytes.get(index) == Some(&b'#') {
        index += 1;
    }
    (bytes.get(index) == Some(&b'"')).then_some((index - start + 1, index - hash_start))
}

fn render_csv(entries: &[ApiEntry]) -> String {
    let mut output = String::from(
        "crate,symbol,interface_kind,target_implementation_file,line,implementation_class,declared_classification,verification_evidence,probe_coverage,known_limitations\n",
    );
    for entry in entries {
        let line = entry.line.to_string();
        let fields = [
            entry.crate_name.as_str(),
            entry.symbol.as_str(),
            entry.interface_kind.as_str(),
            entry.implementation_file.as_str(),
            line.as_str(),
            entry.classification.as_str(),
            entry.declared_classification.as_str(),
            entry.evidence.as_str(),
            entry.probe.as_str(),
            entry.limitations.as_str(),
        ];
        output.push_str(
            &fields
                .iter()
                .map(|field| csv_escape(field))
                .collect::<Vec<_>>()
                .join(","),
        );
        output.push('\n');
    }
    output
}

fn render_status(entries: &[ApiEntry]) -> String {
    let mut by_class = BTreeMap::<&str, usize>::new();
    let mut by_crate = BTreeMap::<&str, (usize, usize)>::new();
    for entry in entries {
        *by_class.entry(&entry.classification).or_default() += 1;
        let crate_counts = by_crate.entry(&entry.crate_name).or_default();
        crate_counts.0 += 1;
        if DISALLOWED_CLASSES.contains(&entry.classification.as_str()) {
            crate_counts.1 += 1;
        }
    }

    let mut output = String::from(
        "# Generated implementation status\n\nThis file is generated by `cargo xtask inventory`. Do not edit it directly.\n\n",
    );
    output.push_str(
        "`dolphin-partial` means the listed behavior passed only within its recorded Dolphin scope. It does not prove physical Wii behavior or any operation named as unverified in the evidence limitation.\n\n",
    );
    output.push_str("## Classification totals\n\n| Class | APIs |\n| --- | ---: |\n");
    for (classification, count) in by_class {
        output.push_str(&format!("| `{classification}` | {count} |\n"));
    }
    output.push_str("\n## Crate totals\n\n| Crate | Public APIs | Disallowed implementations |\n| --- | ---: | ---: |\n");
    for (crate_name, (total, disallowed)) in by_crate {
        output.push_str(&format!("| `{crate_name}` | {total} | {disallowed} |\n"));
    }
    output
}

fn render_formatter_status(contracts: &HashMap<String, Vec<IpcEvidenceContract>>) -> String {
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut rows = Vec::new();
    for (id, evidence) in contracts {
        for item in evidence {
            *counts.entry(item.status.as_str()).or_default() += 1;
            rows.push((id.as_str(), item));
        }
    }
    rows.sort_by_key(|(id, item)| (*id, item.symbol.as_str(), item.function.as_str()));
    let mut output = String::from(
        "# IPC formatter evidence status\n\nGenerated from `config/ipc-format-contracts.csv` by `cargo xtask inventory`. Status describes the stated contract evidence, not the implementation API classification.\n\n",
    );
    output.push_str("## Status counts\n\n");
    output.push_str(&format!(
        "verified: {} | partial: {} | blocked: {} | unverified: {}\n\n",
        counts.get("verified").copied().unwrap_or(0),
        counts.get("partial").copied().unwrap_or(0),
        counts.get("blocked").copied().unwrap_or(0),
        counts.get("unverified").copied().unwrap_or(0),
    ));
    output.push_str("`unverified` is the actionable count. `blocked` records requirements that need unavailable prerequisites.\n\n");
    output.push_str("## Contracts\n\n| Contract | API | Status | Remaining evidence |\n| --- | --- | --- | --- |\n");
    for (id, item) in rows {
        output.push_str(&format!(
            "| `{id}` | `{}` | `{}` | {} |\n",
            item.symbol,
            item.status,
            item.limitation.replace('|', "\\|")
        ));
    }
    output
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatter_assembly_scan_rejects_an_unlisted_global() {
        let found = assembly_global_symbols(
            ".globl IOS_IoctlvFormat\n.global IOS_IoctlvFormatAsync\n.globl helper",
        );
        let listed = HashSet::from(["IOS_IoctlvFormat".to_string()]);
        assert_eq!(
            missing_assembly_exports(&found, &listed),
            ["IOS_IoctlvFormatAsync"]
        );
    }

    #[test]
    fn formatter_assembly_inventory_matches_the_repository_source() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        validate_formatter_assembly_exports(root).expect("formatter globals are inventoried");
    }

    #[test]
    fn contracts_require_explicit_coverage_runner_and_matching_identity() {
        let item = Evidence {
            crate_name: "ogc-ipc".into(),
            path: "src/lib.rs".into(),
            symbol: "IOS_IoctlvFormat".into(),
            line: 30,
            classification: "ios".into(),
            kind: "hardware-probe".into(),
            evidence_path: "source.rs".into(),
            function: "probe".into(),
            api_path: Some("api".into()),
            scope: None,
            contracts: vec!["contract".into()],
        };
        let contract = IpcEvidenceContract {
            crate_name: item.crate_name.clone(),
            symbol: item.symbol.clone(),
            dolphin_runner: "runtime_probe".into(),
            evidence_path: item.evidence_path.clone(),
            function: item.function.clone(),
            api_path: "api".into(),
            status: "partial".into(),
            limitation: "remaining check".into(),
        };
        let mut contracts = HashMap::from([("contract".into(), vec![contract.clone(), contract])]);
        let mut coverage = BTreeMap::from([(
            "ipc-format-probe".into(),
            ProbeCoverage {
                source: "source.rs".into(),
                dolphin_runner: Some("runtime_probe".into()),
            },
        )]);
        assert!(validate_contracts(&item, "ipc-format-probe", &coverage, &contracts).is_ok());
        assert!(validate_contracts(&item, "missing", &coverage, &contracts).is_err());
        contracts.get_mut("contract").unwrap()[1].dolphin_runner = "unknown_probe".into();
        assert!(validate_contracts(&item, "ipc-format-probe", &coverage, &contracts).is_err());
        contracts.get_mut("contract").unwrap()[1].dolphin_runner = "network_probe".into();
        assert!(crate::dolphin::is_known_probe("network_probe"));
        assert!(validate_contracts(&item, "ipc-format-probe", &coverage, &contracts).is_err());
        contracts.get_mut("contract").unwrap()[1].dolphin_runner = "runtime_probe".into();
        coverage.insert(
            "other".into(),
            ProbeCoverage {
                source: "source.rs".into(),
                dolphin_runner: Some("network_probe".into()),
            },
        );
        assert!(validate_contracts(&item, "other", &coverage, &contracts).is_err());
        coverage.get_mut("ipc-format-probe").unwrap().source = "wrong.rs".into();
        assert!(validate_contracts(&item, "ipc-format-probe", &coverage, &contracts).is_err());
        coverage.get_mut("ipc-format-probe").unwrap().source = "source.rs".into();
        coverage.get_mut("ipc-format-probe").unwrap().dolphin_runner = Some("unknown_probe".into());
        assert!(validate_contracts(&item, "ipc-format-probe", &coverage, &contracts).is_err());
    }

    #[test]
    fn formatter_status_report_keeps_hardware_blocked_separate() {
        let make = |status: &str, limitation: &str| IpcEvidenceContract {
            crate_name: "ogc-ipc".into(),
            symbol: "IOS_IoctlvFormat".into(),
            dolphin_runner: "runtime_probe".into(),
            evidence_path: "probe.rs".into(),
            function: "run".into(),
            api_path: "api".into(),
            status: status.into(),
            limitation: limitation.into(),
        };
        let contracts = HashMap::from([
            ("ready".into(), vec![make("verified", "complete")]),
            (
                "in-progress".into(),
                vec![make("partial", "Dolphin cases remain")],
            ),
            (
                "hardware".into(),
                vec![make("blocked", "physical Wii required")],
            ),
            (
                "actionable".into(),
                vec![make("unverified", "probe missing")],
            ),
        ]);
        let report = render_formatter_status(&contracts);
        assert!(report.contains("verified: 1 | partial: 1 | blocked: 1 | unverified: 1"));
        assert!(report.contains("`unverified` is the actionable count"));
        assert!(report.contains("`blocked`"));
    }

    #[test]
    fn global_assembly_exports_keep_their_dolphin_scope_limitations() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask is inside the workspace");
        let entries = collect_inventory(root).expect("inventory loads assembly exports");
        for (symbol, line) in [("IOS_IoctlvFormat", 30), ("IOS_IoctlvFormatAsync", 63)] {
            let entry = entries
                .iter()
                .find(|entry| entry.crate_name == "ogc-ipc" && entry.symbol == symbol)
                .expect("assembly export is inventoried");
            assert_eq!(entry.interface_kind, "assembly-export");
            assert_eq!(entry.implementation_file, "crates/core/ogc-ipc/src/lib.rs");
            assert_eq!(entry.line, line);
            assert_eq!(entry.classification, "dolphin-partial");
            assert!(entry.evidence.contains("probe_ipc_format_bridge"));
        }
    }

    #[test]
    fn public_function_scan_excludes_restricted_visibility() {
        assert!(!is_public_function_line("pub(crate) fn backend() {}"));
        assert!(!is_public_function_line("pub(super) unsafe fn helper() {}"));
        assert!(is_public_function_line("pub fn api() {}"));
    }

    #[test]
    fn public_function_scan_honors_cfg_across_doc_comments() {
        let source = r#"
            #[cfg(target_arch = "powerpc")]
            /// PowerPC implementation.
            pub fn target_api() {}

            #[cfg(not(target_arch = "powerpc"))]
            /// Host-only implementation.
            pub fn host_api() {}
        "#;

        let functions = scan_public_functions_source(source).expect("valid Rust source");
        let names: Vec<_> = functions
            .iter()
            .map(|function| function.name.as_str())
            .collect();
        assert_eq!(names, ["target_api"]);
    }

    #[test]
    fn public_function_scan_excludes_formatter_probe_only_hooks() {
        let source = r#"
            #![cfg(feature = "format-probe-hooks")]
            pub fn reset_probe_ledger() {}
        "#;

        let functions = scan_public_functions_source(source).expect("valid Rust source");
        assert!(functions.is_empty());
    }

    #[test]
    fn completeness_json_counts_each_api_once_and_keeps_all_reasons() {
        let entry = ApiEntry {
            crate_name: "demo".into(),
            symbol: "read".into(),
            interface_kind: "rust-function".into(),
            implementation_file: "crates/demo/src/lib.rs".into(),
            line: 12,
            classification: "unverified".into(),
            declared_classification: "hardware".into(),
            evidence: "none".into(),
            probe: "missing".into(),
            limitations: "no runtime evidence".into(),
        };
        let mut verified = entry.clone();
        verified.symbol = "write".into();
        verified.classification = "hardware".into();
        verified.probe = "present".into();
        let probes = BTreeMap::from([("present", None)]);
        let entries = [entry, verified];
        let report = serde_json::to_value(completeness_report(&entries, &probes)).unwrap();
        assert_eq!(
            report,
            serde_json::json!({
                "schema_version": 1,
                "inventory_count": 2,
                "unverified_count": 1,
                "remaining_count": 1,
                "entries": [{
                    "crate": "demo",
                    "symbol": "read",
                    "interface_kind": "rust-function",
                    "implementation_file": "crates/demo/src/lib.rs",
                    "line": 12,
                    "classification": "unverified",
                    "probe": "missing",
                    "reasons": [
                        {"code": "unmapped_probe", "message": "probe `missing` is not mapped in config/probe-coverage.csv"},
                        {"code": "disallowed_classification", "message": "no runtime evidence"}
                    ]
                }]
            })
        );
    }

    #[test]
    fn accepts_target_gated_unit_tests_as_evidence_sources() {
        let source =
            "#[cfg(all(test, target_arch = \"powerpc\"))] mod tests { #[test] fn api_test() {} }";

        assert!(is_host_test_source("crates/sample/src/lib.rs", source));
        assert!(!is_host_test_source(
            "crates/sample/src/lib.rs",
            "pub fn api() {}"
        ));
    }

    #[test]
    fn parsed_evidence_cache_parses_each_source_once() {
        let mut cache = ParsedEvidenceCache::default();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/inventory.rs");

        let first = cache.get_or_parse(&path).unwrap() as *const ParsedEvidence;
        let second = cache.get_or_parse(&path).unwrap() as *const ParsedEvidence;

        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
    }

    fn finding_for(source: &str, symbol: &str) -> Option<SourceFinding> {
        analyze_function_bodies(source)
            .expect("test source must parse")
            .into_iter()
            .find_map(|((name, _), finding)| (name == symbol).then_some(finding))
    }

    #[test]
    fn constant_success_is_source_derived_fake_success() {
        let finding = finding_for("pub fn initialize() -> i32 { 0 }", "initialize")
            .expect("constant success must be reported");

        assert_eq!(finding.classification, Some("fake-success"));
        assert!(finding.reason.contains("unconditional success"));
    }

    #[test]
    fn ignored_argument_is_source_derived_stub() {
        let finding = finding_for("pub fn configure(value: u32) -> bool { true }", "configure")
            .expect("ignored argument must be reported");

        assert_eq!(finding.classification, Some("stub"));
        assert!(finding.reason.contains("value"));
    }

    #[test]
    fn underscore_argument_is_intentionally_ignored() {
        let source = "pub fn exit(_status: i32) -> ! { loop {} }";

        assert_eq!(finding_for(source, "exit"), None);
    }

    #[test]
    fn discarded_result_is_source_review_not_fake_success() {
        let source = r#"
            pub fn save(value: u32) -> i32 {
                let _ = backend::save(value);
                0
            }
        "#;
        let finding = finding_for(source, "save").expect("discarded result must be reported");

        assert_eq!(finding.classification, None);
        assert!(finding.reason.contains("discards 1 operation result"));
    }

    #[test]
    fn valid_forwarding_function_has_no_source_finding() {
        let source = "pub fn save(value: u32) -> i32 { backend::save(value) }";

        assert_eq!(finding_for(source, "save"), None);
    }

    #[test]
    fn target_forwarding_ignores_unreachable_host_fallback() {
        let source = r#"
            pub fn save(value: u32) -> i32 {
                if cfg!(target_arch = "powerpc") {
                    return backend::save(value);
                }
                let _ = value;
                0
            }
        "#;

        assert_eq!(finding_for(source, "save"), None);
    }

    #[test]
    fn public_function_scan_ignores_braces_and_comment_markers_in_raw_strings() {
        let source = r####"
            pub fn outer() {
                let multiline = br##"{
                    // this is string data, not a comment
                    /* neither is this */
                    pub fn hidden() {}
                }"##;
                /* Outer comment { /* nested comment } */ still a comment. */
                let raw_c = cr#" } // /* "#;
            }

            pub fn after() {}
        "####;

        let functions = scan_public_functions_source(source).expect("valid Rust source");
        let names: Vec<_> = functions
            .iter()
            .map(|function| function.name.as_str())
            .collect();
        assert_eq!(names, ["outer", "after"]);
    }

    #[test]
    fn public_function_scan_keeps_existing_production_methods_visible() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask is inside the workspace");
        let path = root.join("crates/media/asnd-sys/src/lib.rs");
        let functions = scan_public_functions(&path).expect("production source parses");
        assert!(
            functions
                .iter()
                .any(|function| function.name == "add_buffer" && function.line == 640),
            "scanned methods: {:?}",
            functions
                .iter()
                .map(|function| (&function.name, function.line))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn partial_evidence_keeps_production_verification_open() {
        for scope in [
            "mock-dispatch",
            "closed-service",
            "disabled-adapter",
            "invalid-channel",
            "invalid-arguments",
            "empty-channel",
            "no-controller",
            "no-controller-and-invalid-arguments",
            "active-search-state",
            "zero-length",
            "empty-queue",
            "no-pending-interrupt",
            "repeated-initialization",
            "no-event-hook",
            "synchronous-scalar-output",
            "scalar-output",
            "zero-tag-count",
        ] {
            let scope = EvidenceScope::parse(scope).unwrap();
            assert_eq!(scope.classification("protocol"), "unverified");
            assert!(scope.limitation().contains("unverified"));
        }
        assert_eq!(
            EvidenceScope::DolphinCacheRegister.classification("hardware"),
            "hardware"
        );
        for (name, limitation) in [
            (
                "ipc-partial-dolphin",
                "Physical Wii cache behavior remains unverified",
            ),
            ("ipc-reboot-guard", "System Menu handoff remains unverified"),
            (
                "ios-lifecycle-dolphin",
                "Physical Wii reload behavior remains unverified",
            ),
        ] {
            let scope = EvidenceScope::parse(name).unwrap();
            assert_eq!(scope.classification("ios"), "dolphin-partial");
            assert!(scope.limitation().contains(limitation));
        }
        let scalar_scope = EvidenceScope::parse("scalar-and-buffer").unwrap();
        assert_eq!(scalar_scope.classification("ios"), "dolphin-partial");
        assert!(
            scalar_scope
                .limitation()
                .contains("physical cache behavior")
        );
        let zero_tag_scope = EvidenceScope::parse("zero-tag-count").unwrap();
        assert_eq!(zero_tag_scope.classification("hardware"), "unverified");
        assert!(zero_tag_scope.limitation().contains("non-zero tag counts"));
        assert!(
            EvidenceScope::DolphinCacheRegister
                .limitation()
                .contains("Physical")
        );
        assert!(EvidenceScope::parse("unknown").is_none());
        let active_search_scope = EvidenceScope::parse("active-search-state").unwrap();
        assert_eq!(active_search_scope.classification("hardware"), "unverified");
        assert!(
            active_search_scope
                .limitation()
                .contains("usable controller channel")
        );
    }

    #[test]
    fn hardware_classification_rejects_host_test_evidence() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "initialize".to_string(),
            line: 1,
            classification: "hardware".to_string(),
            kind: "host-test".to_string(),
            evidence_path: "unused".to_string(),
            function: "initialize_test".to_string(),
            api_path: None,
            scope: None,
            contracts: vec![],
        };

        let error = verify_evidence(Path::new("."), &evidence, "hardware")
            .expect_err("hardware classifications require a hardware probe");
        assert!(error.contains("cannot use `host-test` evidence"));
    }

    #[test]
    fn evidence_classification_must_match_declaration() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "parse".to_string(),
            line: 1,
            classification: "pure".to_string(),
            kind: "host-test".to_string(),
            evidence_path: "unused".to_string(),
            function: "parse_test".to_string(),
            api_path: None,
            scope: None,
            contracts: vec![],
        };

        let error = verify_evidence(Path::new("."), &evidence, "protocol")
            .expect_err("evidence cannot verify another classification");
        assert!(error.contains("verifies `pure`, but the code is declared `protocol`"));
    }

    #[test]
    fn runtime_driver_trait_rejects_unqualified_name_collision() {
        let evidence = Evidence {
            crate_name: "ogc-runtime".to_string(),
            path: "src/driver.rs".to_string(),
            symbol: "close".to_string(),
            line: 46,
            classification: "pure".to_string(),
            kind: "hardware-probe".to_string(),
            evidence_path: "probe.rs".to_string(),
            function: "probe".to_string(),
            api_path: None,
            scope: None,
            contracts: vec![],
        };
        let error = verify_evidence_call(
            "fn probe() { unrelated.close(); }",
            &evidence,
            Path::new("probe.rs"),
        )
        .expect_err("an unrelated close call must not prove the driver trait");
        assert!(error.contains("requires a qualified API path"));

        let constructor = Evidence {
            path: "src/scheduler.rs".to_string(),
            symbol: "new".to_string(),
            ..evidence
        };
        let error = verify_evidence_call(
            "fn probe() { FileDescriptor::new(7); }",
            &constructor,
            Path::new("probe.rs"),
        )
        .expect_err("an unrelated constructor must not prove the thread ID constructor");
        assert!(error.contains("requires a qualified API path"));
    }

    #[test]
    fn valid_qualified_api_call_in_named_test_is_accepted() {
        let evidence = Evidence {
            crate_name: "xtask".to_string(),
            path: "src/inventory.rs".to_string(),
            symbol: "parse".to_string(),
            line: 1,
            classification: "pure".to_string(),
            kind: "host-test".to_string(),
            evidence_path: "sample.rs".to_string(),
            function: "parses_valid_input".to_string(),
            api_path: None,
            scope: None,
            contracts: vec![],
        };
        let source = r#"
            #[test]
            fn parses_valid_input() {
                assert_eq!(parser::parse("valid"), Ok(()));
            }
        "#;

        verify_evidence_call(source, &evidence, Path::new("sample.rs"))
            .expect("API calls in the named host test must pass");
    }

    #[test]
    fn api_name_in_comment_or_string_is_not_evidence() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "parse".to_string(),
            line: 1,
            classification: "pure".to_string(),
            kind: "host-test".to_string(),
            evidence_path: "sample.rs".to_string(),
            function: "does_not_call_api".to_string(),
            api_path: None,
            scope: None,
            contracts: vec![],
        };
        let source = r#"
            #[test]
            fn does_not_call_api() {
                // parser::parse("comment only")
                let _name = "parser::parse(\"string only\")";
                assert!(true);
            }
        "#;

        let error = verify_evidence_call(source, &evidence, Path::new("sample.rs"))
            .expect_err("comments and strings must not count as API calls");
        assert!(error.contains("does not call the API"));
    }

    #[test]
    fn evidence_api_path_must_be_present_in_the_named_function() {
        let evidence = Evidence {
            crate_name: "ogc-card".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "read".to_string(),
            line: 1,
            classification: "protocol".to_string(),
            kind: "hardware-probe".to_string(),
            evidence_path: "probes/card.rs".to_string(),
            function: "probe".to_string(),
            api_path: Some("CARD_INTERFACE.read".to_string()),
            scope: None,
            contracts: vec![],
        };
        let source = r#"
            use std::fs;
            fn probe() { let _ = fs::read("sample"); }
        "#;

        let error = verify_evidence_call(source, &evidence, Path::new("probes/card.rs"))
            .expect_err("the mapped API path must occur in the evidence function");
        assert!(
            error.contains("does not call the API")
                || error.contains("does not bind or call API path `CARD_INTERFACE.read`")
        );
    }

    #[test]
    fn qualified_api_path_and_call_in_named_function_are_accepted() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "read".to_string(),
            line: 1,
            classification: "protocol".to_string(),
            kind: "hardware-probe".to_string(),
            evidence_path: "probes/sample.rs".to_string(),
            function: "probe".to_string(),
            api_path: Some("sample_api::read".to_string()),
            scope: None,
            contracts: vec![],
        };
        let source = r#"
            fn probe() {
                let Some(read) = sample_api::read else { return; };
                let _ = read();
            }
        "#;

        verify_evidence_call(source, &evidence, Path::new("probes/sample.rs"))
            .expect("the expected API path and call must pass");
    }

    #[test]
    fn api_path_can_name_the_public_facade_for_an_internal_symbol() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "read".to_string(),
            line: 1,
            classification: "protocol".to_string(),
            kind: "hardware-probe".to_string(),
            evidence_path: "probes/sample.rs".to_string(),
            function: "probe".to_string(),
            api_path: Some("sample_api::write".to_string()),
            scope: None,
            contracts: vec![],
        };
        let source = "fn probe() { let _ = sample_api::write(); }";

        verify_evidence_call(source, &evidence, Path::new("probes/sample.rs"))
            .expect("the exact public facade call can evidence its internal implementation");
    }

    #[test]
    fn api_path_must_match_a_method_receiver() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "flush_lun".to_string(),
            line: 1,
            classification: "protocol".to_string(),
            kind: "host-test".to_string(),
            evidence_path: "crates/sample/src/lib.rs".to_string(),
            function: "probe".to_string(),
            api_path: Some("operations.flush_lun".to_string()),
            scope: None,
            contracts: vec![],
        };
        let source = "#[test] fn probe() { other.flush_lun(); }";

        let error = verify_evidence_call(source, &evidence, Path::new("crates/sample/src/lib.rs"))
            .expect_err("a same-named method on another receiver must not count");
        assert!(error.contains("does not call the API"));
    }

    #[test]
    fn api_path_normalizes_indexed_receivers() {
        let expression = syn::parse_str::<syn::Expr>("drives[0].remove")
            .expect("the test expression must parse");

        assert_eq!(
            expression_path(&expression).as_deref(),
            Some("drives[0].remove")
        );
    }

    #[test]
    fn api_call_outside_named_test_is_not_evidence() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "parse".to_string(),
            line: 1,
            classification: "pure".to_string(),
            kind: "host-test".to_string(),
            evidence_path: "sample.rs".to_string(),
            function: "unrelated_test".to_string(),
            api_path: None,
            scope: None,
            contracts: vec![],
        };
        let source = r#"
            fn helper() { let _ = parser::parse("value"); }
            #[test]
            fn unrelated_test() { assert!(true); }
        "#;

        let error = verify_evidence_call(source, &evidence, Path::new("sample.rs"))
            .expect_err("a call in another function must not count");
        assert!(error.contains("does not call the API"));
    }

    #[test]
    fn host_evidence_requires_a_test_function() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "parse".to_string(),
            line: 1,
            classification: "pure".to_string(),
            kind: "host-test".to_string(),
            evidence_path: "sample.rs".to_string(),
            function: "helper".to_string(),
            api_path: None,
            scope: None,
            contracts: vec![],
        };
        let source = "fn helper() { parser::parse(\"value\"); }";

        let error = verify_evidence_call(source, &evidence, Path::new("sample.rs"))
            .expect_err("host evidence must name a #[test] function");
        assert!(error.contains("missing or is not a #[test]"));
    }

    #[test]
    fn duplicate_evidence_function_names_are_rejected() {
        let evidence = Evidence {
            crate_name: "sample".to_string(),
            path: "src/lib.rs".to_string(),
            symbol: "parse".to_string(),
            line: 1,
            classification: "pure".to_string(),
            kind: "host-test".to_string(),
            evidence_path: "sample.rs".to_string(),
            function: "parses_input".to_string(),
            api_path: None,
            scope: None,
            contracts: vec![],
        };
        let source = r#"
            mod first {
                #[test]
                fn parses_input() { parser::parse("value"); }
            }
            mod second {
                #[test]
                fn parses_input() { assert!(true); }
            }
        "#;

        let error = verify_evidence_call(source, &evidence, Path::new("sample.rs"))
            .expect_err("duplicate test names must not make evidence ambiguous");
        assert!(error.contains("is ambiguous"));
    }
}
