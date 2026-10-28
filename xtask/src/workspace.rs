use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct CrateRole {
    pub package: String,
    pub path: PathBuf,
    pub layer: String,
    pub role: String,
    pub group: String,
    pub runtime: bool,
    pub host_policy: String,
    pub default_classification: String,
    pub probe: String,
}

#[derive(Clone, Debug)]
struct DependencyException {
    source: String,
    target: String,
    reason: String,
}

pub const GROUPS: &[&str] = &[
    "core", "storage", "input", "network", "audio", "debug", "hardware", "ui", "abi", "probes",
];

pub fn load_roles(root: &Path) -> Result<Vec<CrateRole>, String> {
    let path = root.join("config/crate-roles.csv");
    let rows = read_csv(&path)?;
    let mut roles = Vec::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if row.len() != 9 {
            return Err(format!(
                "{}:{}: expected 9 columns, found {}",
                path.display(),
                line_no + 1,
                row.len()
            ));
        }
        let runtime = match row[5].as_str() {
            "true" => true,
            "false" => false,
            other => {
                return Err(format!(
                    "{}:{}: invalid runtime value {other}",
                    path.display(),
                    line_no + 1
                ));
            }
        };
        roles.push(CrateRole {
            package: row[0].clone(),
            path: PathBuf::from(&row[1]),
            layer: row[2].clone(),
            role: row[3].clone(),
            group: row[4].clone(),
            runtime,
            host_policy: row[6].clone(),
            default_classification: row[7].clone(),
            probe: row[8].clone(),
        });
    }
    Ok(roles)
}

pub fn workspace_crates(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut manifests = Vec::new();
    collect_manifests(&root.join("crates"), &mut manifests)?;
    manifests
        .into_iter()
        .map(|manifest| package_name(&manifest))
        .collect()
}

fn collect_manifests(dir: &Path, manifests: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if entry.file_type().map_err(|err| err.to_string())?.is_dir() {
            collect_manifests(&path, manifests)?;
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml") {
            manifests.push(path);
        }
    }
    Ok(())
}

pub fn packages_for_group(root: &Path, group: &str) -> Result<Vec<String>, String> {
    if !GROUPS.contains(&group) {
        return Err(format!(
            "unknown crate group {group}; expected one of {}",
            GROUPS.join(", ")
        ));
    }
    let mut packages = load_roles(root)?
        .into_iter()
        .filter(|role| role.group == group && (role.runtime || matches!(group, "abi" | "probes")))
        .map(|role| role.package)
        .collect::<Vec<_>>();
    packages.sort();
    packages.dedup();
    if packages.is_empty() {
        return Err(format!("crate group {group} has no runtime packages"));
    }
    Ok(packages)
}

pub fn all_runtime_packages(root: &Path) -> Result<Vec<String>, String> {
    let mut packages = load_roles(root)?
        .into_iter()
        .filter(|role| role.runtime)
        .map(|role| role.package)
        .collect::<Vec<_>>();
    packages.sort();
    packages.dedup();
    Ok(packages)
}

pub fn check_role_manifest(root: &Path) -> Result<(), String> {
    let roles = load_roles(root)?;
    let mut seen_packages = BTreeSet::new();
    let mut seen_paths = BTreeSet::new();
    let mut findings = Vec::new();

    for role in &roles {
        if !seen_packages.insert(role.package.clone()) {
            findings.push(format!("duplicate package role: {}", role.package));
        }
        if !seen_paths.insert(role.path.clone()) {
            findings.push(format!("duplicate package path: {}", role.path.display()));
        }
        if layer_rank(&role.layer).is_none() {
            findings.push(format!("{} has unknown layer {}", role.package, role.layer));
        }
        if !GROUPS.contains(&role.group.as_str()) && role.group != "tool" {
            findings.push(format!("{} has unknown group {}", role.package, role.group));
        }
        if !matches!(
            role.host_policy.as_str(),
            "reject" | "abi-only" | "probe" | "tool"
        ) {
            findings.push(format!(
                "{} has unknown host policy {}",
                role.package, role.host_policy
            ));
        }
        let manifest = root.join(&role.path).join("Cargo.toml");
        if !manifest.exists() {
            findings.push(format!(
                "{} role points to missing manifest {}",
                role.package,
                manifest.display()
            ));
            continue;
        }
        match package_name(&manifest) {
            Ok(actual) if actual == role.package => {}
            Ok(actual) => findings.push(format!(
                "{} role points at package {} in {}",
                role.package,
                actual,
                manifest.display()
            )),
            Err(err) => findings.push(err),
        }
        if role.runtime && role.host_policy == "reject" {
            let source_root = root.join(&role.path).join("src");
            let mut guarded = false;
            for entry in [source_root.join("lib.rs"), source_root.join("main.rs")] {
                if !entry.exists() {
                    continue;
                }
                let source = fs::read_to_string(&entry)
                    .map_err(|err| format!("{}: {err}", entry.display()))?;
                guarded |= has_powerpc_compile_guard(&source);
            }
            if !guarded {
                findings.push(format!(
                    "{} has host_policy=reject but no PowerPC compile guard",
                    role.package
                ));
            }
        }
    }

    for manifest in discover_manifests(root)? {
        let name = package_name(&manifest)?;
        if !seen_packages.contains(&name) {
            findings.push(format!(
                "workspace package {name} at {} is missing from config/crate-roles.csv",
                manifest.display()
            ));
        }
    }

    if findings.is_empty() {
        println!("crate role manifest covers {} packages", roles.len());
        Ok(())
    } else {
        for finding in &findings {
            eprintln!("{finding}");
        }
        Err(format!(
            "crate role check found {} issue(s)",
            findings.len()
        ))
    }
}

