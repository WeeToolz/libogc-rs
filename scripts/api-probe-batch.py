#!/usr/bin/env python3
"""Plan, run, and record one Dolphin API evidence batch."""

import argparse
from collections import Counter
import csv
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
from urllib.parse import quote


ROOT = Path(__file__).resolve().parents[1]
INVENTORY = Path("docs/generated/implementation-inventory.csv")
EVIDENCE = Path("config/implementation-evidence.csv")
GENERATED = (
    INVENTORY,
    Path("docs/generated/implementation-status.md"),
    Path("docs/generated/runtime-audit-debt.md"),
)
KEYS = ("crate", "target_implementation_file", "symbol", "line")


def command(*args, **kwargs):
    return subprocess.run(args, cwd=ROOT, **kwargs)


def git(*args):
    return subprocess.check_output(("git", *args), cwd=ROOT)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def tree_identity(root):
    state = hashlib.sha256()
    files = sorted(path for path in root.rglob("*") if path.is_file())
    for path in files:
        name = path.relative_to(root).as_posix().encode()
        state.update(len(name).to_bytes(8, "big") + name + bytes.fromhex(digest(path)))
    return {"path": str(root), "files": len(files), "sha256": state.hexdigest()}


def read_inventory():
    with (ROOT / INVENTORY).open(newline="") as stream:
        return list(csv.DictReader(stream))


def json_file(path):
    return json.loads(path.read_text())


def save_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def fingerprint(artifact):
    """Bind the run to HEAD and tracked or untracked work outside its artifacts."""
    exclude = f":(exclude,top){artifact.as_posix()}/**"
    state = hashlib.sha256(git("rev-parse", "HEAD"))
    state.update(git("diff", "--binary", "--no-ext-diff", "HEAD", "--", ".", exclude))
    for name in sorted(git("ls-files", "-z", "--others", "--exclude-standard",
                           "--", ".", exclude).split(b"\0")):
        if not name:
            continue
        path = ROOT / name.decode()
        content = path.readlink().as_posix() if path.is_symlink() else digest(path)
        state.update(name + b"\0" + content.encode() + b"\0")
    return state.hexdigest()


def plan(args):
    rows = [r for r in read_inventory() if r["implementation_class"] == "unverified"]
    if args.crate:
        for row in rows:
            if row["crate"] == args.crate:
                print(*(row[key] for key in KEYS), row["probe_coverage"], sep="\t")
    else:
        groups = Counter((r["crate"], r["probe_coverage"]) for r in rows)
        for (crate, probe), count in groups.most_common(args.limit):
            print(count, crate, probe, sep="\t")


def crate_relative_path(row):
    source = Path(row["target_implementation_file"])
    for parent in source.parents:
        cargo = ROOT / parent / "Cargo.toml"
        if cargo.is_file():
            package = tomllib.loads(cargo.read_text()).get("package", {})
            if package.get("name") == row["crate"]:
                return source.relative_to(parent).as_posix()
    raise ValueError(f"No Cargo package found for {row['target_implementation_file']}")


def manifest_draft(args, rows):
    cases = []
    for selector in args.symbols:
        symbol, sep, line = selector.partition("@")
        if sep and (not line.isdecimal() or int(line) < 1):
            raise ValueError(f"Expected SYMBOL or SYMBOL@LINE: {selector}")
        matches = [
            row for row in rows
            if row["crate"] == args.crate and row["symbol"] == symbol
            and row["implementation_class"] == "unverified"
            and (not sep or row["line"] == line)
            and (not args.source or row["target_implementation_file"] == args.source)
        ]
        if len(matches) != 1:
            locations = ", ".join(
                f"{row['target_implementation_file']}:{row['line']}" for row in matches
            )
            raise ValueError(
                f"{args.crate}::{selector} requires one unverified entry. "
                f"Matches: {locations or 'none'}. Use --source or SYMBOL@LINE."
            )
        row, = matches
        cases.append({
            "crate": row["crate"],
            "path": crate_relative_path(row),
            "symbol": row["symbol"],
            "line": int(row["line"]),
            "evidence_path": args.evidence_path,
            "function": args.function,
            "api_path": f"{args.api_prefix}::{symbol}" if args.api_prefix else symbol,
            "checks": [],
        })
    manifest = {"probe": args.probe, "cases": cases}
    select_cases(manifest, rows)
    return manifest


