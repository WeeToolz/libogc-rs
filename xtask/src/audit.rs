use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::workspace::{load_roles, read_csv};

#[derive(Debug)]
struct Allowance {
    path: String,
    needle: String,
    reason: String,
    owner: String,
    remove_by: String,
}

pub fn audit_source_structure(root: &Path) -> Result<(), String> {
    let allowances = load_allowances(root)?;
    let mut used = vec![false; allowances.len()];
    let roles = load_roles(root)?;
    let mut findings = Vec::new();

    for role in &roles {
        let package_root = root.join(&role.path);
        let src = package_root.join("src");
        let scan_root = if src.exists() { &src } else { &package_root };
        if !scan_root.exists() {
            continue;
        }
        let mut files = Vec::new();
        collect_rust_files(scan_root, &mut files)?;
        for file in files {
            let rel = file
                .strip_prefix(root)
                .map_err(|err| err.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let source =
                fs::read_to_string(&file).map_err(|err| format!("{}: {err}", file.display()))?;
            for (line_index, line) in source.lines().enumerate() {
                let Some(kind) = suspicious_line(line) else {
                    continue;
                };
                if !role.runtime && kind != "warning suppression" {
                    continue;
                }
                if let Some((index, allowance)) =
                    allowances.iter().enumerate().find(|(_, allowance)| {
                        allowance.path == rel && line.contains(&allowance.needle)
                    })
                {
                    used[index] = true;
                    println!(
                        "tracked audit debt: {}:{} [{}] {} (owner {}; remove by {})",
                        rel,
                        line_index + 1,
                        kind,
                        allowance.reason,
                        allowance.owner,
                        allowance.remove_by
                    );
                    continue;
                }
                findings.push(format!(
                    "{}:{}: [{}] {}",
                    rel,
                    line_index + 1,
                    kind,
                    line.trim()
                ));
            }
        }
    }

    for (index, allowance) in allowances.iter().enumerate() {
        if !used[index] {
            findings.push(format!(
                "stale audit allowance: {} contains {:?}",
                allowance.path, allowance.needle
            ));
        }
    }

    if findings.is_empty() {
        println!("source-structure audit passed");
        Ok(())
    } else {
        for finding in &findings {
            eprintln!("{finding}");
        }
        Err(format!(
            "source-structure audit found {} untracked issue(s)",
            findings.len()
        ))
    }
}

pub fn write_debt_report(root: &Path) -> Result<(), String> {
    let allowances = load_allowances(root)?;
    let mut by_milestone = BTreeMap::<String, Vec<&Allowance>>::new();
    for allowance in &allowances {
        by_milestone
            .entry(allowance.remove_by.clone())
            .or_default()
            .push(allowance);
    }
    let mut output = String::from(
        "# Tracked runtime audit debt\n\nThis file is generated from `config/audit-allowlist.csv`. Entries are temporary reviewed debt and are not completion claims.\n\n",
    );
    for (milestone, entries) in by_milestone {
        output.push_str(&format!(
            "## {milestone}\n\n| Path | Marker | Owner | Reason |\n| --- | --- | --- | --- |\n"
        ));
        for entry in entries {
            output.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} |\n",
                entry.path, entry.needle, entry.owner, entry.reason
            ));
        }
        output.push('\n');
    }
    let path = root.join("docs/generated/runtime-audit-debt.md");
    fs::create_dir_all(path.parent().expect("generated docs parent"))
        .map_err(|err| err.to_string())?;
    fs::write(&path, output).map_err(|err| format!("{}: {err}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

fn suspicious_line(line: &str) -> Option<&'static str> {
    let trimmed = line.trim();
    if trimmed.starts_with("//") || trimmed.starts_with("///") || trimmed.starts_with("//!") {
        return None;
    }
    if line.contains("host_model_enabled") || line.contains("runtime_model_supported") {
        return Some("target-model gate");
    }
    if trimmed.starts_with("#[allow(")
        || trimmed.starts_with("#![allow(")
        || trimmed.starts_with("#[expect(")
        || trimmed.starts_with("#![expect(")
        || (trimmed.starts_with("#[cfg_attr(")
            && (trimmed.contains("allow(") || trimmed.contains("expect(")))
    {
        return Some("warning suppression");
    }
    if line.contains("EVENT_QUEUE") {
        return Some("production-side in-memory device queue");
    }
    if line.contains("shadow_state")
        || line.contains("shadow-state")
        || line.contains("shadow state")
    {
        return Some("target shadow state");
    }
    if line.contains("FLASH_BYTES") {
        return Some("synthetic device storage");
    }
    if trimmed.starts_with("pub fn ") && trimmed.ends_with("{}") {
        return Some("empty public function");
    }
    None
}

fn load_allowances(root: &Path) -> Result<Vec<Allowance>, String> {
    let path = root.join("config/audit-allowlist.csv");
    let rows = read_csv(&path)?;
    let mut values = Vec::new();
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
        values.push(Allowance {
            path: row[0].clone(),
            needle: row[1].clone(),
            reason: row[2].clone(),
            owner: row[3].clone(),
            remove_by: row[4].clone(),
        });
    }
    Ok(values)
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
