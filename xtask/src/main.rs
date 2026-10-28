use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
#[path = "../../probes/ogc-system-probe/src/aesnd_output_evidence.rs"]
mod aesnd_output_evidence_tests;
mod audit;
#[cfg(test)]
#[path = "../../crates/compatibility/ogc-card/src/metadata_submit_error.rs"]
mod card_metadata_submit_error_tests;
mod dol_mode;
mod dolphin;
#[cfg(test)]
#[path = "../../crates/hardware/ogc-dsp-sys/src/mailbox.rs"]
mod dsp_mailbox_tests;
mod hardware_register;
mod host_tests;
mod inventory;
#[cfg(test)]
#[path = "../../crates/services/ogc-network-sys/src/startup_ip.rs"]
mod network_startup_tests;
mod pad_logs;
#[cfg(test)]
#[path = "../../probes/ogc-system-probe/src/framebuffer_copy.rs"]
mod probe_framebuffer_copy_tests;
#[cfg(test)]
#[path = "../../probes/ogc-system-probe/src/framebuffer_selection.rs"]
mod probe_framebuffer_selection_tests;
#[cfg(test)]
#[path = "../../probes/ogc-system-probe/src/report_send.rs"]
mod probe_report_send_tests;
mod review_status;
#[cfg(test)]
#[path = "../../crates/services/ogc-usbgecko/src/command.rs"]
mod usbgecko_command_tests;
mod wad_parity;
mod workspace;

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };

    if command == "--help" || command == "-h" || command == "help" {
        print_usage();
        return Ok(());
    }

    match command.as_str() {
        "check-abi" => check_abi(),
        "check-abi-rust-target" => check_abi_rust_target(),
        "check-wii-wiisd" => check_wii_wiisd(),
        "build-dols" => build_dols(),
        "build-dol" => build_custom_dol(&args.collect::<Vec<_>>(), false),
        "build-wii-std-app" => build_custom_dol(&args.collect::<Vec<_>>(), true),
        "build-wii-std-sysroot" => build_wii_std_sysroot_command(),
        "elf-to-dol" => {
            let input = args
                .next()
                .ok_or_else(|| "elf-to-dol requires an input ELF path".to_string())?;
            let output = args
                .next()
                .ok_or_else(|| "elf-to-dol requires an output DOL path".to_string())?;
            if args.next().is_some() {
                return Err("elf-to-dol accepts only an input and output path".into());
            }
            gc_elf::convert_elf_file(
                Path::new(&input),
                Path::new(&output),
                gc_elf::DolProfile::Libogc,
            )
            .map_err(|error| error.to_string())?;
            println!("wrote {output}");
            Ok(())
        }
        "build-examples" => build_examples(),
        "build-probes" => build_probes(),
        "build-probes-logged" => build_probes_logged(),
        "build-pad-probes" => build_pad_probes(),
        "build-dolphin-probes" => build_dolphin_probes(),
        "build-agent-probes" => build_agent_probes(),
        "dolphin-validate" => {
            let rest = args.collect::<Vec<_>>();
            if rest.iter().any(|arg| arg == "--help" || arg == "-h") {
                dolphin::print_help();
                return Ok(());
            }
            let no_build = rest.iter().any(|arg| arg == "--no-build");
            if !no_build {
                build_dolphin_probes()?;
                if dolphin_validation_needs_wii_std_fs(&rest) {
                    build_wii_std_fs_dolphin_probe()?;
                }
            }
            dolphin::validate(&repo_root()?, &rest)
        }
        "validate-pad-logs" => {
            let path = args
                .next()
                .ok_or_else(|| "validate-pad-logs requires a log directory".to_string())?;
            pad_logs::validate(Path::new(&path))
        }
        "audit-hardware-register" => hardware_register::audit(&repo_root()?),
        "build-lwp-probe" => build_lwp_probe(),
        "build-wii-std-probe" => build_lwp_probe_with_std(true),
        "build-storage-probe" => build_storage_probe(),
        "inventory" => generate_inventory(),
        "check-inventory" => check_inventory(),
        "review-status" => review_status::report(&repo_root()?),
        "wad-parity" => wad_parity::run(&repo_root()?, false),
        "wad-parity-refresh" => wad_parity::run(&repo_root()?, true),
        "check-roles" => check_roles(),
        "check-dependencies" => check_dependencies(),
        "test-host" => host_tests::run(&repo_root()?),
        "audit-source" => audit_source(),
        "audit-stubs" => audit_stubs(),
        "check-group" => {
            let group = args
                .next()
                .ok_or_else(|| "check-group requires a group name".to_string())?;
            check_group(&group)
        }
        "check-all-runtime" | "check-runtime-crates" => check_all_runtime(),
        "verify-complete" => verify_complete(),
        "milestone0-verify" => milestone0_verify(),
        "milestone1-verify" => milestone1_verify(),
        "milestone2-verify" => milestone2_verify(),
        "verify" => verify(),
        "phase0-readiness" => phase0_readiness(),
        other => Err(format!("unknown xtask command: {other}")),
    }
}

fn print_usage() {
    eprintln!("xtask commands:");
    eprintln!("  cargo xtask check-abi");
    eprintln!("  cargo xtask check-abi-rust-target");
    eprintln!("  cargo xtask check-wii-wiisd");
    eprintln!("  cargo xtask build-dols");
    eprintln!(
        "  cargo xtask build-dol <package> <--bin|--example> <target> [--manifest-path <path>] [--probe-features <list>] [--dev|--no-dev] [--output <path>]"
    );
    eprintln!("  cargo xtask elf-to-dol <input.elf> <output.dol>");
    eprintln!("  cargo xtask build-examples");
    eprintln!("  cargo xtask build-probes");
    eprintln!("  cargo xtask build-probes-logged");
    eprintln!("  cargo xtask build-pad-probes");
    eprintln!("  cargo xtask build-dolphin-probes");
    eprintln!("  cargo xtask build-agent-probes");
    eprintln!("  cargo xtask dolphin-validate [options]");
    eprintln!("  cargo xtask validate-pad-logs <log-directory>");
    eprintln!("  cargo xtask audit-hardware-register");
    eprintln!("  cargo xtask build-lwp-probe");
    eprintln!("  cargo xtask build-wii-std-probe");
    eprintln!("  cargo xtask build-wii-std-sysroot");
    eprintln!(
        "  cargo xtask build-wii-std-app <package> <--bin|--example> <target> [--manifest-path <path>] [--features <features>] [--output <path>]"
    );
    eprintln!("  cargo xtask build-storage-probe");
    eprintln!("  cargo xtask inventory");
    eprintln!("  cargo xtask check-inventory");
    eprintln!("  cargo xtask review-status");
    eprintln!("  cargo xtask wad-parity | wad-parity-refresh");
    eprintln!("  cargo xtask check-roles");
    eprintln!("  cargo xtask check-dependencies");
    eprintln!("  cargo xtask test-host");
    eprintln!("  cargo xtask audit-source");
    eprintln!("  cargo xtask audit-stubs");
    eprintln!(
        "  cargo xtask check-group <core|storage|input|network|audio|debug|hardware|ui|abi|probes>"
    );
    eprintln!("  cargo xtask check-all-runtime");
    eprintln!("  cargo xtask verify-complete");
    eprintln!("  cargo xtask milestone0-verify");
    eprintln!("  cargo xtask milestone1-verify");
    eprintln!("  cargo xtask milestone2-verify");
    eprintln!("  cargo xtask verify");
    eprintln!("  cargo xtask phase0-readiness");
}