def draft(args):
    print(json.dumps(manifest_draft(args, read_inventory()), indent=2))


def select_cases(manifest, rows, *, recheck=False):
    selected = []
    seen = set()
    for case in manifest["cases"]:
        matches = [
            row for row in rows
            if row["crate"] == case["crate"]
            and row["symbol"] == case["symbol"]
            and row["target_implementation_file"].endswith("/" + case["path"])
            and ("line" not in case or row["line"] == str(case["line"]))
        ]
        if len(matches) != 1:
            raise ValueError(f"Select one inventory entry for {case['crate']}::{case['symbol']}")
        row = matches[0]
        key = tuple(row[k] for k in KEYS)
        if key in seen:
            raise ValueError(f"Duplicate entry: {key}")
        if (row["implementation_class"] != "unverified") != recheck:
            raise ValueError(f"Entry does not match the requested verification mode: {key}")
        seen.add(key)
        selected.append((case, row))
    if not selected:
        raise ValueError("The manifest requires at least one case")
    return selected


def artifact_path(path):
    base = Path("docs/dolphin-runs")
    absolute = (ROOT / path).resolve()
    relative = absolute.relative_to(ROOT / base)
    if not relative.parts:
        raise ValueError("Use a batch directory inside docs/dolphin-runs")
    return base / relative


def validate_checks(manifest):
    for case in manifest["cases"]:
        checks = case["checks"]
        if (not isinstance(checks, list) or not checks or
                any(not isinstance(label, str) or not label.strip() or
                    label.splitlines() != [label] for label in checks)):
            raise ValueError(f"{case['symbol']} requires at least one single-line check label")


def result_for(directory, probe):
    results = [r for r in json_file(directory / "suite.json")["results"] if r["probe"] == probe]
    if len(results) != 1:
        raise ValueError(f"Expected one result for {probe} in {directory / 'suite.json'}")
    return results[0]


def read_checks(result):
    screenshots = {Path(name).name: ROOT / name for name in result.get("page_frames", [])}
    pages = {}
    for marker in result.get("page_capture_markers", []):
        capture = re.search(r"\bpage=(\d+) capture=(\d+)\b", marker)
        if capture:
            page, index = map(int, capture.groups())
            screenshot = screenshots.get(f"page-{index:03d}.png")
            if screenshot and screenshot.is_file():
                pages[page] = screenshot

    checks = {}
    page = 1
    log = ROOT / result["probe_log"]
    if not log.is_file():
        return checks
    for number, line in enumerate(log.read_text().splitlines(), 1):
        if line == "FAILED CHECKS (retained from all pages)":
            break
        marker = re.fullmatch(r"-- page (\d+) --", line)
        if marker:
            page = int(marker[1])
        check = re.fullmatch(r"(PASS|FAIL) (.+)", line)
        if check and not check[2].startswith("summary passes="):
            checks.setdefault(check[2], []).append((check[1], number, pages.get(page)))
    return checks


def markdown_text(value):
    return value.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("|", "&#124;")


def report_link(directory, path, label, anchor=""):
    relative = os.path.relpath(path, directory)
    return f"[{label}]({quote(relative)}{anchor})"


