use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path};

use crate::workspace::read_csv;

const COLUMNS: [&str; 12] = [
    "area",
    "api_or_group",
    "expected_behavior",
    "probe",
    "build_or_test_command",
    "requirements",
    "platforms",
    "status",
    "run_id",
    "build_id",
    "evidence",
    "notes",
];

pub fn audit(root: &Path) -> Result<(), String> {
    let path = root.join("docs/hardware-api-validation.csv");
    let rows = read_csv(&path)?;
    let Some(header) = rows.first() else {
        return Err(format!("{} is empty", path.display()));
    };
    if header.iter().map(String::as_str).ne(COLUMNS) {
        return Err(format!(
            "{} has an invalid header; expected {}",
            path.display(),
            COLUMNS.join(",")
        ));
    }

    let mut statuses = BTreeMap::<String, usize>::new();
    for (line, row) in rows.iter().enumerate().skip(1) {
        let line_number = line + 1;
        if row.len() != COLUMNS.len() {
            return Err(format!(
                "{}:{line_number}: expected {} fields, found {}",
                path.display(),
                COLUMNS.len(),
                row.len()
            ));
        }
        let value = |column: &str| -> &str {
            let index = COLUMNS.iter().position(|name| *name == column).unwrap_or(0);
            row[index].trim()
        };

        for column in [
            "area",
            "api_or_group",
            "expected_behavior",
            "probe",
            "build_or_test_command",
            "requirements",
            "platforms",
            "evidence",
            "notes",
        ] {
            if value(column).is_empty() {
                return Err(format!(
                    "{}:{line_number}: {column} must not be empty",
                    path.display()
                ));
            }
        }

        let status = value("status");
        if !matches!(status, "passed" | "failed" | "skipped" | "not-tested") {
            return Err(format!(
                "{}:{line_number}: invalid status {status:?}",
                path.display()
            ));
        }
        *statuses.entry(status.to_string()).or_default() += 1;

        let evidence = value("evidence");
        let relative_evidence = Path::new(evidence);
        if relative_evidence.is_absolute()
            || relative_evidence
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(format!(
                "{}:{line_number}: evidence path must stay inside the repository",
                path.display()
            ));
        }
        let evidence_path = root.join(relative_evidence);
        if !evidence_path.is_file() {
            return Err(format!(
                "{}:{line_number}: evidence file does not exist: {evidence}",
                path.display()
            ));
        }

        if matches!(status, "passed" | "failed") {
            let run_id = value("run_id");
            let build_id = value("build_id");
            if run_id.is_empty() {
                return Err(format!(
                    "{}:{line_number}: {status} result requires a run_id",
                    path.display()
                ));
            }
            if build_id.len() != 64 || !build_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!(
                    "{}:{line_number}: {status} result requires a 64-digit DOL SHA-256 build_id",
                    path.display()
                ));
            }
            let json: serde_json::Value = serde_json::from_slice(
                &fs::read(&evidence_path)
                    .map_err(|err| format!("{}: {err}", evidence_path.display()))?,
            )
            .map_err(|err| format!("{}: {err}", evidence_path.display()))?;
            if json.get("run_id").and_then(serde_json::Value::as_str) != Some(run_id) {
                return Err(format!(
                    "{}:{line_number}: evidence run_id does not match {run_id}",
                    path.display()
                ));
            }
            if json.get("dol_sha256").and_then(serde_json::Value::as_str) != Some(build_id) {
                return Err(format!(
                    "{}:{line_number}: evidence DOL SHA-256 does not match build_id",
                    path.display()
                ));
            }
        }
    }

    let total = rows.len().saturating_sub(1);
    let summary = statuses
        .iter()
        .map(|(status, count)| format!("{status}={count}"))
        .collect::<Vec<_>>()
        .join(" ");
    println!("hardware API register: {total} rows valid ({summary})");
    Ok(())
}