fn repo_root() -> Result<PathBuf, String> {
    env::current_dir().map_err(|err| err.to_string())
}

fn run_command(command: &mut Command) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|err| format!("failed to run {:?}: {err}", command))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command {:?} exited with {status}", command))
    }
}

fn encoded_powerpc_wii_rustflags(root: &Path) -> String {
    let mut flags = Vec::new();

    if let Ok(encoded) = env::var("CARGO_ENCODED_RUSTFLAGS") {
        flags.extend(
            encoded
                .split('\x1f')
                .filter(|part| !part.is_empty())
                .map(str::to_owned),
        );
    } else if let Ok(rustflags) = env::var("RUSTFLAGS") {
        flags.extend(rustflags.split_whitespace().map(str::to_owned));
    }

    flags.extend([
        "--check-cfg=cfg(gamecube)".to_string(),
        "-C".to_string(),
        "panic=abort".to_string(),
        "-C".to_string(),
        format!(
            "link-arg=-T{}",
            root.join("targets/powerpc-nintendo-none.ld").display()
        ),
    ]);

    flags.join("\x1f")
}

fn check_abi() -> Result<(), String> {
    println!("host pointer width: {} bits", usize::BITS);
    println!("expected target pointer width: 32 bits (powerpc-eabi)");
    for item in modplay_capi::layout_snapshot() {
        println!("{} size={} align={}", item.name, item.size, item.align);
    }
    Ok(())
}

fn find_probe_object(dir: &Path) -> Result<PathBuf, String> {
    fn collect(dir: &Path, matches: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))? {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                collect(&path, matches)?;
                continue;
            }
            let Some(name) = path.file_name().and_then(|part| part.to_str()) else {
                continue;
            };
            if name.starts_with("abi_probe-")
                && path.extension().and_then(|ext| ext.to_str()) == Some("o")
            {
                matches.push(path);
            }
        }
        Ok(())
    }

    let mut matches = Vec::new();
    collect(dir, &mut matches)?;
    matches.sort();
    matches
        .pop()
        .ok_or_else(|| format!("no abi_probe object found under {}", dir.display()))
}

fn decode_layout(bin: &Path, header: &[(&str, String)]) -> Result<Vec<String>, String> {
    const NAME_BYTES: usize = 64;
    const ENTRY_BYTES: usize = NAME_BYTES + 4;

    let bytes = fs::read(bin).map_err(|err| format!("{}: {err}", bin.display()))?;
    if bytes.len() % ENTRY_BYTES != 0 {
        return Err(format!(
            "unexpected layout payload size {} (expected multiple of {ENTRY_BYTES})",
            bytes.len()
        ));
    }

    let mut lines = Vec::new();
    for (key, value) in header {
        lines.push(format!("{key}={value}"));
    }
    for chunk in bytes.chunks_exact(ENTRY_BYTES) {
        let name_end = chunk[..NAME_BYTES]
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(NAME_BYTES);
        let name = std::str::from_utf8(&chunk[..name_end])
            .map_err(|err| format!("invalid UTF-8 in layout entry name: {err}"))?;
        let value = u32::from_be_bytes([
            chunk[NAME_BYTES],
            chunk[NAME_BYTES + 1],
            chunk[NAME_BYTES + 2],
            chunk[NAME_BYTES + 3],
        ]);
        lines.push(format!("{name}={value}"));
    }
    Ok(lines)
}

