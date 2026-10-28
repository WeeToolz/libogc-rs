use std::collections::BTreeSet;
use std::path::Path;

use crate::workspace::{read_csv, workspace_crates};

pub fn report(root: &Path) -> Result<(), String> {
    let path = root.join("config/static-review-scope.csv");
    let rows = read_csv(&path)?;
    let packages = workspace_crates(root)?;
    let mut seen = BTreeSet::new();
    let mut reviewed = 0usize;
    let mut parity_blocked = 0usize;
    let mut needs_re_review = 0usize;
    let mut review_incomplete = 0usize;
    let mut in_review = 0usize;
    let mut findings = 0usize;
    let mut fixes = 0usize;
    let mut re_reviews = 0usize;
    let mut doc_updates = 0usize;

    for (line, row) in rows.into_iter().enumerate().skip(1) {
        if row.iter().all(|field| field.trim().is_empty()) {
            continue;
        }
        if row.len() != 7 {
            return Err(format!(
                "{}:{}: expected 7 columns",
                path.display(),
                line + 1
            ));
        }
        let (package, status, owner, activity, note) =
            (&row[0], &row[1], &row[2], &row[3], &row[4]);
        if !seen.insert(package.clone()) {
            return Err(format!(
                "{}:{}: duplicate package {package}",
                path.display(),
                line + 1
            ));
        }
        if !packages.contains(package.as_str()) {
            return Err(format!(
                "{}:{}: unknown package {package}",
                path.display(),
                line + 1
            ));
        }
        if !note.is_empty() && !root.join(note).is_file() {
            return Err(format!(
                "{}:{}: missing review note {note}",
                path.display(),
                line + 1
            ));
        }
        match status.as_str() {
            "reviewed" => reviewed += 1,
            "parity_blocked" => parity_blocked += 1,
            "needs_re_review" => needs_re_review += 1,
            "review_incomplete" => review_incomplete += 1,
            _ => {
                return Err(format!(
                    "{}:{}: review_status must be reviewed, parity_blocked, needs_re_review, or review_incomplete",
                    path.display(),
                    line + 1
                ));
            }
        }
        match activity.as_str() {
            "idle" => {
                if !owner.is_empty() {
                    return Err(format!(
                        "{}:{}: idle row has an owner",
                        path.display(),
                        line + 1
                    ));
                }
            }
            "in_review" | "finding" | "fix_in_progress" | "re_review" | "doc_update" => {
                if owner.is_empty() {
                    return Err(format!(
                        "{}:{}: active row has no owner",
                        path.display(),
                        line + 1
                    ));
                }
                if note.is_empty() {
                    return Err(format!(
                        "{}:{}: active row has no review note",
                        path.display(),
                        line + 1
                    ));
                }
                match activity.as_str() {
                    "in_review" => in_review += 1,
                    "finding" => findings += 1,
                    "fix_in_progress" => fixes += 1,
                    "re_review" => re_reviews += 1,
                    "doc_update" => doc_updates += 1,
                    _ => unreachable!(),
                }
            }
            _ => {
                return Err(format!(
                    "{}:{}: invalid activity state",
                    path.display(),
                    line + 1
                ));
            }
        }
    }

    if seen != packages {
        let missing = packages
            .difference(&seen)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "review ledger is missing workspace crates: {missing}"
        ));
    }

    println!(
        "Static review ledger: {} crates; {reviewed} reviewed; {parity_blocked} parity blocked; {needs_re_review} need re-review; {review_incomplete} review incomplete; {in_review} in review; {findings} findings; {fixes} fixes; {re_reviews} re-reviews; {doc_updates} doc updates",
        reviewed + parity_blocked + needs_re_review + review_incomplete
    );
    Ok(())
}
