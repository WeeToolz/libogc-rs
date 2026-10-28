//! WAD behavior ledger validation. Reuse the workspace CSV reader.
use crate::workspace::read_csv;
use std::{collections::BTreeSet, fs, path::Path};
const REPORT: &str = "docs/generated/wad-parity.md";
const HEADER: &[&str] = &[
    "id",
    "behavior",
    "source",
    "rust",
    "implementation",
    "review",
    "verification",
    "evidence",
    "follow_up",
];
const STATES: &[&[&str]] = &[
    &["implemented", "partial", "out_of_scope"],
    &["source_reviewed", "unreviewed"],
    &[
        "host_pass",
        "reference_pass",
        "hardware_pending",
        "open",
        "out_of_scope",
    ],
];
fn reference(root: &Path, value: &str) -> Result<(), String> {
    let (file, symbol) = value
        .split_once('#')
        .ok_or_else(|| format!("reference needs path#symbol: {value}"))?;
    let content = fs::read_to_string(root.join(file)).map_err(|e| format!("{file}: {e}"))?;
    if symbol.is_empty() || !content.contains(symbol) {
        return Err(format!("missing source or evidence symbol: {value}"));
    }
    Ok(())
}
fn render(root: &Path) -> Result<String, String> {
    let rows = read_csv(&root.join("config/wad-parity.csv"))?;
    if rows
        .first()
        .map(|r| r.iter().map(String::as_str).collect::<Vec<_>>())
        != Some(HEADER.to_vec())
    {
        return Err("WAD ledger has an invalid header".into());
    }
    let mut seen = BTreeSet::new();
    let mut out = String::from(
        "# WAD behavior parity\n\nGenerated from `config/wad-parity.csv` with `cargo xtask wad-parity-refresh`.\n\nImplementation, source review, and verification are separate states. Host and emulator checks do not prove physical Wii behavior. See [scope and open capabilities](../review/libgc-wad.md).\n\n| ID | Behavior | Implementation | Source review | Verification | Follow-up |\n| --- | --- | --- | --- | --- | --- |\n",
    );
    for row in rows.iter().skip(1) {
        if row.len() != HEADER.len() || row.iter().any(|v| v.is_empty()) {
            return Err("WAD ledger requires nine nonempty fields per row".into());
        }
        if !row[0]
            .strip_prefix("WAD-")
            .is_some_and(|n| n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()))
            || !seen.insert(&row[0])
        {
            return Err(format!("invalid or duplicate WAD ID: {}", row[0]));
        }
        for (n, allowed) in STATES.iter().enumerate() {
            if !allowed.contains(&row[4 + n].as_str()) {
                return Err(format!(
                    "{}: invalid {} state: {}",
                    row[0],
                    HEADER[4 + n],
                    row[4 + n]
                ));
            }
        }
        if (row[4] != "implemented"
            || row[5] != "source_reviewed"
            || matches!(
                row[6].as_str(),
                "hardware_pending" | "open" | "out_of_scope"
            ))
            && row[8] == "none"
        {
            return Err(format!("{}: missing required follow-up", row[0]));
        }
        for n in [2, 3, 7] {
            reference(root, &row[n])?;
        }
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            row[0], row[1], row[4], row[5], row[6], row[8]
        ));
    }
    if seen.is_empty() {
        return Err("empty WAD ledger".into());
    }
    out.push_str(
        "\nSource, Rust, and evidence references for each contract are in the CSV ledger.\n",
    );
    Ok(out)
}
pub fn run(root: &Path, refresh: bool) -> Result<(), String> {
    let expected = render(root)?;
    let path = root.join(REPORT);
    if refresh {
        fs::write(&path, &expected).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let actual = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    if actual != expected {
        return Err("WAD parity report is stale; run wad-parity-refresh".into());
    }
    println!("WAD ledger references, states, follow-up, and generated report pass");
    Ok(())
}