fn print_layout(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

fn check_abi_rust_target() -> Result<(), String> {
    let root = repo_root()?;
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let nightly_rustc = rustup_which(&rustup, "rustc")?;
    let target_spec = root.join("targets/powerpc-nintendo-none.json");
    let target_dir = root.join("target/xtask/rust-abi-target");
    let objcopy = env::var("OBJCOPY").unwrap_or_else(|_| {
        nightly_rust_objcopy(&nightly_rustc).unwrap_or_else(|_| "rust-objcopy".to_string())
    });

    if target_dir.exists() {
        fs::remove_dir_all(&target_dir)
            .map_err(|err| format!("{}: {err}", target_dir.display()))?;
    }
    fs::create_dir_all(&target_dir).map_err(|err| format!("{}: {err}", target_dir.display()))?;

    run_command(
        Command::new(&rustup)
            .arg("run")
            .arg("nightly")
            .arg("cargo")
            .arg("rustc")
            .arg("-Z")
            .arg("build-std=core,alloc")
            .arg("-Z")
            .arg("build-std-features=compiler-builtins-mem")
            .arg("-Z")
            .arg("json-target-spec")
            .arg("--target")
            .arg(&target_spec)
            .arg("--target-dir")
            .arg(&target_dir)
            .arg("-p")
            .arg("abi-probe")
            .arg("--lib")
            .arg("--")
            .arg("--emit=obj")
            .env("RUSTC", &nightly_rustc)
            .current_dir(&root),
    )?;

    // Cargo's directory layout for a JSON target can vary across nightly
    // versions. Search the dedicated target tree instead of assuming a fixed
    // `<target>/debug/deps` path.
    let obj = find_probe_object(&target_dir)?;
    let bin = target_dir.join("abi_probe_layout.bin");
    let mut objcopy_cmd = Command::new(&objcopy);
    objcopy_cmd
        .arg("-O")
        .arg("binary")
        .arg("--only-section=.layout")
        .arg(&obj)
        .arg(&bin);
    if let Some(toolchain_lib_dir) = toolchain_lib_dir(&nightly_rustc) {
        objcopy_cmd.env("DYLD_LIBRARY_PATH", toolchain_lib_dir);
    }
    run_command(&mut objcopy_cmd)?;

    let lines = decode_layout(
        &bin,
        &[
            ("toolchain.rustc", nightly_rustc),
            ("target", "powerpc-nintendo-none".to_string()),
            ("target_spec", target_spec.display().to_string()),
        ],
    )?;
    print_layout(&lines);
    Ok(())
}

fn check_wii_wiisd() -> Result<(), String> {
    let root = repo_root()?;
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let nightly_rustc = rustup_which(&rustup, "rustc")?;
    let target_spec = root.join("targets/powerpc-nintendo-none.json");
    let target_dir = root.join("target/xtask/wii-wiisd-check");

    fs::create_dir_all(&target_dir).map_err(|err| format!("{}: {err}", target_dir.display()))?;

    run_command(
        Command::new(&rustup)
            .arg("run")
            .arg("nightly")
            .arg("cargo")
            .arg("check")
            .arg("-Z")
            .arg("build-std=core,alloc")
            .arg("-Z")
            .arg("build-std-features=compiler-builtins-mem")
            .arg("-Z")
            .arg("json-target-spec")
            .arg("--target")
            .arg(&target_spec)
            .arg("--target-dir")
            .arg(&target_dir)
            .arg("-p")
            .arg("ogc-wiisd")
            .env("RUSTC", &nightly_rustc)
            .current_dir(&root),
    )
}

const PROBE_BINARIES: &[&str] = &[
    "arena_probe",
    "bus_probe",
    "callback_probe",
    "general_probe",
    "runtime_probe",
    "sram_probe",
    "video_probe",
    "wpad_probe",
    "wii_remote_stability_probe",
    "sound_probe",
    "aesnd_native_probe",
    "startup_probe",
    "network_probe",
    "debugger_probe",
    "font_probe",
    "storage_probe",
    "keyboard_probe",
    "input_probe",
    "hid_probe",
    "ipc_async_probe",
    "report_smoke_probe",
    "bluetooth_probe",
    "libmad_probe",
    "libmad_rs_decode_probe",
    "mad_decode_probe",
    "modplay_probe",
    "usbstorage_capacity_probe",
    "usbstorage_transport_probe",
    "hid_descriptor_probe",
    "lwp_probe",
];

#[cfg(test)]
mod probe_artifact_tests {
    #[test]
    fn dolphin_std_filesystem_probe_requests_the_pinned_std_build() {
        assert!(super::dolphin_validation_needs_wii_std_fs(&[]));
        assert!(super::dolphin_validation_needs_wii_std_fs(&[
            "--probe".to_string(),
            "wii_std_fs_probe".to_string(),
        ]));
        assert!(!super::dolphin_validation_needs_wii_std_fs(&[
            "--probe".to_string(),
            "lwp_probe".to_string(),
        ]));
        assert!(super::dolphin_validation_needs_wii_std_fs(&[
            "--probe".to_string(),
            "lwp_probe".to_string(),
            "--probe".to_string(),
            "wii_std_fs_probe".to_string(),
        ]));
    }

    #[test]
    fn physical_audio_and_usb_probes_are_built_as_dols() {
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        for probe in ["aesnd_native_probe", "usbstorage_transport_probe"] {
            assert!(super::PROBE_BINARIES.contains(&probe));
            assert!(manifest.contains(&format!("name = \"{probe}\"")));
            assert!(manifest.contains(&format!("path = \"src/{probe}/main.rs\"")));
        }
    }

    #[test]
    fn probe_builds_isolate_packages_with_different_startup_providers() {
        for include_examples in [false, true] {
            for mode in [
                super::ProbeBuildMode::Plain,
                super::ProbeBuildMode::MountedLog,
                super::ProbeBuildMode::Dolphin,
                super::ProbeBuildMode::AgentReport,
            ] {
                let invocations = mode.invocations(include_examples);
                let packages = invocations
                    .iter()
                    .map(|arguments| {
                        let selected = arguments
                            .windows(2)
                            .filter(|pair| pair[0] == "-p")
                            .map(|pair| pair[1])
                            .collect::<Vec<_>>();
                        assert_eq!(
                            selected.len(),
                            1,
                            "startup features must not merge across packages"
                        );
                        selected[0]
                    })
                    .collect::<Vec<_>>();
                let expected = if include_examples {
                    vec!["ogc-system-probe", "lwp-probe", "examples"]
                } else if mode == super::ProbeBuildMode::AgentReport {
                    vec!["ogc-system-probe"]
                } else {
                    vec!["ogc-system-probe", "lwp-probe"]
                };
                assert_eq!(packages, expected);
            }
        }
    }

    #[test]
    fn modes_select_the_expected_features_and_output_directories() {
        use super::ProbeBuildMode::*;
        for (mode, directory, features) in [
            (Plain, "probes", None),
            (
                MountedLog,
                "probes-logged",
                Some("ogc-system-probe/mounted-log,ogc-system-probe/pad-diagnostics"),
            ),
            (
                Dolphin,
                "probes-dolphin",
                Some("ogc-system-probe/dolphin-automation"),
            ),
            (
                AgentReport,
                "probes-agent",
                Some("ogc-system-probe/agent-report"),
            ),
        ] {
            assert_eq!(mode.output_directory(), directory);
            assert_eq!(mode.features(), features);
        }
    }

    #[test]
    fn modes_preserve_the_dol_output_sets() {
        use super::ProbeBuildMode::*;
        for mode in [Plain, MountedLog, Dolphin, AgentReport] {
            let binaries = mode.binaries();
            let unique = binaries
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(unique.len(), binaries.len(), "duplicate output in {mode:?}");
            for probe in [
                "ogc-system-probe",
                "aesnd_native_probe",
                "usbstorage_transport_probe",
                "bluetooth_probe",
            ] {
                assert!(binaries.contains(&probe));
            }
            for probe in [
                "input_probe",
                "hid_probe",
                "ipc_async_probe",
                "lwp_probe",
                "bt-sync-menu",
            ] {
                assert_eq!(binaries.contains(&probe), mode != AgentReport);
            }
            for probe in super::PAD_PROBE_BINARIES {
                assert_eq!(
                    binaries.contains(probe),
                    matches!(mode, MountedLog | Dolphin)
                );
            }
            let expected_count = match mode {
                Plain => 32,
                MountedLog | Dolphin => 42,
                AgentReport => 27,
            };
            assert_eq!(binaries.len(), expected_count, "output count in {mode:?}");
        }
    }

    #[test]
    fn bluetooth_probe_is_in_the_dol_output_list() {
        assert!(super::PROBE_BINARIES.contains(&"bluetooth_probe"));
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        assert!(manifest.contains("name = \"bluetooth_probe\""));
    }

    #[test]
    fn wii_remote_stability_probe_is_in_the_dol_output_list() {
        assert!(super::PROBE_BINARIES.contains(&"wii_remote_stability_probe"));
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        assert!(manifest.contains("name = \"wii_remote_stability_probe\""));
    }

    #[test]
    fn libmad_probe_is_built_as_a_dol() {
        assert!(super::PROBE_BINARIES.contains(&"libmad_probe"));
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        assert!(manifest.contains("name = \"libmad_probe\""));
        assert!(manifest.contains("libmad-rs = { path = \"../../vendor/libmad-rs\" }"));
    }

    #[test]
    fn libmad_rs_decode_probe_is_built_as_a_dol() {
        assert!(super::PROBE_BINARIES.contains(&"libmad_rs_decode_probe"));
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        assert!(manifest.contains("name = \"libmad_rs_decode_probe\""));
    }

    #[test]
    fn mad_decode_probe_is_built_as_a_dol() {
        assert!(super::PROBE_BINARIES.contains(&"mad_decode_probe"));
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        assert!(manifest.contains("name = \"mad_decode_probe\""));
    }

    #[test]
    fn modplay_probe_is_built_as_a_dol() {
        assert!(super::PROBE_BINARIES.contains(&"modplay_probe"));
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        assert!(manifest.contains("name = \"modplay_probe\""));
        assert!(manifest.contains("modplay-capi = { path = \"../../crates/abi/modplay-capi\" }"));
    }

    #[test]
    fn usbstorage_capacity_probe_is_built_as_a_dol() {
        assert!(super::PROBE_BINARIES.contains(&"usbstorage_capacity_probe"));
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        assert!(manifest.contains("name = \"usbstorage_capacity_probe\""));
    }

    #[test]
    fn hid_descriptor_probe_is_built_as_a_dol() {
        assert!(super::PROBE_BINARIES.contains(&"hid_descriptor_probe"));
        let manifest = include_str!("../../probes/ogc-system-probe/Cargo.toml");
        assert!(manifest.contains("name = \"hid_descriptor_probe\""));
    }
}

const PAD_PROBE_BINARIES: &[&str] = &[
    "pad_state_probe",
    "pad_command_spec0",
    "pad_command_spec1",
    "pad_command_spec2",
    "pad_command_spec4",
    "pad_command_spec5",
    "pad_wired_probe",
    "pad_wavebird_probe",
    "pad_stress_probe",
    "pad_reset_probe",
];

const PLAYABLE_EXAMPLES: &[&str] = &["autoplay_2048", "snake-gc", "sudoku"];

fn build_dols() -> Result<(), String> {
    build_probe_artifacts(true, ProbeBuildMode::Plain)
}

#[derive(Serialize)]
struct DolBuildManifest<'a> {
    schema_version: u32,
    package: &'a str,
    target_kind: &'a str,
    target: &'a str,
    target_triple: &'static str,
    profile: &'static str,
    features: &'a [String],
    standard_library: &'static str,
    dol_bytes: usize,
    dol_sha256: String,
    development: bool,
    elf_path: String,
    source_revision: String,
    source_changes: String,
    sdk_revision: String,
    sdk_changes: String,
}