def evidence_report(directory, manifest, previous=None):
    result = result_for(directory, manifest["probe"])
    checks = read_checks(result)
    log = ROOT / result["probe_log"]
    lines = [
        "# Dolphin evidence report",
        "",
        f"Probe `{manifest['probe']}` reported `{result['status']}`.",
        f"Passed checks {result['passes']}. Failed checks {result['fails']}.",
        "",
        "| API | Required check | Result | Evidence |",
        "| --- | --- | --- | --- |",
    ]
    gaps = []
    selected = set()
    for case in manifest["cases"]:
        api = markdown_text(f"{case['crate']}::{case['symbol']}")
        for label in case["checks"] or [""]:
            selected.add(label)
            hits = checks.get(label, [])
            status = "FAIL" if any(hit[0] == "FAIL" for hit in hits) else "PASS" if hits else "Missing"
            links = []
            for _, number, screenshot in hits:
                links.append(report_link(directory, log, f"log L{number}", f"#L{number}"))
                if screenshot:
                    links.append(report_link(directory, screenshot, screenshot.stem))
            if status != "PASS":
                gaps.append(f"- {api} — {markdown_text(label) if label else 'No required check supplied'} ({status})")
            lines.append(
                f"| {api} | {markdown_text(label)} | {status} | {', '.join(links)} |"
            )

    lines.extend(["", "## Evidence gaps", ""])
    if result["status"] != "pass":
        lines.append(f"- The probe did not pass. See {report_link(directory, directory / 'suite.json', 'suite result')}.")
    if not result["pages_valid"]:
        lines.append("- The runner did not confirm complete page captures.")
    for label, hits in checks.items():
        if label not in selected and any(hit[0] == "FAIL" for hit in hits):
            gaps.append(f"- Other failed check — {markdown_text(label)}")
    lines.extend(gaps or ["- All listed check labels passed. Review the behavior and coverage separately."])
    if manifest.get("limits"):
        lines.extend(["", "## Coverage limits", ""])
        lines.extend(f"- {markdown_text(limit)}" for limit in manifest["limits"])

    if previous:
        old = result_for(previous, manifest["probe"])
        old_checks = read_checks(old)
        lines.extend([
            "", "## Previous run", "",
            report_link(directory, previous / "suite.json", "Previous suite result"),
            "",
            "| Check | Previous | Current |",
            "| --- | --- | --- |",
        ])
        changed = False
        for label in sorted(checks.keys() | old_checks.keys()):
            old_counts = Counter(hit[0] for hit in old_checks.get(label, []))
            counts = Counter(hit[0] for hit in checks.get(label, []))
            if old_counts != counts:
                lines.append(
                    f"| {markdown_text(label)} | {old_counts['PASS']} pass, {old_counts['FAIL']} fail "
                    f"| {counts['PASS']} pass, {counts['FAIL']} fail |"
                )
                changed = True
        if not changed:
            lines.append("| Check counts unchanged | | |")
    return "\n".join(lines) + "\n"


def report(args):
    directory = ROOT / artifact_path(args.artifact)
    previous = ROOT / artifact_path(args.compare) if args.compare else None
    manifest = json_file(args.manifest or directory / "batch.json")
    print(evidence_report(directory, manifest, previous), end="")