fn has_powerpc_compile_guard(source: &str) -> bool {
    let rejects_host = source.contains("cfg(not(target_arch = \"powerpc\"))")
        || source.contains("not(target_arch = \"powerpc\")");
    rejects_host && source.contains("compile_error!")
}

pub fn check_dependency_direction(root: &Path) -> Result<(), String> {
    let roles = load_roles(root)?;
    let by_name = roles
        .iter()
        .map(|role| (role.package.as_str(), role))
        .collect::<BTreeMap<_, _>>();
    let exceptions = load_dependency_exceptions(root)?;
    let exception_pairs = exceptions
        .iter()
        .map(|exception| {
            (
                (exception.source.as_str(), exception.target.as_str()),
                exception,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut used_exceptions = BTreeSet::new();
    let mut findings = Vec::new();

    for source in &roles {
        let manifest = root.join(&source.path).join("Cargo.toml");
        for target_name in manifest_dependencies(&manifest)? {
            let Some(target) = by_name.get(target_name.as_str()) else {
                continue;
            };
            let source_rank = layer_rank(&source.layer).expect("validated layer");
            let target_rank = layer_rank(&target.layer).expect("validated layer");
            if target_rank <= source_rank {
                continue;
            }
            if let Some(exception) =
                exception_pairs.get(&(source.package.as_str(), target.package.as_str()))
            {
                used_exceptions.insert((source.package.clone(), target.package.clone()));
                println!(
                    "allowed dependency exception: {} -> {} ({})",
                    source.package, target.package, exception.reason
                );
                continue;
            }
            findings.push(format!(
                "{} ({}) depends upward on {} ({})",
                source.package, source.layer, target.package, target.layer
            ));
        }
    }

    for exception in &exceptions {
        if !used_exceptions.contains(&(exception.source.clone(), exception.target.clone())) {
            findings.push(format!(
                "stale dependency exception {} -> {}",
                exception.source, exception.target
            ));
        }
    }

    if findings.is_empty() {
        println!("dependency direction check passed");
        Ok(())
    } else {
        for finding in &findings {
            eprintln!("{finding}");
        }
        Err(format!(
            "dependency direction check found {} issue(s)",
            findings.len()
        ))
    }
}

fn discover_manifests(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut manifests = Vec::new();
    for top in ["crates", "examples", "probes"] {
        let dir = root.join(top);
        if !dir.exists() {
            continue;
        }
        let manifest = dir.join("Cargo.toml");
        if manifest.exists() {
            manifests.push(manifest);
        }
        for entry in fs::read_dir(&dir).map_err(|err| format!("{}: {err}", dir.display()))? {
            let entry = entry.map_err(|err| err.to_string())?;
            if entry.file_type().map_err(|err| err.to_string())?.is_dir() {
                let manifest = entry.path().join("Cargo.toml");
                if manifest.exists() {
                    manifests.push(manifest);
                }
            }
        }
    }
    let xtask = root.join("xtask/Cargo.toml");
    if xtask.exists() {
        manifests.push(xtask);
    }
    manifests.sort();
    Ok(manifests)
}

fn package_name(manifest: &Path) -> Result<String, String> {
    let source = fs::read_to_string(manifest)
        .map_err(|err| format!("failed to read {}: {err}", manifest.display()))?;
    let mut in_package = false;
    for line in source.lines() {
        let line = strip_comment(line).trim();
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package
            && line.starts_with("name")
            && let Some(value) = value_after_equals(line)
        {
            return Ok(unquote(value).to_string());
        }
    }
    Err(format!("{} has no [package] name", manifest.display()))
}

fn manifest_dependencies(manifest: &Path) -> Result<Vec<String>, String> {
    let source = fs::read_to_string(manifest)
        .map_err(|err| format!("failed to read {}: {err}", manifest.display()))?;
    let mut dependencies = Vec::new();
    let mut in_dependencies = false;
    for line in source.lines() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            in_dependencies = section == "dependencies"
                || section == "dev-dependencies"
                || section == "build-dependencies"
                || section.ends_with(".dependencies")
                || section.ends_with(".dev-dependencies")
                || section.ends_with(".build-dependencies");
            continue;
        }
        if !in_dependencies {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = unquote(key.trim()).to_string();
        let target = inline_table_string(value, "package").unwrap_or(key);
        dependencies.push(target);
    }
    dependencies.sort();
    dependencies.dedup();
    Ok(dependencies)
}

fn load_dependency_exceptions(root: &Path) -> Result<Vec<DependencyException>, String> {
    let path = root.join("config/dependency-exceptions.csv");
    let rows = read_csv(&path)?;
    let mut exceptions = Vec::new();
    for (line_no, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if row.len() != 5 {
            return Err(format!(
                "{}:{}: expected 5 columns, found {}",
                path.display(),
                line_no + 1,
                row.len()
            ));
        }
        exceptions.push(DependencyException {
            source: row[0].clone(),
            target: row[1].clone(),
            reason: row[2].clone(),
        });
    }
    Ok(exceptions)
}

fn layer_rank(layer: &str) -> Option<u8> {
    match layer {
        "abi" => Some(0),
        "core" => Some(1),
        "hardware" => Some(2),
        "services" => Some(3),
        "media" => Some(4),
        "compatibility" => Some(5),
        "probe" => Some(6),
        "tool" => Some(7),
        _ => None,
    }
}

fn inline_table_string(value: &str, key: &str) -> Option<String> {
    let needle = format!("{key} =");
    let start = value.find(&needle)? + needle.len();
    let rest = value[start..].trim_start();
    if !rest.starts_with('"') {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn value_after_equals(line: &str) -> Option<&str> {
    line.split_once('=').map(|(_, value)| value.trim())
}

fn unquote(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in line.as_bytes().iter().enumerate() {
        match *byte {
            b'\\' if quoted && !escaped => escaped = true,
            b'"' if !escaped => quoted = !quoted,
            b'#' if !quoted => return &line[..index],
            _ => escaped = false,
        }
    }
    line
}

pub fn read_csv(path: &Path) -> Result<Vec<Vec<String>>, String> {
    let source = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    source
        .lines()
        .enumerate()
        .map(|(index, line)| {
            parse_csv_line(line).map_err(|err| format!("{}:{}: {err}", path.display(), index + 1))
        })
        .collect()
}

fn parse_csv_line(line: &str) -> Result<Vec<String>, String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut chars = line.chars().peekable();
    let mut quoted = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            '"' => quoted = !quoted,
            ',' if !quoted => {
                fields.push(field);
                field = String::new();
            }
            _ => field.push(ch),
        }
    }
    if quoted {
        return Err("unterminated quoted CSV field".to_string());
    }
    fields.push(field);
    Ok(fields)
}

#[cfg(test)]
mod tests {
    use super::has_powerpc_compile_guard;

    #[test]
    fn detects_host_guard_with_test_exception() {
        let source = r#"
            #[cfg(all(not(target_arch = "powerpc"), not(test)))]
            compile_error!("PowerPC target required");
        "#;

        assert!(has_powerpc_compile_guard(source));
    }

    #[test]
    fn detects_direct_non_powerpc_guard() {
        let source = r#"
            #[cfg(not(target_arch = "powerpc"))]
            compile_error!("PowerPC target required");
        "#;

        assert!(has_powerpc_compile_guard(source));
    }

    #[test]
    fn rejects_target_checks_without_compile_error() {
        let source = r#"#[cfg(not(target_arch = "powerpc"))] fn host_path() {}"#;

        assert!(!has_powerpc_compile_guard(source));
    }
}