/// Build a workspace or external Rust target with an explicit Cargo feature set,
/// convert it to DOL, and write a deterministic manifest beside the output.
fn build_custom_dol(args: &[String], with_std: bool) -> Result<(), String> {
    let text = |index: usize, label: &str| -> Result<&str, String> {
        args.get(index)
            .map(String::as_str)
            .ok_or_else(|| format!("build-dol requires a valid {label}"))
    };
    let package = text(0, "package")?;
    let target_kind = text(1, "target kind (--bin or --example)")?;
    if !matches!(target_kind, "--bin" | "--example") {
        return Err("build-dol target kind must be --bin or --example".into());
    }
    let target = text(2, "target name")?;
    let is_cargo_name = |name: &str| {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    };
    if !is_cargo_name(package) || !is_cargo_name(target) {
        return Err("package and target must be Cargo names".into());
    }
    let mut features = Vec::new();
    let mut output_path = None;
    let mut manifest_path = None;
    let mut mode = dol_mode::Mode::default();
    let mut index = 3;
    while index < args.len() {
        let key = args[index].as_str();
        if mode.take_flag(key)? {
            index += 1;
            continue;
        }
        let value = args
            .get(index + 1)
            .map(String::as_str)
            .ok_or_else(|| format!("build-dol option {key} requires a value"))?;
        match key {
            "--features" | "--probe-features" => {
                features.extend(value.split(',').map(str::to_owned))
            }
            "--output" => output_path = Some(PathBuf::from(value)),
            "--manifest-path" => manifest_path = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown build-dol option: {key}")),
        }
        index += 2;
    }
    if features.iter().any(|feature| {
        feature.is_empty()
            || !feature
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'/'))
    }) {
        return Err("features must be comma-separated Cargo feature names".into());
    }
    features.sort();
    features.dedup();
    if mode.development
        && features.iter().any(|feature| {
            matches!(
                feature.rsplit('/').next(),
                Some(
                    "agent-report"
                        | "startup-diagnostics"
                        | "recovery-validation"
                        | "recovery-scan-validation"
                )
            )
        })
    {
        return Err(
            "--dev owns reporting and recovery; remove app-specific diagnostic features".into(),
        );
    }
    if mode.development && with_std {
        return Err("--dev currently supports build-dol (core,alloc); the prebuilt std runtime needs separate development instrumentation".into());
    }

    let root = repo_root()?;
    let manifest_path = manifest_path
        .map(|path| {
            let path = root.join(path);
            let resolved =
                fs::canonicalize(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            if !resolved.is_file() {
                return Err(format!(
                    "{} is not a Cargo manifest file",
                    resolved.display()
                ));
            }
            Ok(resolved)
        })
        .transpose()?;
    let target_spec = root.join("targets/powerpc-nintendo-none.json");
    let target_dir = root.join(if mode.development {
        "target/xtask/dev-dol-build"
    } else if with_std {
        "target/xtask/std-app-build"
    } else {
        "target/xtask/custom-dol-build"
    });
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let nightly_rustc = if with_std {
        wii_build_compiler(&root, &rustup)?
    } else {
        PathBuf::from(rustup_which(&rustup, "rustc")?)
    };
    let mut rustflags = encoded_powerpc_wii_rustflags(&root);
    if rustflags.contains("ogc_dev") {
        return Err("set development mode with --dev, not global RUSTFLAGS".into());
    }
    rustflags.push_str("\x1f--check-cfg=cfg(ogc_dev)");
    if mode.development {
        rustflags.push_str("\x1f--cfg=ogc_dev");
    }
    let mut command = Command::new(&rustup);
    command
        .arg("run")
        .arg("nightly")
        .arg("cargo")
        .arg("build")
        .arg("-Z")
        .arg("json-target-spec")
        .arg("--target")
        .arg(&target_spec)
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--release")
        .arg("-p")
        .arg(package)
        .arg(target_kind)
        .arg(target)
        .env("RUSTC", &nightly_rustc)
        .env("CARGO_ENCODED_RUSTFLAGS", &rustflags)
        .env_remove("RUSTFLAGS")
        .current_dir(&root);
    if !with_std {
        command.args([
            "-Z",
            "build-std=core,alloc",
            "-Z",
            "build-std-features=compiler-builtins-mem",
        ]);
    }
    if !features.is_empty() {
        command.arg("--features").arg(features.join(","));
    }
    let source_root = manifest_path
        .as_ref()
        .and_then(|path| path.parent())
        .unwrap_or(&root)
        .to_path_buf();
    if let Some(path) = manifest_path {
        command.arg("--manifest-path").arg(path);
    }
    run_command(&mut command)?;

    let release_dir = target_dir.join("powerpc-nintendo-none/release");
    let elf_path = release_dir.join(if target_kind == "--example" {
        PathBuf::from("examples").join(target)
    } else {
        PathBuf::from(target)
    });
    let output_path = output_path.unwrap_or_else(|| {
        let safe_package = package
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character
                } else {
                    '_'
                }
            })
            .collect::<String>();
        root.join("target/xtask/custom-dols").join(format!(
            "{safe_package}-{target}{}.dol",
            if mode.development { "-dev" } else { "" }
        ))
    });
    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    gc_elf::convert_elf_file(&elf_path, &output_path, gc_elf::DolProfile::Libogc)
        .map_err(|error| error.to_string())?;
    let dol =
        fs::read(&output_path).map_err(|error| format!("{}: {error}", output_path.display()))?;
    let digest = Sha256::digest(&dol);
    let hash = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let output_path = fs::canonicalize(&output_path).map_err(|error| error.to_string())?;
    let git_value = |directory: &Path, arguments: &[&str]| -> Result<String, String> {
        let result = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(arguments)
            .output()
            .map_err(|error| error.to_string())?;
        if !result.status.success() {
            return Err(format!(
                "cannot record source identity at {}: {}",
                directory.display(),
                String::from_utf8_lossy(&result.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&result.stdout).trim().to_owned())
    };
    let source_revision = git_value(&source_root, &["rev-parse", "HEAD"])?;
    let source_changes = git_value(&source_root, &["status", "--short"])?;
    let sdk_revision = git_value(&root, &["rev-parse", "HEAD"])?;
    let sdk_changes = git_value(&root, &["status", "--short"])?;
    let manifest = DolBuildManifest {
        schema_version: 2,
        package,
        target_kind,
        target,
        target_triple: "powerpc-nintendo-none",
        profile: "release",
        features: &features,
        standard_library: if with_std {
            "std,panic_abort"
        } else {
            "core,alloc"
        },
        dol_bytes: dol.len(),
        dol_sha256: hash,
        development: mode.development,
        elf_path: elf_path.display().to_string(),
        source_revision,
        source_changes,
        sdk_revision,
        sdk_changes,
    };
    let manifest_path = output_path.with_extension("manifest.json");
    let json = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(&manifest_path, json)
        .map_err(|error| format!("{}: {error}", manifest_path.display()))?;
    println!("wrote {}", output_path.display());
    println!("wrote {}", manifest_path.display());
    if mode.development {
        let launcher_path = output_path.with_extension("run.command");
        let feature_text = format!(
            "dev-wrapper{}{}",
            if features.is_empty() { "" } else { "," },
            features.join(",")
        );
        let changes = if manifest.source_changes.is_empty() {
            "none"
        } else {
            &manifest.source_changes
        };
        let script = dol_mode::launcher(
            &output_path,
            &manifest_path,
            &root
                .parent()
                .ok_or("SDK parent directory missing")?
                .join("sandbox-command-scripts/run-wii-diagnostic.command"),
            &feature_text,
            &manifest.source_revision,
            changes,
        );
        fs::write(&launcher_path, script).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&launcher_path, fs::Permissions::from_mode(0o755))
                .map_err(|error| error.to_string())?;
        }
        println!("wrote {}", launcher_path.display());
    }
    Ok(())
}