def run(args):
    artifact = artifact_path(args.artifact)
    manifest = json_file(args.manifest)
    select_cases(manifest, read_inventory(), recheck=args.recheck)
    validate_checks(manifest)
    previous = ROOT / artifact_path(args.compare) if args.compare else None
    if previous:
        result_for(previous, manifest["probe"])
    directory = ROOT / artifact
    directory.mkdir(parents=True, exist_ok=False)
    save_json(directory / "batch.json", manifest)
    before = fingerprint(artifact)
    dolphin = args.dolphin.resolve()
    options = ["cargo", "xtask", "dolphin-validate", "--probe", manifest["probe"],
               "--dolphin", str(dolphin), "--artifact-dir", str(artifact),
               "--timeout-seconds", str(args.timeout)]
    if args.nand_dir:
        options.extend(["--nand-dir", str(args.nand_dir.resolve())])
    fixtures = {
        key: {"path": os.environ[key], "sha256": digest(Path(os.environ[key]).resolve())}
        for key in ("LIBOGC_MAD_REFERENCE_MP3", "LIBOGC_MAD_PROBE_MP3", "LIBOGC_DOLPHIN_CARD_IMAGE")
        if os.environ.get(key)
    }
    rc = command(*options).returncode
    suite_path = directory / "suite.json"
    if not suite_path.is_file():
        raise ValueError(f"Dolphin produced no suite report (exit {rc})")
    suite = json_file(suite_path)
    result, = suite["results"]
    dolphin_log = ROOT / result["user_directory"] / "Logs/dolphin.log"
    if dolphin_log.is_file():
        shutil.copyfile(dolphin_log, directory / manifest["probe"] / "dolphin.log")
    snapshots = {}
    for relative in (
        result["probe_log"], result["stdout_log"], result["stderr_log"],
        result["probe_transport_bytes"], *result["page_frames"], *result["frames"],
    ):
        if not relative:
            continue
        path = ROOT / relative
        path.resolve().relative_to(directory.resolve())
        if path.is_file() or not rc:
            snapshots[relative] = digest(path)
    run_info = {
        "source_revision": git("rev-parse", "HEAD").decode().strip(),
        "source_fingerprint": before,
        "command": options,
        "dolphin_sha256": digest(dolphin),
        "dol_sha256": result["dol_sha256"],
        "rustc": subprocess.check_output(("rustc", "-vV"), cwd=ROOT, text=True),
        "external_fixtures_before_run": fixtures,
        "nand_source_after_run": tree_identity(ROOT / suite["nand_source"]),
        "manifest_sha256": digest(directory / "batch.json"),
        "suite_sha256": digest(suite_path),
        "evidence_sha256": snapshots,
        "exit_code": rc,
    }
    save_json(directory / "batch-run.json", run_info)
    (directory / "batch-report.md").write_text(evidence_report(directory, manifest, previous))
    if fingerprint(artifact) != before:
        raise ValueError("Worktree changed during the run. Start a new batch directory")
    if rc:
        raise ValueError(f"Dolphin failed. See {suite_path}")
    verify_run(directory, manifest, run_info)
    print(f"Review {artifact / 'batch-report.md'} before finishing.")


def verify_run(directory, manifest, run_info):
    suite = json_file(directory / "suite.json")
    result, = suite["results"]
    if (run_info["exit_code"] or not suite["passed"] or result["status"] != "pass"
            or result["probe"] != manifest["probe"] or result["fails"] != 0
            or result["dol_sha256"] != run_info["dol_sha256"]):
        raise ValueError("A passing run for this probe is required")
    validate_checks(manifest)
    checks = read_checks(result)
    for case in manifest["cases"]:
        missing = [
            label for label in case["checks"]
            if {hit[0] for hit in checks.get(label, [])} != {"PASS"}
        ]
        if missing:
            raise ValueError(f"{case['symbol']} did not pass its checks: {missing}")
    return result


def audit(directory):
    report_path = directory / "completeness-audit.json"
    report_path.unlink(missing_ok=True)
    with (directory / "completeness-audit.log").open("w") as log:
        result = command(
            "cargo", "xtask", "audit-stubs", "--refresh-inventory", "--json", str(report_path),
            stdout=log, stderr=subprocess.STDOUT,
        )
    if not report_path.is_file():
        raise ValueError("Audit failed. See completeness-audit.log")
    report = json_file(report_path)
    if report["schema_version"] != 1:
        raise ValueError("Unsupported completeness audit schema")
    count = report["remaining_count"]
    if count != len(report["entries"]) or result.returncode != int(count != 0):
        raise ValueError("Audit result disagrees with the exit status. See completeness-audit.log")
    if count:
        print(f"Completeness audit reports {count} remaining entries.")
    return report


