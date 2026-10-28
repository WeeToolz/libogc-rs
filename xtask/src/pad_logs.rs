use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct PadCoverageReport {
    schema_version: u32,
    status: String,
    logs: Vec<String>,
    covered_scenarios: Vec<String>,
    missing_scenarios: Vec<String>,
    errors: Vec<String>,
}

pub fn validate(path: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    collect_logs(path, &mut files)?;
    let expected = expected_scenarios();
    let mut covered = BTreeSet::new();
    let mut errors = Vec::new();
    let mut retained = Vec::new();
    let mut corpus = String::new();

    for file in files {
        let text = fs::read_to_string(&file).map_err(|err| format!("{}: {err}", file.display()))?;
        if !text.contains("pad-recalibration-v1")
            && !text.contains("PAD guided")
            && !text.contains("PAD LWP stress")
            && !text.contains("PAD system-reset")
        {
            continue;
        }
        retained.push(file.display().to_string());
        corpus.push_str(&text);
        corpus.push('\n');
        if text.lines().any(|line| line.starts_with("FAIL ")) {
            errors.push(format!("{} contains a FAIL record", file.display()));
        }
        if !text
            .lines()
            .any(|line| line.starts_with("PASS summary passes=") && line.ends_with("fails=0"))
        {
            errors.push(format!("{} has no zero-failure summary", file.display()));
        }
        if !text.contains("PAD trace hook received events") {
            errors.push(format!("{} has no PAD hook evidence", file.display()));
        }
        if !text.contains("pad trace dropped=0") {
            errors.push(format!(
                "{} reports missing or dropped PAD trace state",
                file.display()
            ));
        }
        if !text.contains("CHECKPOINT ") {
            errors.push(format!("{} has no durable checkpoint", file.display()));
        }

        let mut begins = BTreeMap::<String, usize>::new();
        let mut ends = BTreeMap::<String, usize>::new();
        for line in text.lines() {
            if let Some(id) = line.strip_prefix("INFO BEGIN ") {
                *begins.entry(id.to_string()).or_default() += 1;
                if expected.contains(id) {
                    covered.insert(id.to_string());
                }
            }
            if let Some(id) = line.strip_prefix("INFO END ") {
                *ends.entry(id.to_string()).or_default() += 1;
            }
        }
        for (id, count) in begins {
            if ends.get(&id).copied().unwrap_or(0) != count {
                errors.push(format!(
                    "{} has unmatched BEGIN/END records for {id}",
                    file.display()
                ));
            }
        }

        for id in expected.iter() {
            if text.lines().any(|line| line == format!("PASS {id}")) {
                covered.insert((*id).to_string());
            }
        }
    }

    let missing = expected
        .difference(&covered.iter().map(String::as_str).collect())
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
    if retained.is_empty() {
        errors.push("no PAD recalibration logs were found".to_string());
    }
    for specification in [0, 1, 2, 4, 5] {
        let marker = format!("schema=pad-recalibration-v1 spec={specification}");
        if !corpus.contains(&marker) {
            errors.push(format!("missing PAD specification {specification} log"));
        }
    }
    for (family, marker) in [
        (
            "deterministic state",
            "environment=deterministic-si-boundary",
        ),
        ("wired hardware", "PAD guided wired hardware probe"),
        ("WaveBird hardware", "PAD guided WaveBird probe"),
        ("concurrent stress", "PAD LWP stress"),
        ("system reset", "PAD system-reset"),
    ] {
        if !corpus.contains(marker) {
            errors.push(format!("missing {family} probe log"));
        }
    }
    let status = if errors.is_empty() && missing.is_empty() {
        "pass"
    } else {
        "incomplete"
    };
    let report = PadCoverageReport {
        schema_version: 1,
        status: status.to_string(),
        logs: retained,
        covered_scenarios: covered.into_iter().collect(),
        missing_scenarios: missing,
        errors,
    };
    let output = path.join("pad-coverage.json");
    let json = serde_json::to_vec_pretty(&report).map_err(|err| err.to_string())?;
    fs::write(&output, json).map_err(|err| format!("{}: {err}", output.display()))?;
    println!("wrote {}", output.display());
    if report.status == "pass" {
        Ok(())
    } else {
        Err(format!(
            "PAD coverage is incomplete: {} missing scenarios, {} log errors",
            report.missing_scenarios.len(),
            report.errors.len()
        ))
    }
}

fn collect_logs(path: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "log") {
            output.push(path.to_path_buf());
        }
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|err| format!("{}: {err}", path.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let child = entry.path();
        if child.is_dir() {
            collect_logs(&child, output)?;
        } else if child
            .extension()
            .is_some_and(|extension| extension == "log")
        {
            output.push(child);
        }
    }
    Ok(())
}

fn expected_scenarios() -> BTreeSet<&'static str> {
    let mut expected = BTreeSet::from([
        "mask-channel-0",
        "mask-channel-1",
        "mask-channel-2",
        "mask-channel-3",
        "mask-all",
        "mask-mixed-0-2",
        "invalid-mask-filter",
        "return-busy-read-lifecycle",
        "active-reset-queue",
        "transfer-submission-failure-recovery",
        "non-controller-keyboard",
        "non-controller-steering",
        "no-controller",
        "waiting-checking-exclusion",
        "legacy-origin-transfer-selection",
        "calibration-command-42",
        "full-origin-payload-transform",
        "main-stick-x-patch",
        "recalibration-disabled-low-memory",
        "wired-full-analog-origin",
        "wired-multi-channel-isolation",
        "wired-polling-isolation",
        "unselected-controller-and-rumble",
        "wired-x-patch-hardware",
        "wired-empty-socket",
        "unplug-during-type-query",
        "unplug-during-calibration-transfer",
        "wavebird-connected-command-path",
        "wavebird-receiver-only-wait",
        "wavebird-disconnect-reconnect",
        "recalibration accepted during discovery",
        "four PAD stress threads created",
        "stress completed without stale PAD state",
        "reset queue retried and finalized PAD",
    ]);
    for channel in 0..4 {
        expected.insert(match channel {
            0 => "wired-channel-0-recalibrate",
            1 => "wired-channel-1-recalibrate",
            2 => "wired-channel-2-recalibrate",
            _ => "wired-channel-3-recalibrate",
        });
    }
    for index in 0..6 {
        expected.insert(match index {
            0 => "si-type-error-0",
            1 => "si-type-error-1",
            2 => "si-type-error-2",
            3 => "si-type-error-3",
            4 => "si-type-error-4",
            _ => "si-type-error-5",
        });
        expected.insert(match index {
            0 => "si-origin-error-0",
            1 => "si-origin-error-1",
            2 => "si-origin-error-2",
            3 => "si-origin-error-3",
            4 => "si-origin-error-4",
            _ => "si-origin-error-5",
        });
    }
    expected
}