fn build_examples() -> Result<(), String> {
    build_probe_artifacts(true, ProbeBuildMode::Plain)
}

fn build_probes() -> Result<(), String> {
    build_probe_artifacts(false, ProbeBuildMode::Plain)
}

fn build_probes_logged() -> Result<(), String> {
    build_probe_artifacts(false, ProbeBuildMode::MountedLog)
}

fn build_pad_probes() -> Result<(), String> {
    build_probe_artifacts(false, ProbeBuildMode::MountedLog)
}

fn build_dolphin_probes() -> Result<(), String> {
    build_probe_artifacts(false, ProbeBuildMode::Dolphin)
}

fn build_agent_probes() -> Result<(), String> {
    build_probe_artifacts(false, ProbeBuildMode::AgentReport)
}

fn build_lwp_probe() -> Result<(), String> {
    build_lwp_probe_with_std(false)
}

fn dolphin_validation_needs_wii_std_fs(args: &[String]) -> bool {
    !args.iter().any(|arg| arg == "--probe") || args.iter().any(|arg| arg == "wii_std_fs_probe")
}

/// Prepare and install the pinned Wii sysroot before any consumer build.
fn wii_build_compiler(root: &Path, rustup: &str) -> Result<PathBuf, String> {
    let prepared = Command::new("python3")
        .arg(root.join("scripts/prepare-wii-std.py"))
        .arg("--toolchain")
        .arg("nightly")
        .env("RUSTUP", rustup)
        .current_dir(root)
        .output()
        .map_err(|error| format!("prepare Wii Rust compiler: {error}"))?;
    if !prepared.status.success() {
        return Err(format!(
            "prepare Wii Rust compiler: {}",
            String::from_utf8_lossy(&prepared.stderr)
        ));
    }
    let compiler = String::from_utf8(prepared.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_string();
    if compiler.is_empty() {
        return Err("prepared Wii compiler path is empty".into());
    }
    let status = Command::new("python3")
        .arg(root.join("scripts/build-wii-std-sysroot.py"))
        .arg("--toolchain")
        .arg("nightly")
        .arg("--compiler")
        .arg(&compiler)
        .env("RUSTUP", rustup)
        .env(
            "CARGO_ENCODED_RUSTFLAGS",
            encoded_powerpc_wii_rustflags(root),
        )
        .current_dir(root)
        .status()
        .map_err(|error| format!("build Wii Rust sysroot: {error}"))?;
    if !status.success() {
        return Err(format!("build Wii Rust sysroot exited with {status}"));
    }
    Ok(PathBuf::from(compiler))
}

fn build_wii_std_sysroot_command() -> Result<(), String> {
    let root = repo_root()?;
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let compiler = wii_build_compiler(&root, &rustup)?;
    println!("prepared Wii Rust compiler: {}", compiler.display());
    Ok(())
}

fn build_lwp_probe_with_std(with_std: bool) -> Result<(), String> {
    let root = repo_root()?;
    let target_spec = root.join("targets/powerpc-nintendo-none.json");
    let target_dir = root.join(if with_std {
        "target/xtask/wii-std-probe-build"
    } else {
        "target/xtask/lwp-probe-build"
    });
    let output_dir = root.join("target/xtask/probes");
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let nightly_rustc = wii_build_compiler(&root, &rustup)?;
    let rustflags = encoded_powerpc_wii_rustflags(&root);

    fs::create_dir_all(&target_dir).map_err(|err| format!("{}: {err}", target_dir.display()))?;
    fs::create_dir_all(&output_dir).map_err(|err| format!("{}: {err}", output_dir.display()))?;

    let mut command = Command::new(&rustup);
    command.args(["run", "nightly", "cargo", "build"]);
    if with_std {
        command.args(["--features", "wii-std-sync"]);
        command.args([
            "--bin",
            "wii_std_thread_probe",
            "--bin",
            "wii_std_clock_probe",
            "--bin",
            "wii_std_network_probe",
            "--bin",
            "wii_std_fs_probe",
            "--bin",
            "wii_std_services_probe",
            "--bin",
            "wii_std_panic_probe",
            "--bin",
            "wii_std_usb_probe",
            "--bin",
            "wii_std_error_cleanup_probe",
            "--bin",
            "wii_std_readiness_probe",
        ]);
    }
    run_command(
        command
            .arg("-Z")
            .arg("json-target-spec")
            .arg("--target")
            .arg(&target_spec)
            .arg("--target-dir")
            .arg(&target_dir)
            .arg("--release")
            .arg("-p")
            .arg("lwp-probe")
            .arg("--bin")
            .arg("lwp_probe")
            .env("RUSTC", &nightly_rustc)
            .env("CARGO_ENCODED_RUSTFLAGS", &rustflags)
            .env_remove("RUSTFLAGS")
            .current_dir(&root),
    )?;

    let artifacts: &[(&str, &str)] = if with_std {
        &[
            ("lwp_probe", "wii_std_probe.dol"),
            ("wii_std_thread_probe", "wii_std_thread_probe.dol"),
            ("wii_std_clock_probe", "wii_std_clock_probe.dol"),
            ("wii_std_network_probe", "wii_std_network_probe.dol"),
            ("wii_std_fs_probe", "wii_std_fs_probe.dol"),
            ("wii_std_services_probe", "wii_std_services_probe.dol"),
            ("wii_std_panic_probe", "wii_std_panic_probe.dol"),
            ("wii_std_usb_probe", "wii_std_usb_probe.dol"),
            (
                "wii_std_error_cleanup_probe",
                "wii_std_error_cleanup_probe.dol",
            ),
            ("wii_std_readiness_probe", "wii_std_readiness_probe.dol"),
        ]
    } else {
        &[("lwp_probe", "lwp_probe.dol")]
    };
    for (binary, filename) in artifacts {
        let output = output_dir.join(filename);
        gc_elf::convert_elf_file(
            &target_dir
                .join("powerpc-nintendo-none/release")
                .join(binary),
            &output,
            gc_elf::DolProfile::Libogc,
        )
        .map_err(|error| error.to_string())?;
        println!("wrote {}", output.display());
    }
    Ok(())
}

fn build_wii_std_fs_dolphin_probe() -> Result<(), String> {
    let root = repo_root()?;
    let target_spec = root.join("targets/powerpc-nintendo-none.json");
    let target_dir = root.join("target/xtask/wii-std-fs-dolphin-build");
    let output_dir = root.join("target/xtask/probes-dolphin");
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let nightly_rustc = wii_build_compiler(&root, &rustup)?;
    let rustflags = encoded_powerpc_wii_rustflags(&root);

    fs::create_dir_all(&target_dir).map_err(|err| format!("{}: {err}", target_dir.display()))?;
    fs::create_dir_all(&output_dir).map_err(|err| format!("{}: {err}", output_dir.display()))?;

    run_command(
        Command::new(&rustup)
            .args(["run", "nightly", "cargo", "build"])
            .args([
                "--features",
                "wii-std-sync,ogc-system-probe/dolphin-automation",
                "--bin",
                "wii_std_fs_probe",
                "-Z",
                "json-target-spec",
                "--target",
            ])
            .arg(&target_spec)
            .arg("--target-dir")
            .arg(&target_dir)
            .args(["--release", "-p", "lwp-probe"])
            .env("RUSTC", &nightly_rustc)
            .env("CARGO_ENCODED_RUSTFLAGS", &rustflags)
            .env_remove("RUSTFLAGS")
            .current_dir(&root),
    )?;

    gc_elf::convert_elf_file(
        &target_dir.join("powerpc-nintendo-none/release/wii_std_fs_probe"),
        &output_dir.join("wii_std_fs_probe.dol"),
        gc_elf::DolProfile::Libogc,
    )
    .map_err(|error| error.to_string())
}

fn build_storage_probe() -> Result<(), String> {
    let root = repo_root()?;
    let target_spec = root.join("targets/powerpc-nintendo-none.json");
    let target_dir = root.join("target/xtask/storage-probe-build");
    let output_dir = root.join("target/xtask/probes");
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let nightly_rustc = wii_build_compiler(&root, &rustup)?;
    let rustflags = encoded_powerpc_wii_rustflags(&root);

    fs::create_dir_all(&target_dir).map_err(|err| format!("{}: {err}", target_dir.display()))?;
    fs::create_dir_all(&output_dir).map_err(|err| format!("{}: {err}", output_dir.display()))?;

    run_command(
        Command::new(&rustup)
            .arg("run")
            .arg("nightly")
            .arg("cargo")
            .arg("build")
            .arg("-Z")
            .arg("json-target-spec")
            .arg("--target")
            .arg(&target_spec)
            .arg("--target-dir")
            .arg(&target_dir)
            .arg("--release")
            .arg("-p")
            .arg("ogc-system-probe")
            .arg("--bin")
            .arg("storage_probe")
            .env("RUSTC", &nightly_rustc)
            .env("CARGO_ENCODED_RUSTFLAGS", &rustflags)
            .env_remove("RUSTFLAGS")
            .current_dir(&root),
    )?;

    gc_elf::convert_elf_file(
        &target_dir.join("powerpc-nintendo-none/release/storage_probe"),
        &output_dir.join("storage_probe.dol"),
        gc_elf::DolProfile::Libogc,
    )
    .map_err(|error| error.to_string())?;
    println!("wrote {}", output_dir.join("storage_probe.dol").display());
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeBuildMode {
    Plain,
    MountedLog,
    Dolphin,
    AgentReport,
}

impl ProbeBuildMode {
    fn output_directory(self) -> &'static str {
        match self {
            Self::Plain => "probes",
            Self::MountedLog => "probes-logged",
            Self::Dolphin => "probes-dolphin",
            Self::AgentReport => "probes-agent",
        }
    }

    fn features(self) -> Option<&'static str> {
        match self {
            Self::Plain => None,
            Self::MountedLog => {
                Some("ogc-system-probe/mounted-log,ogc-system-probe/pad-diagnostics")
            }
            Self::Dolphin => Some("ogc-system-probe/dolphin-automation"),
            Self::AgentReport => Some("ogc-system-probe/agent-report"),
        }
    }

    fn invocations(self, include_examples: bool) -> Vec<Vec<&'static str>> {
        // Separate invocations keep LWP's std startup feature out of binaries
        // that do not link the std runtime provider.
        let mut invocations = vec![vec!["-p", "ogc-system-probe", "--bins"]];
        if include_examples || self != Self::AgentReport {
            invocations.push(vec!["-p", "lwp-probe", "--bin", "lwp_probe"]);
        }
        if include_examples {
            invocations.push(vec![
                "-p",
                "examples",
                "--examples",
                "--features",
                "examples/agent-report",
            ]);
        }
        invocations
    }

    fn binaries(self) -> Vec<&'static str> {
        let mut binaries = vec!["ogc-system-probe"];
        binaries.extend(PROBE_BINARIES.iter().copied().filter(|probe| {
            self != Self::AgentReport
                || !matches!(
                    *probe,
                    "input_probe" | "hid_probe" | "ipc_async_probe" | "lwp_probe"
                )
        }));
        if matches!(self, Self::MountedLog | Self::Dolphin) {
            binaries.extend(PAD_PROBE_BINARIES.iter().copied());
        }
        if self != Self::AgentReport {
            binaries.push("bt-sync-menu");
        }
        binaries
    }
}

