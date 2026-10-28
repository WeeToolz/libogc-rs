#!/usr/bin/env python3
"""Build one std readiness DOL and package its HBC loader settings."""
import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]


def metadata(offset, endpoint, mode, console_model=None, motherboard=None, media="both", focus="all"):
    if not -86399 <= offset <= 86399:
        raise ValueError("UTC offset must be in -86399..86399 seconds")
    if mode not in ("normal", "panic", "entropy"):
        raise ValueError("mode must be normal, panic, or entropy")
    if focus not in ("all", "network", "storage", "usb"):
        raise ValueError("invalid focus")
    if media not in ("sd", "usb", "both"):
        raise ValueError("media must be sd, usb, or both")
    host, separator, port = endpoint.rpartition(":")
    if not host or not separator or not port.isdecimal() or not 1 <= int(port) <= 65535:
        raise ValueError("network endpoint must be host:port")
    root = ET.Element("app", version="1")
    for name, value in (
        ("name", "Wii Rust std readiness"),
        ("coder", "libogc-rs"),
        ("version", "1"),
        ("short_description", "SD, USB, threads, clocks, networking and std contracts"),
        ("long_description", "Runs shared production std acceptance suites. Writes exclusive probe files on SD and USB. Missing prerequisites are failures. RESET changes final failure pages. Panic and entropy modes are terminal tests. Real media failure and power loss require separate runs."),
    ):
        ET.SubElement(root, name).text = value
    arguments = ET.SubElement(root, "arguments")
    for value in (f"--mode={mode}", f"--utc-offset-seconds={offset}", f"--network-endpoint={endpoint}", f"--media={media}", f"--focus={focus}"):
        ET.SubElement(arguments, "arg").text = value
    for name, value in (("console-model", console_model), ("motherboard", motherboard)):
        if value is not None:
            if not value or len(value.encode("utf-8")) > 40 or any(not 32 <= ord(ch) <= 126 for ch in value):
                raise ValueError("hardware labels must contain 1..40 printable bytes")
            ET.SubElement(arguments, "arg").text = f"--{name}={value}"
    ET.indent(root)
    return ET.tostring(root, encoding="utf-8", xml_declaration=True) + b"\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--utc-offset-seconds", type=int, help="Wii local clock minus UTC; defaults to the build computer's current offset")
    parser.add_argument("--focus", choices=("all", "network", "storage", "usb"), default="all")
    parser.add_argument("--media", choices=("sd", "usb", "both"), default="both")
    parser.add_argument("--network-endpoint", default="example.com:80")
    parser.add_argument("--mode", choices=("normal", "panic", "entropy"), default="normal")
    parser.add_argument("--console-model", help="Printed console model, shown as a supplied label")
    parser.add_argument("--motherboard", help="Printed board revision, shown as a supplied label")
    parser.add_argument("--agent-report", action="store_true", help="Enable the existing Wiiload report and framebuffer transport")
    parser.add_argument("--no-build", action="store_true", help="Package the existing DOL and its build manifest")
    args = parser.parse_args()
    offset = args.utc_offset_seconds
    if offset is None:
        actual = datetime.now().astimezone().utcoffset()
        if actual is None:
            parser.error("the computer has no UTC offset; supply --utc-offset-seconds")
        offset = int(actual.total_seconds())
        print(f"Using computer local UTC offset {offset} seconds; the Wii clock must use the same local time.", flush=True)
    try:
        meta = metadata(offset, args.network_endpoint, args.mode, args.console_model, args.motherboard, args.media, args.focus)
    except ValueError as error:
        parser.error(str(error))
    source = ROOT / "target/xtask/probes/wii_std_readiness_probe.dol"
    manifest = source.with_suffix(".manifest.json")
    if not args.no_build:
        subprocess.run(["cargo", "xtask", "build-wii-std-app", "lwp-probe", "--bin", "wii_std_readiness_probe", "--features", "wii-std-sync,agent-report" if args.agent_report else "wii-std-sync", "--output", str(source)], cwd=ROOT, check=True)
    digest = hashlib.sha256(source.read_bytes()).hexdigest()
    build = json.loads(manifest.read_text())
    # The manifest field is checked before copying an existing artifact.
    if build.get("dol_sha256") != digest:
        raise ValueError("DOL hash does not match its build manifest")
    output = ROOT / "target/xtask/apps/wii-std-readiness-probe"
    output.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, output / "boot.dol")
    shutil.copyfile(manifest, output / "build.manifest.json")
    (output / "meta.xml").write_bytes(meta)
    (output / "loader-settings.json").write_text(json.dumps({"focus": args.focus, "media": args.media, "agent_report": args.agent_report, "mode": args.mode, "utc_offset_seconds": offset, "network_endpoint": args.network_endpoint, "dol_sha256": digest}, indent=2) + "\n")
    print(f"HBC app: {output}\nDOL SHA-256: {digest}")


if __name__ == "__main__":
    main()
