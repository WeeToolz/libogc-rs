use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    let entries = collect_inventory(root)?;
    let csv = render_csv(&entries);
    let csv_path = root.join("docs/generated/implementation-inventory.csv");
    if let Some(parent) = csv_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    }
    fs::write(&csv_path, csv).map_err(|err| format!("{}: {err}", csv_path.display()))?;

    let status = render_status(&entries);
    let status_path = root.join("docs/generated/implementation-status.md");
    fs::write(&status_path, status).map_err(|err| format!("{}: {err}", status_path.display()))?;

    println!(
        "wrote {} API entries to {}",
        entries.len(),
        csv_path.display()
    );
    Ok(())
}

pub fn check_inventory(root: &Path) -> Result<(), String> {
    let entries = collect_inventory(root)?;
    let expected = render_csv(&entries);
    let path = root.join("docs/generated/implementation-inventory.csv");
    let actual = fs::read_to_string(&path)
        .map_err(|err| format!("{}: {err}; run cargo xtask inventory", path.display()))?;
    if actual != expected {
        return Err(format!(
            "{} is stale; run cargo xtask inventory",
            path.display()
        ));
    }

    let expected_status = render_status(&entries);
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

    println!(
        "implementation inventory is current ({} entries)",
        entries.len()
    );
    Ok(())
}

pub fn audit_runtime_completeness(root: &Path) -> Result<(), String> {
    let entries = collect_inventory(root)?;
    let mut findings = Vec::new();
    let probe_sources = load_probe_coverage(root)?;
    let mut checked_probes = BTreeMap::new();
    for entry in entries {
        if entry.probe.trim().is_empty() || entry.probe == "none" {
            findings.push(format!(
                "{}::{} [{}] {}:{} — missing focused probe",
                entry.crate_name,
                entry.symbol,
                entry.classification,
                entry.implementation_file,
                entry.line
            ));
            continue;
        }
        if !checked_probes.contains_key(&entry.probe) {
            match probe_sources.get(&entry.probe) {
                Some(source) if root.join(source).is_file() => {
                    checked_probes.insert(entry.probe.clone(), source.clone());
                }
                Some(source) => findings.push(format!(
                    "{}::{} — focused probe source does not exist: {}",
                    entry.crate_name,
                    entry.symbol,
                    root.join(source).display()
                )),
                None => findings.push(format!(
                    "{}::{} — probe `{}` is not mapped in config/probe-coverage.csv",
                    entry.crate_name, entry.symbol, entry.probe
                )),
            }
        }
        if DISALLOWED_CLASSES.contains(&entry.classification.as_str()) {
            findings.push(format!(
                "{}::{} [{}] {}:{} — {}",
                entry.crate_name,
                entry.symbol,
                entry.classification,
                entry.implementation_file,
                entry.line,
                entry.limitations
            ));
        }
    }
    if findings.is_empty() {
        println!("runtime inventory contains no disallowed implementation classes");
        Ok(())
    } else {
        for finding in &findings {
            eprintln!("{finding}");
        }
        Err(format!(
            "runtime completeness audit found {} disallowed API implementation(s)",
            findings.len()
        ))
    }
}

fn load_probe_coverage(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let path = root.join("config/probe-coverage.csv");
    let rows = crate::workspace::read_csv(&path)?;
    let mut coverage = BTreeMap::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if row.len() != 2 || row[0].trim().is_empty() || row[1].trim().is_empty() {
            return Err(format!(
                "{}:{}: expected probe and source columns",
                path.display(),
                line_no + 1
            ));
        }
        if coverage.insert(row[0].clone(), row[1].clone()).is_some() {
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
    let roles = load_roles(root)?;
    let overrides = load_overrides(root)?;
    let evidence = load_evidence(root)?;
    let mut used_evidence = vec![false; evidence.len()];
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
                    let matching_evidence = evidence.iter().enumerate().find(|(_, item)| {
                        item.crate_name == role.package
                            && item.path == relative_to_crate
                            && item.symbol == function.name
                            && item.line == function.line
                    });
                    if let Some((index, item)) = matching_evidence {
                        verify_evidence(root, item, &declared_classification)?;
                        used_evidence[index] = true;
                        evidence_summary = format!(
                            "{}:{}::{} calls {}",
                            item.kind, item.evidence_path, item.function, item.symbol
                        );
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

fn load_evidence(root: &Path) -> Result<Vec<Evidence>, String> {
    let path = root.join("config/implementation-evidence.csv");
    let rows = read_csv(&path)?;
    let mut evidence = Vec::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if row.len() != 8 || row.iter().any(|field| field.trim().is_empty()) {
            return Err(format!(
                "{}:{}: expected eight non-empty evidence columns",
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
        };
        if evidence.iter().any(|existing: &Evidence| {
            existing.crate_name == item.crate_name
                && existing.path == item.path
                && existing.symbol == item.symbol
                && existing.line == item.line
        }) {
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

fn verify_evidence(root: &Path, evidence: &Evidence, declared: &str) -> Result<(), String> {
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
    let path = root.join(&evidence.evidence_path);
    let source = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let normalized_path = evidence.evidence_path.replace('\\', "/");
    match evidence.kind.as_str() {
        "hardware-probe" if !normalized_path.starts_with("probes/") => {
            return Err(format!(
                "{}::{} hardware evidence must be under probes/",
                evidence.crate_name, evidence.symbol
            ));
        }
        "host-test" if !is_host_test_source(&normalized_path, &source) => {
            return Err(format!(
                "{}::{} host evidence must be a test source",
                evidence.crate_name, evidence.symbol
            ));
        }
        _ => {}
    }
    verify_evidence_call(&source, evidence, &path)
}

fn is_host_test_source(path: &str, source: &str) -> bool {
    path.contains("/tests/") || source.contains("#[test]")
}

fn verify_evidence_call(source: &str, evidence: &Evidence, path: &Path) -> Result<(), String> {
    let file = syn::parse_file(source).map_err(|err| {
        format!(
            "{}::{} evidence source {} is invalid Rust: {err}",
            evidence.crate_name,
            evidence.symbol,
            path.display()
        )
    })?;
    let mut visitor = EvidenceFunctionVisitor {
        function: &evidence.function,
        symbol: &evidence.symbol,
        require_test: evidence.kind == "host-test",
        function_count: 0,
        call_found: false,
    };
    visitor.visit_file(&file);

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
    Ok(())
}

struct EvidenceFunctionVisitor<'a> {
    function: &'a str,
    symbol: &'a str,
    require_test: bool,
    function_count: usize,
    call_found: bool,
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
                found: false,
            };
            calls.visit_block(&function.block);
            self.call_found |= calls.found;
        }
        visit::visit_item_fn(self, function);
    }
}

struct EvidenceCallVisitor<'a> {
    symbol: &'a str,
    found: bool,
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
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) {
        self.found |= self.path_calls_symbol(&expression.func);
        visit::visit_expr_call(self, expression);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        self.found |= expression.method == self.symbol;
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

        if trimmed.starts_with("#[") {
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
    fn accepts_target_gated_unit_tests_as_evidence_sources() {
        let source =
            "#[cfg(all(test, target_arch = \"powerpc\"))] mod tests { #[test] fn api_test() {} }";

        assert!(is_host_test_source("crates/sample/src/lib.rs", source));
        assert!(!is_host_test_source(
            "crates/sample/src/lib.rs",
            "pub fn api() {}"
        ));
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
        };

        let error = verify_evidence(Path::new("."), &evidence, "protocol")
            .expect_err("evidence cannot verify another classification");
        assert!(error.contains("verifies `pure`, but the code is declared `protocol`"));
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