fn build_probe_artifacts(include_examples: bool, mode: ProbeBuildMode) -> Result<(), String> {
    let root = repo_root()?;
    let target_spec = root.join("targets/powerpc-nintendo-none.json");
    let target_dir = root.join("target/xtask/system-probe-build");
    let dol_root = root.join("target/xtask");
    let cargo_home = root.join("target/xtask-cargo-home");
    let probe_output_name = mode.output_directory();
    let probe_output = dol_root.join(probe_output_name);
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let nightly_rustc = wii_build_compiler(&root, &rustup)?;
    let rustflags = encoded_powerpc_wii_rustflags(&root);

    fs::create_dir_all(&target_dir).map_err(|err| format!("{}: {err}", target_dir.display()))?;
    fs::create_dir_all(&cargo_home).map_err(|err| format!("{}: {err}", cargo_home.display()))?;
    for output_dir in ["system-probe", probe_output_name] {
        let path = dol_root.join(output_dir);
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|err| format!("{}: {err}", path.display()))?;
        }
    }
    if include_examples {
        let examples = dol_root.join("examples");
        if examples.exists() {
            fs::remove_dir_all(&examples)
                .map_err(|err| format!("{}: {err}", examples.display()))?;
        }
        fs::create_dir_all(&examples).map_err(|err| format!("{}: {err}", examples.display()))?;
    }
    fs::create_dir_all(&probe_output)
        .map_err(|err| format!("{}: {err}", probe_output.display()))?;

    for arguments in mode.invocations(include_examples) {
        let mut command = Command::new(&rustup);
        command
            .arg("run")
            .arg("nightly")
            .arg("cargo")
            .arg("build")
            .arg("-Z")
            .arg("json-target-spec")
            .arg("--target")
            .arg(&target_spec)
            .arg("--target-dir")
            .arg(&target_dir)
            .arg("--release")
            .args(arguments)
            .env("RUSTC", &nightly_rustc)
            .env("CARGO_HOME", &cargo_home)
            .env("CARGO_ENCODED_RUSTFLAGS", &rustflags)
            .env_remove("RUSTFLAGS")
            .current_dir(&root);
        if let Some(features) = mode.features() {
            command.arg("--features").arg(features);
        }
        run_command(&mut command)?;
    }

    let build_root = target_dir.join("powerpc-nintendo-none/release");
    for probe in mode.binaries() {
        gc_elf::convert_elf_file(
            &build_root.join(probe),
            &probe_output.join(format!("{probe}.dol")),
            gc_elf::DolProfile::Libogc,
        )
        .map_err(|error| error.to_string())?;
    }

    if include_examples {
        for example in PLAYABLE_EXAMPLES {
            gc_elf::convert_elf_file(
                &build_root.join("examples").join(example),
                &dol_root.join("examples").join(format!("{example}.dol")),
                gc_elf::DolProfile::Libogc,
            )
            .map_err(|error| error.to_string())?;
        }
    }

    if include_examples {
        println!("wrote probe and example DOLs under {}", dol_root.display());
    } else {
        println!("wrote probe DOLs under {}", probe_output.display());
    }
    Ok(())
}

