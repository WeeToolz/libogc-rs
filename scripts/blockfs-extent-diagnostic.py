#!/usr/bin/env python3
"""Run the production BlockFS extent scanner on Dolphin's PowerPC CPU.

Uses synthetic sector reads, not the SD driver or an imported NAND.
The existing bare-DOL MAD harness supplies the linker and startup code.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import struct
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
HARNESS = ROOT / "scripts/mad-ppc-differential/harness"
SOURCE = r'''
#![no_std]
#![no_main]
use core::panic::PanicInfo;
mod extent;
use extent::{ExtentError, exfat_extent};

#[unsafe(no_mangle)]
static mut RESULTS: [u32; 3] = [0; 3];
#[panic_handler]
fn panic(_: &PanicInfo) -> ! { loop { core::hint::spin_loop(); } }

fn partition(boot: &mut [u8; 512], slot: usize, kind: u8, start: u32, length: u32) {
    let offset = 446 + 16 * slot;
    boot[offset + 4] = kind;
    boot[offset + 8..offset + 12].copy_from_slice(&start.to_le_bytes());
    boot[offset + 12..offset + 16].copy_from_slice(&length.to_le_bytes());
    boot[510..512].copy_from_slice(&[0x55, 0xaa]);
}
fn exfat_boot(length: u64) -> [u8; 512] {
    let mut boot = [0; 512];
    boot[3..11].copy_from_slice(b"EXFAT   ");
    boot[72..80].copy_from_slice(&length.to_le_bytes());
    boot
}

#[unsafe(no_mangle)]
pub extern "C" fn run() {
    let mut mbr = [0; 512];
    partition(&mut mbr, 0, 7, 32, 16);
    partition(&mut mbr, 2, 7, 64, 128);
    let mut reads = [0; 3];
    let mut count = 0;
    let result = exfat_extent(|lba, out| {
        if count == reads.len() { return Err(ExtentError::Io); }
        reads[count] = lba;
        count += 1;
        out.0 = match lba {
            0 => mbr, 32 => [0; 512], 64 => exfat_boot(120),
            _ => return Err(ExtentError::Io),
        };
        Ok(())
    });
    let mut flags = u32::from(result == Ok(Some(192)) && reads == [0, 32, 64]);
    mbr = [0; 512];
    partition(&mut mbr, 0, 7, 32, 16);
    let mut candidate = [0; 512];
    partition(&mut candidate, 1, 7, 100, 20);
    let result = exfat_extent(|lba, out| {
        out.0 = match lba {
            0 => mbr, 32 => candidate, _ => return Err(ExtentError::Io),
        };
        Ok(())
    });
    flags |= u32::from(result == Ok(None)) << 1;
    let result = exfat_extent(|lba, out| {
        if lba != 0 { return Err(ExtentError::Io); }
        out.0 = exfat_boot(123);
        Ok(())
    });
    flags |= u32::from(result == Ok(Some(123))) << 2;
    let result = exfat_extent(|lba, out| {
        if lba != 0 { return Err(ExtentError::Io); }
        out.0 = mbr;
        Ok(())
    });
    flags |= u32::from(result == Err(ExtentError::Io)) << 3;
    let result = exfat_extent(|lba, out| {
        out.0 = if lba == 0 { mbr } else { exfat_boot(17) };
        Ok(())
    });
    flags |= u32::from(result == Err(ExtentError::InvalidGeometry)) << 4;
    let out = core::ptr::addr_of_mut!(RESULTS).cast::<u32>();
    unsafe {
        out.add(1).write_volatile(flags);
        out.add(2).write_volatile(5);
        out.write_volatile(0x42465331);
    }
}
'''


def run(*args, cwd):
    subprocess.run(args, cwd=cwd, check=True)


def capture(elf, dol, output):
    symbols = subprocess.check_output(["readelf", "-Ws", str(elf)], text=True)
    result = re.search(r"\b([0-9a-fA-F]+)\s+12\s+OBJECT\s+GLOBAL.*\bRESULTS\b", symbols)
    finished = re.search(r"\b([0-9a-fA-F]+)\s+0\s+NOTYPE\s+GLOBAL.*\bfinished\b", symbols)
    assert result and finished
    address, pc = int(result[1], 16), int(finished[1], 16)
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    args = ["xvfb-run", "-a", os.environ["DOLPHIN_BIN"], "--user", str(output / "user"),
            "--batch", "--video_backend=OGL", "-C", "Dolphin.Analytics.PermissionAsked=True",
            "-C", "Dolphin.Analytics.Enabled=False", "-C", "Dolphin.Interface.ConfirmStop=False",
            "-C", "Dolphin.Interface.AbortOnPanicAlert=True", "-C", f"Dolphin.General.GDBPort={port}",
            "-C", "Dolphin.DSP.Backend=No Audio Output", "--exec", str(dol)]
    with (output / "dolphin.log").open("wb") as log:
        process = subprocess.Popen(args, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            for _ in range(120):
                if process.poll() is not None:
                    raise RuntimeError(f"Dolphin exited {process.returncode}")
                try:
                    connection = socket.create_connection(("127.0.0.1", port), timeout=0.25)
                    break
                except OSError:
                    time.sleep(0.25)
            else:
                raise TimeoutError("Dolphin GDB stub did not start")
            with connection as sock:
                sock.settimeout(10)
                pending = []

                def byte():
                    value = sock.recv(1)
                    if not value:
                        raise EOFError("GDB disconnected")
                    return value

                def receive_frame():
                    data = bytearray()
                    while (value := byte()) != b"#":
                        data.extend(value)
                    assert int(byte() + byte(), 16) == sum(data) % 256
                    sock.sendall(b"+")
                    return data.decode()

                def packet(payload):
                    data = payload.encode()
                    sock.sendall(b"$" + data + b"#" + f"{sum(data) % 256:02x}".encode())
                    ack = byte()
                    if ack == b"$":
                        pending.append(receive_frame())
                    else:
                        assert ack == b"+", ack

                def reply():
                    if pending:
                        return pending.pop(0)
                    while byte() != b"$":
                        pass
                    return receive_frame()

                packet("?")
                assert reply().startswith(("T", "S"))
                packet("c")
                for _ in range(8):
                    time.sleep(1)
                    sock.sendall(b"\x03")
                    stop = reply()
                    while not stop:
                        stop = reply()
                    assert stop.startswith(("T", "S")), stop
                    packet(f"m{address:x},c")
                    response = reply()
                    while not response or response.startswith(("T", "S")):
                        response = reply()
                    data = bytes.fromhex(response)
                    if len(data) == 12 and struct.unpack_from(">I", data)[0] == 0x42465331:
                        assert f"40:{pc:08x}" in stop.lower(), stop
                        return data, stop
                    packet("c")
                raise TimeoutError("BlockFS diagnostic did not finish")
        finally:
            try:
                os.killpg(process.pid, signal.SIGTERM)
                process.wait(timeout=3)
            except ProcessLookupError:
                pass
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--source", type=Path, default=ROOT / "crates/services/ogc-blockfs/src/extent.rs")
    parser.add_argument("--expect", type=lambda n: int(n, 0), default=31)
    options = parser.parse_args()
    output = options.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    source = options.source.resolve().read_bytes()
    with tempfile.TemporaryDirectory(prefix="blockfs-extent-") as directory:
        work = Path(directory)
        (work / "src").mkdir()
        (work / ".cargo").mkdir()
        (work / "src/main.rs").write_text(SOURCE)
        (work / "src/extent.rs").write_bytes(source)
        for name in ("link.ld", "start.S", "powerpc-nintendo-none.json"):
            shutil.copyfile(HARNESS / name, work / name)
        (work / "Cargo.toml").write_text('[package]\nname="blockfs-extent-diagnostic"\nversion="0.0.0"\nedition="2024"\n[profile.release]\npanic="abort"\nopt-level=2\n')
        linker = shutil.which("ld.lld")
        assert linker, "ld.lld not found"
        (work / ".cargo/config.toml").write_text('[build]\ntarget="powerpc-nintendo-none.json"\n[target.powerpc-nintendo-none]\nlinker=' + json.dumps(linker) + '\nrustflags=["-C","link-arg=-Tlink.ld","-C","link-arg=start.o","-C","link-arg=--gc-sections"]\n')
        run("clang", "--target=powerpc-none-eabi", "-mcpu=750", "-c", "start.S", "-o", "start.o", cwd=work)
        run("cargo", "-Z", "json-target-spec", "-Z", "build-std=core", "-Z", "build-std-features=compiler-builtins-mem", "build", "--offline", "--release", cwd=work)
        elf = work / "target/powerpc-nintendo-none/release/blockfs-extent-diagnostic"
        dol = work / "diagnostic.dol"
        run("python3", str(HARNESS / "elf-to-dol.py"), str(elf), str(dol), cwd=work)
        data, stop = capture(elf, dol, output)
        _, flags, count = struct.unpack(">III", data)
        report = {"source_sha256": hashlib.sha256(source).hexdigest(), "dol_sha256": hashlib.sha256(dol.read_bytes()).hexdigest(), "completion_stop": stop,
                  "checks": count, "passed_mask": flags, "expected_mask": options.expect,
                  "passed": flags.bit_count(), "failed": count - flags.bit_count()}
        (output / "results.bin").write_bytes(data)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
        assert count == 5 and flags == options.expect


if __name__ == "__main__":
    main()