def finish(args):
    artifact = artifact_path(args.artifact)
    directory = ROOT / artifact
    manifest = json_file(directory / "batch.json")
    run_info = json_file(directory / "batch-run.json")
    if fingerprint(artifact) != run_info["source_fingerprint"]:
        raise ValueError("Worktree changed since the run. Start a new batch directory")
    for name, expected in (
        ("batch.json", run_info["manifest_sha256"]),
        ("suite.json", run_info["suite_sha256"]),
    ):
        if digest(directory / name) != expected:
            raise ValueError(f"{name} changed since the run")
    for name, expected in run_info["evidence_sha256"].items():
        if digest(ROOT / name) != expected:
            raise ValueError(f"{name} changed since the run")
    result = verify_run(directory, manifest, run_info)
    rows = read_inventory()
    selected = select_cases(manifest, rows)
    uncovered = sum(row["implementation_class"] == "unverified" for row in rows)
    saved = {path: (ROOT / path).read_bytes() for path in (EVIDENCE, *GENERATED)}
    try:
        with (ROOT / EVIDENCE).open("a", newline="") as stream:
            writer = csv.writer(stream, lineterminator="\n")
            for case, row in selected:
                writer.writerow([
                    row["crate"], case["path"], row["symbol"], row["line"],
                    row["declared_classification"], "hardware-probe",
                    case["evidence_path"], case["function"], case["api_path"],
                ])
        command("cargo", "fmt", "--all", "--check", check=True)
        command("cargo", "clippy", "--all-targets", check=True)
        command("cargo", "test", check=True)
        command("cargo", "test", "-p", "xtask", "-p", "ogc-controller", check=True)
        report = audit(directory)
        updated = read_inventory()
        by_key = {tuple(row[k] for k in KEYS): row for row in updated}
        for case, previous in selected:
            row = by_key[tuple(previous[k] for k in KEYS)]
            expected = f"hardware-probe:{case['evidence_path']}::{case['function']} "
            if row["implementation_class"] == "unverified" or not row["verification_evidence"].startswith(expected):
                raise ValueError(f"Inventory did not accept evidence for {case['symbol']}")
        remaining = report["unverified_count"]
        if uncovered - remaining < len(selected):
            raise ValueError("Inventory delta is smaller than the selected cases")
    except (Exception, KeyboardInterrupt):
        for path, content in saved.items():
            (ROOT / path).write_bytes(content)
        raise
    save_json(directory / "batch-summary.json", {
        "probe_passes": result["passes"],
        "unverified_before": uncovered,
        "unverified_after": remaining,
        "cases": len(selected),
        "remaining_audit_entries": report["remaining_count"],
    })
    print(f"Verified {len(selected)} entries. Unverified: {uncovered} -> {remaining}.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(required=True)
    choice = actions.add_parser("plan")
    choice.add_argument("--crate")
    choice.add_argument("--limit", type=int, default=20)
    choice.set_defaults(action=plan)
    choice = actions.add_parser("manifest", help="Draft a manifest for selected unverified APIs")
    choice.add_argument("--crate", required=True)
    choice.add_argument("--probe", required=True)
    choice.add_argument("--source", help="Repository-relative API source file for ambiguous symbols")
    choice.add_argument("--evidence-path", required=True)
    choice.add_argument("--function", required=True)
    choice.add_argument("--api-prefix", default="")
    choice.add_argument("symbols", nargs="+", metavar="SYMBOL[@LINE]")
    choice.set_defaults(action=draft)
    choice = actions.add_parser("report", help="Print an evidence report from a saved run")
    choice.add_argument("artifact", type=Path)
    choice.add_argument("--manifest", type=Path, help="Manifest for a run without batch.json")
    choice.add_argument("--compare", type=Path)
    choice.set_defaults(action=report)
    choice = actions.add_parser("run")
    choice.add_argument("manifest", type=Path)
    choice.add_argument("artifact", type=Path)
    choice.add_argument("--dolphin", required=True, type=Path)
    choice.add_argument("--nand-dir", type=Path)
    choice.add_argument("--compare", type=Path)
    choice.add_argument("--timeout", type=int, default=180)
    choice.add_argument("--recheck", action="store_true",
                        help="Run checks for APIs already recorded in the inventory")
    choice.set_defaults(action=run)
    choice = actions.add_parser("finish")
    choice.add_argument("artifact", type=Path)
    choice.set_defaults(action=finish)
    args = parser.parse_args()
    try:
        args.action(args)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        sys.exit(str(error))


if __name__ == "__main__":
    main()