fn generate_inventory() -> Result<(), String> {
    let root = repo_root()?;
    inventory::generate_inventory(&root)?;
    audit::write_debt_report(&root)
}

fn check_inventory() -> Result<(), String> {
    inventory::check_inventory(&repo_root()?)
}

fn check_roles() -> Result<(), String> {
    workspace::check_role_manifest(&repo_root()?)
}

fn check_dependencies() -> Result<(), String> {
    workspace::check_dependency_direction(&repo_root()?)
}

fn audit_source() -> Result<(), String> {
    audit::audit_source_structure(&repo_root()?)
}

fn audit_stubs() -> Result<(), String> {
    let root = repo_root()?;
    audit::audit_source_structure(&root)?;
    inventory::check_inventory(&root)?;
    inventory::audit_runtime_completeness(&root)
}

fn rustup_which(rustup: &str, tool: &str) -> Result<String, String> {
    let output = Command::new(rustup)
        .arg("which")
        .arg(tool)
        .arg("--toolchain")
        .arg("nightly")
        .output()
        .map_err(|err| format!("failed to run {rustup} which {tool}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{rustup} which {tool} --toolchain nightly exited with {}",
            output.status
        ));
    }
    let path = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    Ok(path.trim().to_string())
}

fn nightly_rust_objcopy(nightly_rustc: &str) -> Result<String, String> {
    let rustc_path = Path::new(nightly_rustc);
    let Some(toolchain_root) = rustc_path.parent().and_then(Path::parent) else {
        return Err(format!(
            "could not derive toolchain root from {}",
            rustc_path.display()
        ));
    };
    let rustlib_dir = toolchain_root.join("lib/rustlib");
    for entry in
        fs::read_dir(&rustlib_dir).map_err(|err| format!("{}: {err}", rustlib_dir.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let candidate = entry.path().join("bin/rust-objcopy");
        if candidate.exists() {
            return Ok(candidate.display().to_string());
        }
    }
    Err(format!(
        "no rust-objcopy found under {}",
        rustlib_dir.display()
    ))
}

fn toolchain_lib_dir(nightly_rustc: &str) -> Option<String> {
    let rustc_path = Path::new(nightly_rustc);
    rustc_path
        .parent()
        .and_then(Path::parent)
        .map(|toolchain_root| toolchain_root.join("lib").display().to_string())
}

fn check_group(group: &str) -> Result<(), String> {
    let root = repo_root()?;
    let packages = workspace::packages_for_group(&root, group)?;
    check_runtime_packages(&root, &packages, &format!("group-{group}"))
}

fn check_all_runtime() -> Result<(), String> {
    let root = repo_root()?;
    let packages = workspace::all_runtime_packages(&root)?;
    check_runtime_packages(&root, &packages, "all-runtime")
}

fn check_runtime_packages(root: &Path, packages: &[String], label: &str) -> Result<(), String> {
    let rustup = env::var("RUSTUP").unwrap_or_else(|_| "rustup".to_string());
    let nightly_rustc = rustup_which(&rustup, "rustc")?;
    let target_spec = root.join("targets/powerpc-nintendo-none.json");
    let target_dir = root.join("target/xtask/runtime-checks").join(label);
    let mut rustflags = encoded_powerpc_wii_rustflags(root);
    rustflags.push_str("\x1f-D\x1fwarnings");

    fs::create_dir_all(&target_dir).map_err(|err| format!("{}: {err}", target_dir.display()))?;

    let mut command = Command::new(&rustup);
    command
        .arg("run")
        .arg("nightly")
        .arg("cargo")
        .arg("check")
        .arg("-Z")
        .arg("build-std=core,alloc")
        .arg("-Z")
        .arg("build-std-features=compiler-builtins-mem")
        .arg("-Z")
        .arg("json-target-spec")
        .arg("--target")
        .arg(&target_spec)
        .arg("--target-dir")
        .arg(&target_dir)
        .env("RUSTC", &nightly_rustc)
        .env("CARGO_ENCODED_RUSTFLAGS", rustflags)
        .env_remove("RUSTFLAGS")
        .current_dir(root);

    for package in packages {
        command.arg("-p").arg(package);
    }

    println!(
        "checking {} Wii runtime package(s) for {label}",
        packages.len()
    );
    run_command(&mut command)
}

fn phase0_readiness() -> Result<(), String> {
    let root = repo_root()?;
    let required = [
        "Cargo.toml",
        ".cargo/config.toml",
        "docs/review/libmodplay.md",
        "docs/review/libaesnd.md",
        "docs/review/libdi.md",
        "docs/review/libiso9660.md",
        "docs/review/libogc.md",
        "docs/decisions/0001-phase0-readiness.md",
        "docs/decisions/open-questions.md",
        "docs/architecture.md",
        "docs/crate-roles.md",
        "docs/implementation-status.md",
        "docs/generated/implementation-inventory.csv",
        "docs/generated/implementation-status.md",
        "docs/generated/runtime-audit-debt.md",
        "config/crate-roles.csv",
        "config/dependency-exceptions.csv",
        "config/implementation-overrides.csv",
        "config/implementation-evidence.csv",
        "config/audit-allowlist.csv",
        "config/probe-coverage.csv",
        "examples/Cargo.toml",
        "probes/README.md",
        "tests/abi/README.md",
        "tests/compare/README.md",
        "tests/fixtures/README.md",
    ];

    let mut missing = Vec::new();
    for file in required {
        let path = root.join(file);
        if !path.exists() {
            missing.push(file);
        }
    }

    if missing.is_empty() {
        println!("phase0 readiness required files are present");
        Ok(())
    } else {
        Err(format!("missing required files: {}", missing.join(", ")))
    }
}

fn milestone0_verify() -> Result<(), String> {
    let root = repo_root()?;
    workspace::check_role_manifest(&root)?;
    workspace::check_dependency_direction(&root)?;
    audit::audit_source_structure(&root)?;
    inventory::check_inventory(&root)?;
    phase0_readiness()?;
    host_tests::run(&root)?;
    check_all_runtime()
}

fn milestone1_readiness(root: &Path) -> Result<(), String> {
    let required = [
        "crates/core/ogc-lwp-driver/Cargo.toml",
        "crates/core/ogc-lwp-driver/src/arch/powerpc.rs",
        "crates/core/ogc-lwp-driver/src/runtime.rs",
        "crates/core/ogc-lwp-driver/src/contracts.rs",
        "probes/lwp-probe/Cargo.toml",
        "probes/lwp-probe/src/main.rs",
        "docs/lwp-runtime.md",
        "docs/milestone1-hardware-report.md",
    ];
    let missing: Vec<_> = required
        .iter()
        .copied()
        .filter(|file| !root.join(file).exists())
        .collect();
    if missing.is_empty() {
        println!("milestone1 runtime, probe, and documentation files are present");
        Ok(())
    } else {
        Err(format!("missing Milestone 1 files: {}", missing.join(", ")))
    }
}

fn milestone1_verify() -> Result<(), String> {
    let root = repo_root()?;
    workspace::check_role_manifest(&root)?;
    workspace::check_dependency_direction(&root)?;
    audit::audit_source_structure(&root)?;
    inventory::check_inventory(&root)?;
    phase0_readiness()?;
    milestone1_readiness(&root)?;
    host_tests::run(&root)?;
    check_group("core")?;
    build_lwp_probe()
}

fn milestone2_readiness(root: &Path) -> Result<(), String> {
    let required = [
        "crates/compatibility/ogc-dvd-sys/src/lib.rs",
        "crates/services/ogc-card-sys/src/wii_backend.rs",
        "crates/services/ogc-fat/src/lib.rs",
        "crates/services/ogc-usbstorage-sys/src/wii_backend.rs",
        "probes/ogc-system-probe/src/storage_probe.rs",
        "probes/ogc-system-probe/src/storage_probe_main.rs",
        "docs/milestone2-storage.md",
        "docs/milestone2-hardware-report.md",
    ];
    let missing: Vec<_> = required
        .iter()
        .copied()
        .filter(|file| !root.join(file).exists())
        .collect();
    if missing.is_empty() {
        println!("milestone2 storage runtime, probe, and documentation files are present");
        Ok(())
    } else {
        Err(format!("missing Milestone 2 files: {}", missing.join(", ")))
    }
}

fn milestone2_verify() -> Result<(), String> {
    let root = repo_root()?;
    workspace::check_role_manifest(&root)?;
    workspace::check_dependency_direction(&root)?;
    audit::audit_source_structure(&root)?;
    inventory::check_inventory(&root)?;
    phase0_readiness()?;
    milestone1_readiness(&root)?;
    milestone2_readiness(&root)?;
    host_tests::run(&root)?;
    check_group("storage")?;
    build_storage_probe()
}

fn verify() -> Result<(), String> {
    let root = repo_root()?;
    milestone0_verify()?;
    hardware_register::audit(&root)?;
    check_abi()?;
    check_abi_rust_target()?;
    inventory::audit_runtime_completeness(&root)?;
    run_command(
        Command::new("git")
            .arg("status")
            .arg("--short")
            .arg("--untracked-files=all")
            .current_dir(&root),
    )
}

fn verify_complete() -> Result<(), String> {
    let root = repo_root()?;
    hardware_register::audit(&root)?;
    phase0_readiness()?;
    workspace::check_role_manifest(&root)?;
    workspace::check_dependency_direction(&root)?;
    host_tests::run(&root)?;
    check_abi()?;
    check_abi_rust_target()?;
    inventory::check_inventory(&root)?;
    audit::audit_source_structure(&root)?;
    audit_stubs()?;
    check_all_runtime()?;
    build_dols()?;
    build_probes()
}
