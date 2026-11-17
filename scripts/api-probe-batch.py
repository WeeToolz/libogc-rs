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


def select_cases(manifest, rows):
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
        if key in seen or row["implementation_class"] != "unverified":
            raise ValueError(f"Duplicate or already verified entry: {key}")
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


def run(args):
    artifact = artifact_path(args.artifact)
    manifest = json_file(args.manifest)
    select_cases(manifest, read_inventory())
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
        path = ROOT / relative
        path.resolve().relative_to(directory.resolve())
        snapshots[relative] = digest(path)
    save_json(directory / "batch-run.json", {
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
    })
    if fingerprint(artifact) != before:
        raise ValueError("Worktree changed during the run. Start a new batch directory")
    if rc:
        raise ValueError(f"Dolphin failed. See {suite_path}")
    print(f"Review {result['probe_log']} and its screenshots before finishing.")


def verify_run(directory, manifest, run_info):
    suite = json_file(directory / "suite.json")
    result, = suite["results"]
    if (run_info["exit_code"] or not suite["passed"] or result["status"] != "pass"
            or result["probe"] != manifest["probe"] or result["fails"] != 0
            or result["dol_sha256"] != run_info["dol_sha256"]):
        raise ValueError("A passing run for this probe is required")
    lines = set((ROOT / result["probe_log"]).read_text().splitlines())
    for case in manifest["cases"]:
        if not case["checks"]:
            raise ValueError(f"{case['symbol']} requires at least one check label")
        missing = [label for label in case["checks"] if f"PASS {label}" not in lines]
        if missing:
            raise ValueError(f"{case['symbol']} did not pass its checks: {missing}")
    return result


def audit(directory):
    with (directory / "completeness-audit.log").open("w") as log:
        result = command("cargo", "xtask", "audit-stubs", stdout=log, stderr=subprocess.STDOUT)
    output = (directory / "completeness-audit.log").read_text()
    if result.returncode:
        match = re.search(r'Error: "runtime completeness audit found (\d+) disallowed API implementation\(s\)"\s*$', output)
        if result.returncode != 1 or not match:
            raise ValueError("Audit failed. See completeness-audit.log")
        print(f"Completeness audit reports {match[1]} remaining entries.")


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
        command("cargo", "xtask", "inventory", check=True)
        updated = read_inventory()
        by_key = {tuple(row[k] for k in KEYS): row for row in updated}
        for case, previous in selected:
            row = by_key[tuple(previous[k] for k in KEYS)]
            expected = f"hardware-probe:{case['evidence_path']}::{case['function']} "
            if row["implementation_class"] == "unverified" or not row["verification_evidence"].startswith(expected):
                raise ValueError(f"Inventory did not accept evidence for {case['symbol']}")
        remaining = sum(row["implementation_class"] == "unverified" for row in updated)
        if uncovered - remaining != len(selected):
            raise ValueError("Inventory delta differs from the selected cases")
        command("cargo", "fmt", "--all", "--check", check=True)
        command("cargo", "clippy", "--all-targets", check=True)
        command("cargo", "test", check=True)
        audit(directory)
    except (Exception, KeyboardInterrupt):
        for path, content in saved.items():
            (ROOT / path).write_bytes(content)
        raise
    save_json(directory / "batch-summary.json", {
        "probe_passes": result["passes"],
        "unverified_before": uncovered,
        "unverified_after": remaining,
        "cases": len(selected),
    })
    print(f"Verified {len(selected)} entries. Unverified: {uncovered} -> {remaining}.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(required=True)
    choice = actions.add_parser("plan")
    choice.add_argument("--crate")
    choice.add_argument("--limit", type=int, default=20)
    choice.set_defaults(action=plan)
    choice = actions.add_parser("run")
    choice.add_argument("manifest", type=Path)
    choice.add_argument("artifact", type=Path)
    choice.add_argument("--dolphin", required=True, type=Path)
    choice.add_argument("--nand-dir", type=Path)
    choice.add_argument("--timeout", type=int, default=180)
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
