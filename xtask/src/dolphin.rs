use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::ops::{Deref, DerefMut};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_DOLPHIN_NAND: &str = "Library/Application Support/Dolphin/Wii";
const SUMMARY_PREFIX_PASS: &str = "PASS summary passes=";
const SUMMARY_PREFIX_FAIL: &str = "FAIL summary passes=";
const INPUT_PIPE_NAME: &str = "libogc-validation";
const PAGE_CAPTURE_PREFIX: &str = "OGC_DOLPHIN_PAGE_CAPTURE ";
const CAPTURE_COMPLETE: &str = "OGC_DOLPHIN_CAPTURE_COMPLETE";
const USB_GECKO_FIRST_PORT: u16 = 0xd6ec;
const USB_GECKO_PORT_ATTEMPTS: u16 = 11;
const DOLPHIN_CARD_IMAGE_ENV: &str = "LIBOGC_DOLPHIN_CARD_IMAGE";
const GAMECUBE_CARD_BLOCK_SIZE: u64 = 8192;

#[derive(Clone, Copy)]
struct ProbeSpec {
    name: &'static str,
    timeout_seconds: u64,
    input_pipe: bool,
}

pub(crate) fn is_known_probe(name: &str) -> bool {
    matches!(name, "ipc_relaunch_probe" | "ios_reload_probe")
        || PROBES.iter().any(|probe| probe.name == name)
}

const PROBES: &[ProbeSpec] = &[
    ProbeSpec {
        name: "gx_gamecube_abort_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "card_probe",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "modplay_probe",
        timeout_seconds: 45,
        input_pipe: false,
    },
    ProbeSpec {
        name: "mp3player_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "aram_probe",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "usbstorage_capacity_probe",
        timeout_seconds: 45,
        input_pipe: false,
    },
    ProbeSpec {
        name: "fat_sd_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "hid_descriptor_probe",
        timeout_seconds: 45,
        input_pipe: false,
    },
    ProbeSpec {
        name: "libmad_probe",
        timeout_seconds: 45,
        input_pipe: false,
    },
    ProbeSpec {
        name: "arena_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "pad_state_probe",
        timeout_seconds: 180,
        input_pipe: false,
    },
    ProbeSpec {
        name: "pad_command_spec0",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "pad_command_spec1",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "pad_command_spec2",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "pad_command_spec4",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "pad_command_spec5",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "bus_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "callback_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "general_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "runtime_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "sram_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "video_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "gx_api_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "stm_api_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "aesnd_native_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "sound_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "audio_sys_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "asnd_sys_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "startup_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "network_probe",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "wd_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "tinysmb_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "debugger_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "libdb_diagnostics_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "font_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "storage_probe",
        timeout_seconds: 240,
        input_pipe: false,
    },
    ProbeSpec {
        name: "dvd_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "wiisd_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "wii_std_fs_probe",
        timeout_seconds: 240,
        input_pipe: false,
    },
    ProbeSpec {
        name: "wii_std_thread_probe",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "keyboard_probe",
        timeout_seconds: 120,
        input_pipe: false,
    },
    ProbeSpec {
        name: "input_probe",
        timeout_seconds: 90,
        input_pipe: true,
    },
    ProbeSpec {
        name: "wpad_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "hid_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "ipc_async_probe",
        timeout_seconds: 90,
        input_pipe: false,
    },
    ProbeSpec {
        name: "lwp_probe",
        timeout_seconds: 120,
        input_pipe: false,
    },
];

#[derive(Default)]
struct Options {
    no_build: bool,
    dolphin: Option<PathBuf>,
    nand_dir: Option<PathBuf>,
    disc_image: Option<PathBuf>,
    sd_image: Option<PathBuf>,
    probes: Vec<String>,
    artifact_dir: Option<PathBuf>,
    timeout_seconds: Option<u64>,
    disable_wii_sd_card: bool,
    allow_sd_image_writes: bool,
    format_private_card: bool,
}

fn validate_format_selection(options: &Options, has_card: bool) -> Result<(), String> {
    if options.probes.iter().any(|name| name == "ogc_system_probe")
        && (!options.no_build
            || std::env::var_os("LIBOGC_DOLPHIN_DEV_DOL").is_none()
            || std::env::var_os("LIBOGC_DOLPHIN_DEV_ELF").is_none())
    {
        return Err("ogc_system_probe requires --no-build and matching LIBOGC_DOLPHIN_DEV_DOL and LIBOGC_DOLPHIN_DEV_ELF paths; build with --dev --probe-features dolphin-automation".to_string());
    }
    if std::env::var_os("LIBOGC_DOLPHIN_DEV_DOL").is_some()
        && (options.probes.len() != 1
            || options.format_private_card
            || std::env::var_os("LIBOGC_DOLPHIN_CARD_HOTPLUG").is_some())
    {
        return Err(
            "development DOL requires one explicit probe and no card format or hotplug mode"
                .to_string(),
        );
    }
    if std::env::var_os("LIBOGC_DOLPHIN_CARD_HOTPLUG").is_some()
        && (!matches!(options.probes.as_slice(), [probe] if probe == "card_probe")
            || !has_card
            || options.format_private_card)
    {
        return Err("LIBOGC_DOLPHIN_CARD_HOTPLUG requires only --probe card_probe, a card image, and no format option".to_string());
    }
    if options.format_private_card
        && (!matches!(options.probes.as_slice(), [probe] if probe == "storage_probe" || probe == "card_probe")
            || !has_card)
    {
        return Err("--format-private-card requires only --probe storage_probe or card_probe and LIBOGC_DOLPHIN_CARD_IMAGE".to_string());
    }
    Ok(())
}

fn format_card_boot_arguments(address: u32, probe: &str) -> ([u8; 12], &'static [u8]) {
    let command = match probe {
        "storage_probe" => {
            b"storage_probe.dol\0--destructive-card-format=ERASE-SLOT-A\0".as_slice()
        }
        "card_probe" => b"card_probe.dol\0--destructive-card-format=ERASE-SLOT-A\0".as_slice(),
        _ => unreachable!("format probe selection is validated"),
    };
    let mut header = [0; 12];
    header[..4].copy_from_slice(&0x5f61_7267u32.to_be_bytes());
    header[4..8].copy_from_slice(&address.to_be_bytes());
    header[8..].copy_from_slice(&(command.len() as u32).to_be_bytes());
    (header, command)
}

fn hotplug_card_boot_arguments(address: u32) -> ([u8; 12], &'static [u8]) {
    let command = b"card_probe.dol\0--card-hotplug\0".as_slice();
    let mut header = [0; 12];
    header[..4].copy_from_slice(&0x5f61_7267u32.to_be_bytes());
    header[4..8].copy_from_slice(&address.to_be_bytes());
    header[8..].copy_from_slice(&(command.len() as u32).to_be_bytes());
    (header, command)
}

fn card_format_evidence_complete(log: &str) -> bool {
    [
        "PASS card format marker removed after remount",
        "PASS card destructive format final unmount",
    ]
    .iter()
    .all(|expected| log.lines().any(|line| line == *expected))
}

#[derive(Serialize)]
struct SuiteReport {
    schema_version: u32,
    repository_revision: String,
    dirty_worktree: bool,
    host: String,
    dolphin_path: String,
    dolphin_version: String,
    nand_source: String,
    started_unix_seconds: u64,
    finished_unix_seconds: u64,
    artifact_directory: String,
    requested_probes: Vec<String>,
    aborted_after_smoke_failure: bool,
    passed: bool,
    results: Vec<ProbeResult>,
}

#[derive(Serialize)]
struct ProbeResult {
    schema_version: u32,
    probe: String,
    status: String,
    dol_path: String,
    dol_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    sd_image: Option<SdImageEvidence>,
    user_directory: String,
    dolphin_arguments: Vec<String>,
    input_config: Option<String>,
    input_commands: Vec<String>,
    timeout_seconds: u64,
    duration_milliseconds: u128,
    exit_code: Option<i32>,
    summary_line: Option<String>,
    passes: Option<usize>,
    fails: Option<usize>,
    failure_lines: Vec<String>,
    frame_count: usize,
    unique_frame_count: usize,
    page_capture_count: usize,
    page_capture_markers: Vec<String>,
    page_frames: Vec<String>,
    page_capture_error: Option<String>,
    pages_valid: bool,
    frames: Vec<String>,
    final_frame: Option<String>,
    frames_valid: bool,
    stdout_log: String,
    stderr_log: String,
    probe_log: String,
    probe_transport_bytes: String,
    probe_log_transport: String,
    symbol_elf: String,
    timeout_diagnostic: Option<TimeoutDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    relaunch_handoff: Option<RelaunchHandoff>,
    dolphin_log: String,
    probe_log_tail: Vec<String>,
    dolphin_log_tail: Vec<String>,
    detail: String,
}

#[derive(Serialize)]
struct RelaunchHandoff {
    mode: String,
    title: String,
    boot_content: String,
    boot_content_sha256: String,
    signature_address: u32,
    signature_sha256: String,
    program_counter: u32,
    stop_reply: String,
}

struct RelaunchDestination {
    content: String,
    content_sha256: String,
    signature_address: u32,
    signature: Vec<u8>,
    source_text: Vec<std::ops::Range<u32>>,
}

#[derive(Serialize)]
struct SdImageEvidence {
    source: String,
    bytes: u64,
    sha256_before: String,
    sha256_after: String,
    writes_allowed: bool,
}

#[derive(Serialize)]
struct TimeoutDiagnostic {
    boot_stage_address: Option<u32>,
    boot_stage: Option<u32>,
    program_counter: Option<u32>,
    program_counter_symbol: Option<String>,
    program_counter_reply: Option<String>,
    debugger_stop: Option<String>,
    stack_trace: Vec<String>,
    error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ElfSymbol {
    address: u32,
    name: String,
}

#[derive(Debug, Eq, PartialEq)]
struct Summary {
    line: String,
    reported_pass: bool,
    passes: usize,
    fails: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PageCapture {
    page: u32,
    capture: u32,
    address: u32,
    width: u32,
    height: u32,
    stride: u32,
    marker: String,
}

struct NandProfileCopy {
    path: PathBuf,
    cleaned: bool,
}

impl NandProfileCopy {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            cleaned: false,
        }
    }

    fn cleanup(&mut self) -> Result<(), String> {
        if self.path.exists() {
            fs::remove_dir_all(&self.path)
                .map_err(|err| format!("{}: {err}", self.path.display()))?;
        }
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for NandProfileCopy {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub fn validate(root: &Path, args: &[String]) -> Result<(), String> {
    let options = parse_options(args)?;
    let usb_device = std::env::var("LIBOGC_DOLPHIN_USB_READ_DEVICE").ok();
    validate_usb_read_selection(&options.probes, usb_device.as_deref())?;
    validate_format_selection(&options, std::env::var_os(DOLPHIN_CARD_IMAGE_ENV).is_some())?;
    if options.disc_image.is_some() && options.probes.is_empty() {
        return Err("--disc-image requires an explicit --probe selection".to_string());
    }
    let disc_image = options
        .disc_image
        .as_ref()
        .map(|path| {
            let canonical = fs::canonicalize(path)
                .map_err(|error| format!("disc image {}: {error}", path.display()))?;
            if !canonical.is_file() {
                return Err(format!("disc image {} is not a file", path.display()));
            }
            Ok(canonical)
        })
        .transpose()?;
    let sd_image = validate_sd_image_selection(&options)?;
    let nand_source = options
        .nand_dir
        .or_else(|| std::env::var_os("DOLPHIN_WII_NAND").map(PathBuf::from))
        .unwrap_or_else(default_nand_path);
    validate_nand_source(&nand_source)?;
    let dolphin = options
        .dolphin
        .or_else(|| std::env::var_os("DOLPHIN_BIN").map(PathBuf::from))
        .or_else(|| find_on_path("dolphin"))
        .ok_or_else(|| {
            "Dolphin executable not found; set DOLPHIN_BIN or pass --dolphin PATH".to_string()
        })?;
    check_dolphin(&dolphin)?;
    let dolphin_version = command_output(&dolphin, &["--version"])?;
    let help = command_output(&dolphin, &["--help"])?;
    for required in ["--user", "--exec", "--video_backend", "--config"] {
        if !help.contains(required) {
            return Err(format!(
                "Dolphin {} does not advertise required option {required}",
                dolphin.display()
            ));
        }
    }

    let headless = !help.contains("--batch") && help.contains("--platform");
    if !help.contains("--batch") && !headless {
        return Err("Dolphin must support --batch or the NoGUI --platform option".into());
    }

    let selected = select_probes(&options.probes)?;
    if headless && selected.iter().any(|spec| spec.name != "network_probe") {
        return Err("The NoGUI Null backend is currently qualified only for network_probe; use the graphical Dolphin frontend for other probes".into());
    }
    if !options.no_build {
        if options.probes.is_empty() {
            super::build_dolphin_probes()?;
        } else {
            let names: Vec<_> = selected
                .iter()
                .map(|spec| spec.name)
                .filter(|name| {
                    !matches!(
                        *name,
                        "wii_std_fs_probe" | "wii_std_exfat_probe" | "wii_std_thread_probe"
                    )
                })
                .collect();
            if !names.is_empty() {
                super::build_selected_dolphin_probes(&names)?;
            }
        }
        if selected.iter().any(|spec| {
            matches!(
                spec.name,
                "wii_std_fs_probe" | "wii_std_exfat_probe" | "wii_std_thread_probe"
            )
        }) {
            super::build_wii_std_dolphin_probes()?;
        }
    }

    let revision =
        git_output(root, &["rev-parse", "HEAD"]).unwrap_or_else(|_| "unknown".to_string());
    let short_revision = revision.chars().take(12).collect::<String>();
    let started = unix_seconds();
    let artifact_dir = options.artifact_dir.unwrap_or_else(|| {
        root.join("target/dolphin-validation")
            .join(format!("{started}-{short_revision}"))
    });
    fs::create_dir_all(&artifact_dir)
        .map_err(|err| format!("{}: {err}", artifact_dir.display()))?;

    let requested_names: Vec<String> = selected.iter().map(|spec| spec.name.to_string()).collect();
    let dirty = git_output(root, &["status", "--porcelain"])
        .map(|value| !value.trim().is_empty())
        .unwrap_or(true);
    let host = command_output(Path::new("/usr/bin/uname"), &["-a"])
        .unwrap_or_else(|_| std::env::consts::OS.to_string());

    let mut results = Vec::new();
    let mut aborted_after_smoke_failure = false;
    let default_suite = options.probes.is_empty();
    for spec in selected {
        println!("Dolphin probe: {}", spec.name);
        let spec = ProbeSpec {
            timeout_seconds: options.timeout_seconds.unwrap_or(spec.timeout_seconds),
            ..spec
        };
        let result = run_probe(
            root,
            &artifact_dir,
            &dolphin,
            &nand_source,
            headless,
            spec,
            ProbeMedia {
                wii_sd_card_enabled: !options.disable_wii_sd_card,
                disc_image: disc_image.as_deref(),
                sd_image: sd_image.as_deref(),
                allow_sd_image_writes: options.allow_sd_image_writes,
                format_private_card: options.format_private_card,
            },
        )?;
        let passed = result.status == "pass";
        println!("  status={} frames={}", result.status, result.frame_count);
        results.push(result);
        if default_suite && spec.name == "arena_probe" && !passed {
            aborted_after_smoke_failure = true;
            break;
        }
    }

    let passed = !aborted_after_smoke_failure
        && results.len() == requested_names.len()
        && results.iter().all(|result| result.status == "pass");
    let report = SuiteReport {
        schema_version: 1,
        repository_revision: revision,
        dirty_worktree: dirty,
        host,
        dolphin_path: dolphin.display().to_string(),
        dolphin_version,
        nand_source: nand_source.display().to_string(),
        started_unix_seconds: started,
        finished_unix_seconds: unix_seconds(),
        artifact_directory: artifact_dir.display().to_string(),
        requested_probes: requested_names,
        aborted_after_smoke_failure,
        passed,
        results,
    };
    write_json(&artifact_dir.join("suite.json"), &report)?;
    println!(
        "Dolphin validation report: {}",
        artifact_dir.join("suite.json").display()
    );
    if passed {
        Ok(())
    } else {
        Err("Dolphin validation failed; see suite.json and probe artifacts".to_string())
    }
}

fn validate_sd_image_selection(options: &Options) -> Result<Option<PathBuf>, String> {
    let exfat_selected = options
        .probes
        .iter()
        .any(|name| matches!(name.as_str(), "exfat_sd_probe" | "wii_std_exfat_probe"));
    let Some(path) = options.sd_image.as_ref() else {
        if options.allow_sd_image_writes {
            return Err("--allow-sd-image-writes requires --sd-image".to_string());
        }
        return if exfat_selected {
            Err("exfat_sd_probe requires --sd-image".to_string())
        } else {
            Ok(None)
        };
    };
    if options.probes.is_empty() || options.disable_wii_sd_card {
        return Err("--sd-image requires an explicit --probe with Wii SD enabled".to_string());
    }
    let path =
        fs::canonicalize(path).map_err(|error| format!("SD image {}: {error}", path.display()))?;
    let metadata = fs::metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    if !metadata.is_file() || metadata.len() == 0 || !metadata.len().is_multiple_of(512) {
        return Err(format!(
            "{}: expected a nonempty raw SD image",
            path.display()
        ));
    }
    Ok(Some(path))
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--no-build" => {
                options.no_build = true;
                index += 1;
            }
            "--format-private-card" => {
                options.format_private_card = true;
                index += 1;
            }
            "--disable-wii-sd-card" => {
                options.disable_wii_sd_card = true;
                index += 1;
            }
            "--allow-sd-image-writes" => {
                options.allow_sd_image_writes = true;
                index += 1;
            }
            "--dolphin" | "--nand-dir" | "--disc-image" | "--probe" | "--artifact-dir"
            | "--timeout-seconds" | "--sd-image" => {
                let key = args[index].as_str();
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{key} requires a value"))?;
                match key {
                    "--dolphin" => options.dolphin = Some(PathBuf::from(value)),
                    "--nand-dir" => options.nand_dir = Some(PathBuf::from(value)),
                    "--disc-image" => options.disc_image = Some(PathBuf::from(value)),
                    "--sd-image" => options.sd_image = Some(PathBuf::from(value)),
                    "--probe" => options.probes.push(value.clone()),
                    "--artifact-dir" => options.artifact_dir = Some(PathBuf::from(value)),
                    "--timeout-seconds" => {
                        let timeout = value
                            .parse::<u64>()
                            .map_err(|_| format!("invalid timeout: {value}"))?;
                        if timeout == 0 {
                            return Err("timeout must be greater than zero".to_string());
                        }
                        options.timeout_seconds = Some(timeout);
                    }
                    _ => unreachable!(),
                }
                index += 2;
            }
            "--help" | "-h" => index += 1,
            other => return Err(format!("unknown dolphin-validate option: {other}")),
        }
    }
    Ok(options)
}

pub fn print_help() {
    eprintln!("cargo xtask dolphin-validate [--no-build] [--probe NAME]...");
    eprintln!("  --dolphin PATH          Dolphin executable");
    eprintln!("  --nand-dir PATH         source Wii NAND (default: Dolphin user NAND)");
    eprintln!("  --disc-image PATH       attach a disc image; requires explicit --probe");
    eprintln!(
        "  --format-private-card   format a private card copy with storage_probe or card_probe"
    );
    eprintln!("  --sd-image PATH         attach a private copy of a raw Wii SD image");
    eprintln!("  --allow-sd-image-writes allow writes to that private image copy");
    eprintln!("  --artifact-dir PATH     result directory");
    eprintln!("  --timeout-seconds N     override every probe timeout");
    eprintln!("  --disable-wii-sd-card   run probes without Dolphin's virtual Wii SD card");
    eprintln!(
        "  LIBOGC_DOLPHIN_IPC_BOUNDS=low|high|both tests nonzero loader bounds with ipc_async_probe"
    );
    eprintln!(
        "  LIBOGC_DOLPHIN_RELAUNCH_BACKGROUND=1 selects the background build of ipc_relaunch_probe (select only that probe)"
    );
    eprintln!(
        "  LIBOGC_DOLPHIN_RELAUNCH_PANIC_DIAGNOSTIC=1 disables automatic abort on panic alerts for ipc_relaunch_probe"
    );
    eprintln!(
        "  {DOLPHIN_CARD_IMAGE_ENV}=PATH attaches a copied raw card image to storage_probe or card_probe"
    );
}

fn copy_card_image_to_profile(template: &Path, profile: &Path) -> Result<PathBuf, String> {
    let source_metadata =
        fs::metadata(template).map_err(|error| format!("{}: {error}", template.display()))?;
    if !source_metadata.is_file()
        || source_metadata.len() < GAMECUBE_CARD_BLOCK_SIZE * 6
        || !source_metadata
            .len()
            .is_multiple_of(GAMECUBE_CARD_BLOCK_SIZE)
    {
        return Err(format!(
            "{} is not a raw GameCube memory card image",
            template.display()
        ));
    }

    let card_directory = profile.join("GC");
    fs::create_dir_all(&card_directory)
        .map_err(|error| format!("{}: {error}", card_directory.display()))?;
    let destination = card_directory.join("MemoryCardA.USA.raw");
    fs::copy(template, &destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?;
    let copied_size = fs::metadata(&destination)
        .map_err(|error| format!("{}: {error}", destination.display()))?
        .len();
    if copied_size != source_metadata.len() {
        return Err(format!(
            "{}: copied card size {copied_size} differs from source size {}",
            destination.display(),
            source_metadata.len()
        ));
    }
    Ok(destination)
}

fn select_probes(names: &[String]) -> Result<Vec<ProbeSpec>, String> {
    if names.is_empty() {
        return Ok(PROBES.to_vec());
    }
    names
        .iter()
        .map(|name| {
            if name == "ipc_relaunch_probe" || name == "ios_reload_probe" {
                return Ok(ProbeSpec {
                    name: if name == "ios_reload_probe" {
                        "ios_reload_probe"
                    } else {
                        "ipc_relaunch_probe"
                    },
                    timeout_seconds: 90,
                    input_pipe: false,
                });
            }
            if name == "ogc_system_probe" {
                return Ok(ProbeSpec {
                    name: "ogc_system_probe",
                    timeout_seconds: 360,
                    input_pipe: false,
                });
            }
            if name == "exfat_sd_probe" {
                return Ok(ProbeSpec {
                    name: "exfat_sd_probe",
                    timeout_seconds: 90,
                    input_pipe: false,
                });
            }
            if name == "wii_std_exfat_probe" {
                return Ok(ProbeSpec {
                    name: "wii_std_exfat_probe",
                    timeout_seconds: 120,
                    input_pipe: false,
                });
            }
            if name == "usbstorage_transport_probe" {
                return Ok(ProbeSpec {
                    name: "usbstorage_transport_probe",
                    timeout_seconds: 120,
                    input_pipe: false,
                });
            }
            PROBES
                .iter()
                .find(|spec| spec.name == name)
                .copied()
                .ok_or_else(|| format!("unknown Dolphin probe: {name}"))
        })
        .collect()
}

#[cfg(test)]
mod wpad_probe_selection_tests {
    #[test]
    fn ios_reload_probe_requires_explicit_selection() {
        assert!(
            !super::select_probes(&[])
                .unwrap()
                .iter()
                .any(|p| p.name == "ios_reload_probe")
        );
        assert_eq!(
            super::select_probes(&["ios_reload_probe".into()]).unwrap()[0].name,
            "ios_reload_probe"
        );
    }
    #[test]
    fn loader_fixture_only_narrows_reserved_bounds() {
        let (lo, hi) = (0x933e0000, 0x93400000);
        assert_eq!(
            super::ipc_test_loader_bounds("low", lo, hi),
            Ok((lo + 0x1000, 0))
        );
        assert_eq!(
            super::ipc_test_loader_bounds("high", lo, hi),
            Ok((0, hi - 0x1000))
        );
        assert_eq!(
            super::ipc_test_loader_bounds("both", lo, hi),
            Ok((lo + 0x1000, hi - 0x1000))
        );
        assert!(super::ipc_test_loader_bounds("both", hi, lo).is_err());
        assert!(super::ipc_test_loader_bounds("bad", lo, hi).is_err());
    }

    #[test]
    fn relaunch_requires_guest_handoff_without_failure() {
        let good = "PASS IPC relaunch system menu ticket view\nINFO IPC relaunch mode=foreground\nINFO IPC relaunch handoff begins\n";
        assert!(super::relaunch_source_ready(good, false));
        assert!(super::relaunch_source_ready(
            &format!("{good}INFO IPC relaunch returned=0\n"),
            false
        ));
        assert!(!super::relaunch_source_ready(
            &format!("{good}INFO IPC relaunch returned=-1\n"),
            false
        ));
        assert!(!super::relaunch_source_ready(
            &format!("{good}FAIL a check\n"),
            false
        ));
        assert!(!super::relaunch_source_ready(
            "INFO IPC relaunch handoff begins\n",
            false
        ));
        let background = good.replace("foreground", "background");
        assert!(!super::relaunch_source_ready(&background, true));
        assert!(super::relaunch_source_ready(
            &format!(
                "{background}INFO IPC relaunch returned=0\nPASS IPC relaunch request accepted\n"
            ),
            true
        ));
        assert!(!super::relaunch_source_ready(good, true));
        assert_eq!(
            super::relaunch_stop_pc("T0540:81534418;01:816a7628;"),
            Some(0x81534418)
        );
        assert_eq!(super::relaunch_stop_pc("S05"), None);
        assert_eq!(super::relaunch_stop_pc("T0540:abc;"), None);
    }

    #[test]
    fn panic_alert_override_is_limited_to_explicit_relaunch_diagnostics() {
        let mut args = vec!["Dolphin.Interface.AbortOnPanicAlert=True".to_string()];
        super::configure_relaunch_panic_diagnostic(&mut args, "ipc_relaunch_probe", true);
        assert_eq!(args[0], "Dolphin.Interface.AbortOnPanicAlert=False");
        let mut other = vec!["Dolphin.Interface.AbortOnPanicAlert=True".to_string()];
        super::configure_relaunch_panic_diagnostic(&mut other, "runtime_probe", true);
        assert_eq!(other[0], "Dolphin.Interface.AbortOnPanicAlert=True");
    }

    #[test]
    fn headless_frontend_uses_null_backend_without_batch() {
        let mut args = vec!["--batch".to_string(), "--video_backend=OGL".to_string()];
        let original = args.clone();
        super::configure_headless_backend(&mut args, false);
        assert_eq!(args, original);
        super::configure_headless_backend(&mut args, true);
        assert_eq!(args, ["--video_backend=Null", "--platform=headless"]);
    }

    #[test]
    fn relaunch_backend_override_does_not_change_other_probes() {
        let mut args = vec!["--video_backend=OGL".to_string()];
        super::configure_relaunch_backend(&mut args, "ipc_relaunch_probe", true);
        assert_eq!(args[0], "--video_backend=Metal");
        let mut other = vec!["--video_backend=OGL".to_string()];
        super::configure_relaunch_backend(&mut other, "runtime_probe", true);
        assert_eq!(other[0], "--video_backend=OGL");
    }
    #[test]
    fn relaunch_probe_requires_explicit_selection() {
        let defaults = super::select_probes(&[]).unwrap();
        assert!(
            !defaults
                .iter()
                .any(|probe| probe.name == "ipc_relaunch_probe")
        );
        let selected = super::select_probes(&["ipc_relaunch_probe".to_string()]).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "ipc_relaunch_probe");
    }

    #[test]
    fn selects_wpad_probe() {
        let selected = super::select_probes(&["wpad_probe".to_string()]).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "wpad_probe");
        assert_eq!(selected[0].timeout_seconds, 90);
        assert!(!selected[0].input_pipe);
    }
}

fn validate_usb_read_selection(names: &[String], device: Option<&str>) -> Result<(), String> {
    let read_probe = matches!(names, [name] if name == "usbstorage_transport_probe");
    match (read_probe, device) {
        (false, None) => Ok(()),
        (true, Some(value)) if value.len() == 9 && value.as_bytes()[4] == b':'
            && value.bytes().enumerate().all(|(i, b)| i == 4 || b.is_ascii_hexdigit()) => Ok(()),
        _ => Err("USB passthrough requires only --probe usbstorage_transport_probe and LIBOGC_DOLPHIN_USB_READ_DEVICE=vvvv:pppp".to_string()),
    }
}

#[cfg(test)]
mod usb_read_selection_tests {
    #[test]
    fn passthrough_requires_explicit_read_probe_and_valid_identifier() {
        let selected = vec!["usbstorage_transport_probe".to_string()];
        assert!(super::select_probes(&selected).is_ok());
        assert!(super::validate_usb_read_selection(&selected, Some("14cd:1212")).is_ok());
        assert!(super::validate_usb_read_selection(&[], Some("14cd:1212")).is_err());
        assert!(super::validate_usb_read_selection(&selected, Some("bad")).is_err());
        assert!(super::validate_usb_read_selection(&selected, None).is_err());
    }
}

fn default_nand_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_DOLPHIN_NAND)
}

fn find_on_path(executable: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|directory| directory.join(executable))
        .find(|candidate| candidate.is_file())
}

fn validate_nand_source(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!(
            "Dolphin Wii NAND directory is missing: {} (set --nand-dir or DOLPHIN_WII_NAND)",
            path.display()
        ));
    }
    let system_menu_tmd = path.join("title/00000001/00000002/content/title.tmd");
    if !system_menu_tmd.is_file() {
        return Err(format!(
            "Dolphin Wii NAND has no installed System Menu: {}",
            system_menu_tmd.display()
        ));
    }
    Ok(())
}

fn copy_nand(source: &Path, destination: &Path) -> Result<(), String> {
    validate_nand_source(source)?;
    let source = fs::canonicalize(source).map_err(|err| format!("{}: {err}", source.display()))?;
    let destination_base = destination.parent().ok_or_else(|| {
        format!(
            "invalid Dolphin NAND destination: {}",
            destination.display()
        )
    })?;
    let destination_name = destination.file_name().ok_or_else(|| {
        format!(
            "invalid Dolphin NAND destination: {}",
            destination.display()
        )
    })?;
    let destination = fs::canonicalize(destination)
        .or_else(|_| fs::canonicalize(destination_base).map(|parent| parent.join(destination_name)))
        .map_err(|err| format!("{}: {err}", destination.display()))?;
    if destination.starts_with(&source) || source.starts_with(&destination) {
        return Err(format!(
            "Dolphin NAND source and profile copy overlap: {} and {}",
            source.display(),
            destination.display()
        ));
    }
    if destination.exists() {
        fs::remove_dir_all(&destination)
            .map_err(|err| format!("{}: {err}", destination.display()))?;
    }
    copy_directory(&source, &destination)
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|err| format!("{}: {err}", destination.display()))?;
    for entry in fs::read_dir(source).map_err(|err| format!("{}: {err}", source.display()))? {
        let entry = entry.map_err(|err| format!("{}: {err}", source.display()))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|err| format!("{}: {err}", source_path.display()))?;
        if file_type.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)
                .map_err(|err| format!("{}: {err}", source_path.display()))?;
        } else {
            return Err(format!(
                "unsupported file type in Wii NAND: {}",
                source_path.display()
            ));
        }
    }
    Ok(())
}

fn dolphin_probe_elf_path(root: &Path, spec: ProbeSpec) -> PathBuf {
    if super::GAMECUBE_PROBE_BINARIES.contains(&spec.name) {
        return root
            .join("target/xtask/gamecube-system-probe-build/powerpc-nintendo-gamecube/release")
            .join(spec.name);
    }
    let directory = if matches!(
        spec.name,
        "wii_std_fs_probe" | "wii_std_exfat_probe" | "wii_std_thread_probe"
    ) {
        "target/xtask/wii-std-fs-dolphin-build/powerpc-nintendo-none/release"
    } else {
        "target/xtask/system-probe-build/powerpc-nintendo-none/release"
    };
    root.join(directory).join(spec.name)
}

fn dolphin_probe_dol_path(root: &Path, spec: ProbeSpec) -> PathBuf {
    let directory = "target/xtask/probes-dolphin";
    root.join(directory).join(format!("{}.dol", spec.name))
}

struct ProbeMedia<'a> {
    wii_sd_card_enabled: bool,
    disc_image: Option<&'a Path>,
    sd_image: Option<&'a Path>,
    allow_sd_image_writes: bool,
    format_private_card: bool,
}

fn run_probe(
    root: &Path,
    artifact_dir: &Path,
    dolphin: &Path,
    nand_source: &Path,
    headless: bool,
    spec: ProbeSpec,
    media: ProbeMedia<'_>,
) -> Result<ProbeResult, String> {
    let timeout_seconds = spec.timeout_seconds;
    let probe_dir = artifact_dir.join(spec.name);
    let user_dir = probe_dir.join("user");
    fs::create_dir_all(&user_dir).map_err(|err| format!("{}: {err}", user_dir.display()))?;
    let memory_card_path = if matches!(spec.name, "storage_probe" | "card_probe") {
        std::env::var_os(DOLPHIN_CARD_IMAGE_ENV)
            .map(PathBuf::from)
            .map(|template| copy_card_image_to_profile(&template, &user_dir))
            .transpose()?
    } else {
        None
    };
    let mut nand_copy = NandProfileCopy::new(user_dir.join("Wii"));
    copy_nand(nand_source, &nand_copy.path)?;
    let private_sd = media
        .sd_image
        .map(|source| {
            let path = nand_copy.path.join("libogc-probe-sd.raw");
            let bytes = fs::copy(source, &path)
                .map_err(|err| format!("private SD copy {}: {err}", path.display()))?;
            let before = sha256_file(&path)?;
            Ok::<_, String>((path, bytes, before))
        })
        .transpose()?;
    let isfs_tmp = user_dir.join("Wii/tmp");
    fs::create_dir_all(&isfs_tmp).map_err(|err| format!("{}: {err}", isfs_tmp.display()))?;
    let dev_dol = std::env::var_os("LIBOGC_DOLPHIN_DEV_DOL").map(PathBuf::from);
    let dol_path = dev_dol
        .clone()
        .unwrap_or_else(|| dolphin_probe_dol_path(root, spec));
    if !dol_path.is_file() {
        return Err(format!(
            "Dolphin probe DOL is missing: {}",
            dol_path.display()
        ));
    }
    let background_relaunch = std::env::var_os("LIBOGC_DOLPHIN_RELAUNCH_BACKGROUND").as_deref()
        == Some(std::ffi::OsStr::new("1"));
    let destination = if spec.name == "ipc_relaunch_probe"
        && std::env::var_os("LIBOGC_DOLPHIN_RELAUNCH_PANIC_DIAGNOSTIC").is_none()
    {
        Some(relaunch_destination(&nand_copy.path, &dol_path)?)
    } else {
        None
    };
    let symbol_elf = if dev_dol.is_some() {
        std::env::var_os("LIBOGC_DOLPHIN_DEV_ELF")
            .map(PathBuf::from)
            .ok_or("LIBOGC_DOLPHIN_DEV_ELF is required with LIBOGC_DOLPHIN_DEV_DOL")?
    } else {
        dolphin_probe_elf_path(root, spec)
    };
    let symbols = fs::read(&symbol_elf)
        .map_err(|err| format!("{}: {err}", symbol_elf.display()))
        .and_then(|bytes| parse_elf32_symbols(&bytes));
    let boot_stage_address = symbols.as_ref().ok().and_then(|symbols| {
        symbols
            .iter()
            .find(|symbol| symbol.name == "OGC_PROBE_BOOT_STAGE")
            .map(|symbol| symbol.address)
    });

    let mut input_pipe = if spec.input_pipe {
        Some(setup_input_pipe(&user_dir)?)
    } else {
        None
    };
    let stdout_path = probe_dir.join("stdout.log");
    let stderr_path = probe_dir.join("stderr.log");
    let transport_bytes_path = probe_dir.join("probe-transport.bin");
    let stdout =
        File::create(&stdout_path).map_err(|err| format!("{}: {err}", stdout_path.display()))?;
    let stderr =
        File::create(&stderr_path).map_err(|err| format!("{}: {err}", stderr_path.display()))?;
    let mut transport_bytes = File::create(&transport_bytes_path)
        .map_err(|err| format!("{}: {err}", transport_bytes_path.display()))?;
    let gdb_port = unused_tcp_port()?;
    let mut arguments = dolphin_arguments(
        &user_dir,
        &dol_path,
        gdb_port,
        memory_card_path.as_deref(),
        media.disc_image,
        SdLaunch {
            enabled: media.wii_sd_card_enabled,
            image: private_sd.as_ref().map(|(path, _, _)| path.as_path()),
            allow_writes: media.allow_sd_image_writes,
        },
    );
    configure_headless_backend(&mut arguments, headless);
    configure_relaunch_panic_diagnostic(
        &mut arguments,
        spec.name,
        std::env::var_os("LIBOGC_DOLPHIN_RELAUNCH_PANIC_DIAGNOSTIC").is_some(),
    );
    configure_relaunch_backend(
        &mut arguments,
        spec.name,
        std::env::var_os("LIBOGC_DOLPHIN_RELAUNCH_METAL").is_some(),
    );
    if spec.name == "usbstorage_transport_probe" {
        let device = std::env::var("LIBOGC_DOLPHIN_USB_READ_DEVICE")
            .map_err(|_| "USB read device was not set".to_string())?;
        arguments.extend([
            "-C".to_string(),
            format!("Dolphin.USBPassthrough.Devices={device}"),
            "-C".to_string(),
            "Logger.Logs.IOS_USB=True".to_string(),
        ]);
    }
    let started = Instant::now();
    let mut child = ChildGuard::new(
        Command::new(dolphin)
            .args(&arguments)
            .current_dir(root)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|err| format!("failed to start {}: {err}", dolphin.display()))?,
    );
    let mut gdb = GdbClient::connect(gdb_port, Duration::from_secs(10))?;
    gdb.initialise()?;
    // An opt-in test loader may narrow only the existing reserved linker arena.
    if let Some(mode) = std::env::var_os("LIBOGC_DOLPHIN_IPC_BOUNDS") {
        if spec.name != "ipc_async_probe" {
            return Err("IPC bounds injection requires ipc_async_probe".to_string());
        }
        let symbols = symbols.as_ref().map_err(|error| error.to_string())?;
        let address = |name: &str| {
            symbols
                .iter()
                .find(|symbol| symbol.name == name)
                .map(|symbol| symbol.address)
                .ok_or_else(|| format!("missing linker symbol {name}"))
        };
        let lo = address("__ipcbufferLo")?;
        let hi = address("__ipcbufferHi")?;
        let (low, high) = ipc_test_loader_bounds(mode.to_str().unwrap_or_default(), lo, hi)?;
        let mut bytes = [0u8; 8];
        bytes[..4].copy_from_slice(&low.to_be_bytes());
        bytes[4..].copy_from_slice(&high.to_be_bytes());
        gdb.write_memory(0x8000_3130, &bytes)?;
        if gdb.read_memory(0x8000_3130, bytes.len())? != bytes {
            return Err("IPC loader bounds write did not round trip".to_string());
        }
        write_json(&probe_dir.join("loader-bounds.json"), &(lo, hi, low, high))?;
    }
    if media.format_private_card
        || std::env::var_os("LIBOGC_DOLPHIN_CARD_HOTPLUG").is_some()
        || dev_dol.is_some()
    {
        let symbols = symbols
            .as_ref()
            .map_err(|error| format!("card format needs ELF symbols: {error}"))?;
        let symbol = |name: &str| {
            symbols
                .iter()
                .find(|symbol| symbol.name == name)
                .map(|symbol| symbol.address)
                .ok_or_else(|| format!("card format needs ELF symbol {name}"))
        };
        let header_address = symbol("__ogc_startup_argv")?;
        let buffer_address = symbol("__bss_end")?
            .checked_add(0x10000)
            .filter(|address| *address < 0x817e0000)
            .ok_or_else(|| "no safe startup argument space".to_string())?;
        let dev_command = if dev_dol.is_some() {
            let recovery_ms = if spec.name == "ogc_system_probe" {
                // Give the host time to capture its timeout diagnostic before
                // the guest watchdog exits.
                timeout_seconds
                    .saturating_add(30)
                    .saturating_mul(1000)
                    .min(i32::MAX as u64)
            } else {
                180_000
            };
            format!("{}.dol\0--report=127.0.0.1:4298\0--run-id=dolphin-debug\0--build-id={}\0--recovery-reset-ms={recovery_ms}\0", spec.name, sha256_file(&dol_path)?).into_bytes()
        } else {
            Vec::new()
        };
        let (header, command) = if dev_dol.is_some() {
            let mut header = [0u8; 12];
            header[..4].copy_from_slice(&0x5f61_7267u32.to_be_bytes());
            header[4..8].copy_from_slice(&buffer_address.to_be_bytes());
            header[8..].copy_from_slice(&(dev_command.len() as u32).to_be_bytes());
            (header, dev_command.as_slice())
        } else if media.format_private_card {
            format_card_boot_arguments(buffer_address, spec.name)
        } else {
            hotplug_card_boot_arguments(buffer_address)
        };
        gdb.write_memory(buffer_address, command)?;
        gdb.write_memory(header_address, &header)?;
    }
    gdb.continue_execution()?;

    let session_probe_log_path = user_dir.join("WiiSession/tmp/ogc-system-probe.log");
    let configured_probe_log_path = user_dir.join("Wii/tmp/ogc-system-probe.log");
    let retained_probe_log_path = probe_dir.join("probe.log");
    let page_frames_dir = probe_dir.join("pages");
    fs::create_dir_all(&page_frames_dir)
        .map_err(|err| format!("{}: {err}", page_frames_dir.display()))?;
    let deadline = started + Duration::from_secs(timeout_seconds);
    let input_commands = [
        "RELEASE A\n",
        "RELEASE START\n",
        "SET MAIN 0.5 0.5\n",
        "PRESS A\n",
        "RELEASE A\n",
        "PRESS D_RIGHT\n",
        "RELEASE D_RIGHT\n",
        "SET MAIN 1 0.5\n",
        "SET MAIN 0.5 0.5\n",
        "PRESS START\n",
        "RELEASE START\n",
    ];
    let mut input_index = 0usize;
    let mut next_input = Instant::now();
    let mut completion = Completion::TimedOut;
    let mut captured_probe_log = String::new();
    let mut gecko_receiver = None;
    let mut next_gecko_connect = Instant::now();
    let mut progress = ProbeProgress::default();
    let mut page_frames = Vec::new();
    let mut page_capture_error = None;
    let mut timeout_diagnostic = None;
    let mut relaunch_handoff = None;
    let mut destination_seen = None;
    'probe: loop {
        if gecko_receiver.is_none() && Instant::now() >= next_gecko_connect {
            gecko_receiver = connect_usb_gecko()
                .map(spawn_usb_gecko_reader)
                .transpose()?;
            next_gecko_connect = Instant::now() + Duration::from_millis(100);
        }
        if let Some(receiver) = gecko_receiver.as_ref() {
            let mut disconnected = false;
            loop {
                match receiver.try_recv() {
                    Ok(UsbGeckoRead::Data(bytes)) => {
                        transport_bytes.write_all(&bytes).map_err(|err| {
                            format!("USB Gecko raw transcript write failed: {err}")
                        })?;
                        transport_bytes.flush().map_err(|err| {
                            format!("USB Gecko raw transcript flush failed: {err}")
                        })?;
                        captured_probe_log.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    Ok(UsbGeckoRead::Error(err)) => {
                        page_capture_error = Some(err);
                        completion = Completion::CaptureError;
                        break 'probe;
                    }
                    Ok(UsbGeckoRead::Closed) => {
                        disconnected = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnected = true;
                        break;
                    }
                }
            }
            if disconnected {
                gecko_receiver = None;
                next_gecko_connect = Instant::now() + Duration::from_millis(100);
            }
        }
        if captured_probe_log.is_empty()
            && let Some(contents) =
                read_probe_log(&session_probe_log_path, &configured_probe_log_path)?
        {
            captured_probe_log = contents;
        }
        progress.update(&captured_probe_log);
        let captures = &progress.captures;
        while page_frames.len() < captures.len() {
            let capture = &captures[page_frames.len()];
            let path = page_frames_dir.join(format!("page-{:03}.png", capture.capture));
            if let Err(err) = gdb.capture_xfb(capture, &path) {
                page_capture_error = Some(err);
                completion = Completion::CaptureError;
                break 'probe;
            }
            page_frames.push(path);
        }
        if progress.summaries > 1 {
            return Err(format!("probe {} wrote duplicate summaries", spec.name));
        }
        if progress.summaries == 1 && progress.complete && page_frames.len() == captures.len() {
            completion = Completion::Summary;
            break;
        }
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            completion = Completion::Exited(status.to_string());
            break;
        }
        // A successful title handoff replaces the guest, so it cannot emit a
        // normal final summary. Require the explicit source marker, IOS title
        // boot evidence, and live destination code instead of treating a timeout
        // or a title log alone as a pass.
        if let Some(destination) = destination.as_ref()
            && page_frames.len() == captures.len()
            && !page_frames.is_empty()
            && relaunch_source_ready(&captured_probe_log, background_relaunch)
        {
            let log = fs::read_to_string(user_dir.join("Logs/dolphin.log")).unwrap_or_default();
            let boot_line = format!(
                "BootstrapPPC: /title/00000001/00000002/content/{}",
                destination.content
            );
            if log.contains("Re-launching title 0000000100000002 after IOS reload.")
                && log.contains(&boot_line)
            {
                let seen = *destination_seen.get_or_insert_with(Instant::now);
                if seen.elapsed() >= Duration::from_secs(3) {
                    match gdb.observe_relaunch(destination, background_relaunch) {
                        Ok(Some(evidence)) => {
                            relaunch_handoff = Some(evidence);
                            completion = Completion::Handoff;
                            break;
                        }
                        Ok(None) => destination_seen = Some(Instant::now()),
                        Err(error) => {
                            page_capture_error = Some(error);
                            completion = Completion::CaptureError;
                            break;
                        }
                    }
                }
            }
        }
        if Instant::now() >= deadline {
            timeout_diagnostic = Some(match boot_stage_address {
                Some(address) => gdb.capture_timeout_diagnostic(address, &symbols),
                None => TimeoutDiagnostic {
                    boot_stage_address: None,
                    boot_stage: None,
                    program_counter: None,
                    program_counter_symbol: None,
                    program_counter_reply: None,
                    debugger_stop: None,
                    stack_trace: Vec::new(),
                    error: Some(match symbols {
                        Ok(_) => "ELF has no OGC_PROBE_BOOT_STAGE symbol".to_string(),
                        Err(error) => error,
                    }),
                },
            });
            break;
        }
        if let Some(pipe) = input_pipe.as_mut()
            && Instant::now() >= next_input
        {
            pipe.write_all(input_commands[input_index].as_bytes())
                .map_err(|err| format!("input pipe write failed: {err}"))?;
            pipe.flush()
                .map_err(|err| format!("input pipe flush failed: {err}"))?;
            input_index = (input_index + 1) % input_commands.len();
            next_input = Instant::now() + Duration::from_millis(250);
        }
        thread::sleep(Duration::from_millis(100));
    }

    drop(gdb);
    let exit_code = stop_child(&mut child)?;
    drop(input_pipe);
    let duration_milliseconds = started.elapsed().as_millis();
    if captured_probe_log.is_empty() {
        captured_probe_log = read_probe_log(&session_probe_log_path, &configured_probe_log_path)?
            .unwrap_or_default();
    }
    let sd_image = private_sd
        .zip(media.sd_image)
        .map(|((path, bytes, sha256_before), source)| {
            Ok::<_, String>(SdImageEvidence {
                source: source.display().to_string(),
                bytes,
                sha256_after: sha256_file(&path)?,
                sha256_before,
                writes_allowed: media.allow_sd_image_writes,
            })
        })
        .transpose()?;
    nand_copy.cleanup()?;
    let page_capture_markers = captured_probe_log
        .lines()
        .filter(|line| line.starts_with(PAGE_CAPTURE_PREFIX))
        .map(str::to_string)
        .collect::<Vec<_>>();
    captured_probe_log = strip_dolphin_protocol(&captured_probe_log);
    fs::write(&retained_probe_log_path, captured_probe_log.as_bytes())
        .map_err(|err| format!("{}: {err}", retained_probe_log_path.display()))?;
    let probe_log = captured_probe_log;
    let dolphin_log_path = user_dir.join("Logs/dolphin.log");
    let dolphin_log = fs::read_to_string(&dolphin_log_path).unwrap_or_default();
    let summaries = parse_summaries(&probe_log);
    let failure_lines = probe_log
        .lines()
        .filter(|line| line.starts_with("FAIL ") && !line.starts_with(SUMMARY_PREFIX_FAIL))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let frames_dir = user_dir.join("Dump/Frames");
    let frames = list_frames(&frames_dir)?;
    // Dolphin's movie frame dumper enables deterministic mode, which rejects
    // Wii network sockets. Page PNGs are the required visual evidence. Accept
    // an empty supporting frame-dump set, but reject any corrupt file present.
    let frames_valid = frames.iter().all(|path| valid_png(path));
    let unique_frame_count = frames
        .iter()
        .map(|path| sha256_file(path))
        .collect::<Result<HashSet<_>, _>>()?
        .len();
    let pages_valid = !page_capture_markers.is_empty()
        && page_frames.len() == page_capture_markers.len()
        && page_frames.iter().all(|path| valid_png(path));
    let frame_paths = frames
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    let final_frame = frames
        .last()
        .or_else(|| page_frames.last())
        .map(|path| path.display().to_string());
    let page_frame_paths = page_frames
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    let parsed = if summaries.len() == 1 {
        summaries.first()
    } else {
        None
    };
    let (mut status, mut detail) = classify_probe_completion(
        &completion,
        timeout_seconds,
        CompletionEvidence {
            parsed,
            frames_valid,
            pages_valid,
            page_capture_error: page_capture_error.as_deref(),
            page_frames: page_frames.len(),
            page_markers: page_capture_markers.len(),
            summary_count: summaries.len(),
        },
    );
    if matches!(completion, Completion::Handoff)
        && (relaunch_handoff.is_none()
            || !failure_lines.is_empty()
            || !relaunch_source_ready(&probe_log, background_relaunch))
    {
        status = "fail";
        detail = "relaunch handoff evidence incomplete or guest reported failure".to_string();
    }
    if std::env::var_os("LIBOGC_DOLPHIN_IPC_BOUNDS").is_some()
        && ![
            "IPC loader lower bound selected",
            "IPC loader upper bound selected",
            "IPC loader allocated watermarks restored",
        ]
        .iter()
        .all(|label| {
            probe_log
                .lines()
                .any(|line| line == format!("PASS {label}"))
        })
    {
        status = "fail";
        detail = "requested loader bounds checks did not all pass".to_string();
    }
    if media.format_private_card && !card_format_evidence_complete(&probe_log) {
        status = "fail";
        detail = "card format did not verify marker removal and final unmount".to_string();
    }
    if !media.allow_sd_image_writes
        && sd_image
            .as_ref()
            .is_some_and(|image| image.sha256_before != image.sha256_after)
    {
        status = "fail";
        detail = "the read-only SD image changed during the probe".to_string();
    }
    let result = ProbeResult {
        schema_version: 1,
        probe: spec.name.to_string(),
        status: status.to_string(),
        dol_path: dol_path.display().to_string(),
        dol_sha256: sha256_file(&dol_path)?,
        sd_image,
        user_directory: user_dir.display().to_string(),
        dolphin_arguments: arguments,
        input_config: spec
            .input_pipe
            .then(|| user_dir.join("Config/GCPadNew.ini").display().to_string()),
        input_commands: if spec.input_pipe {
            input_commands
                .iter()
                .map(|line| line.trim().to_string())
                .collect()
        } else {
            Vec::new()
        },
        timeout_seconds,
        duration_milliseconds,
        exit_code,
        summary_line: parsed.map(|summary| summary.line.clone()),
        passes: parsed.map(|summary| summary.passes),
        fails: parsed.map(|summary| summary.fails),
        failure_lines,
        frame_count: frames.len(),
        unique_frame_count,
        page_capture_count: page_capture_markers.len(),
        page_capture_markers,
        page_frames: page_frame_paths,
        page_capture_error,
        pages_valid,
        frames: frame_paths,
        final_frame,
        frames_valid,
        stdout_log: stdout_path.display().to_string(),
        stderr_log: stderr_path.display().to_string(),
        probe_log: retained_probe_log_path.display().to_string(),
        probe_transport_bytes: transport_bytes_path.display().to_string(),
        probe_log_transport: "Dolphin USB Gecko TCP".to_string(),
        symbol_elf: symbol_elf.display().to_string(),
        timeout_diagnostic,
        relaunch_handoff,
        dolphin_log: dolphin_log_path.display().to_string(),
        probe_log_tail: last_lines(&probe_log, 50),
        dolphin_log_tail: last_lines(&dolphin_log, 50),
        detail,
    };
    if let Some(diagnostic) = result.timeout_diagnostic.as_ref() {
        write_json(&probe_dir.join("timeout-diagnostic.json"), diagnostic)?;
    }
    write_json(&probe_dir.join("result.json"), &result)?;
    Ok(result)
}

enum Completion {
    Summary,
    Handoff,
    CaptureError,
    Exited(String),
    TimedOut,
}

struct CompletionEvidence<'a> {
    parsed: Option<&'a Summary>,
    frames_valid: bool,
    pages_valid: bool,
    page_capture_error: Option<&'a str>,
    page_frames: usize,
    page_markers: usize,
    summary_count: usize,
}

fn classify_probe_completion(
    completion: &Completion,
    timeout_seconds: u64,
    evidence: CompletionEvidence<'_>,
) -> (&'static str, String) {
    let CompletionEvidence {
        parsed,
        frames_valid,
        pages_valid,
        page_capture_error,
        page_frames,
        page_markers,
        summary_count,
    } = evidence;
    match completion {
        Completion::TimedOut => (
            "timeout",
            format!(
                "probe did not complete within {timeout_seconds} seconds; captured {page_frames} page PNG(s) for {page_markers} acknowledged page(s)"
            ),
        ),
        Completion::Exited(status) => (
            "crash",
            format!("Dolphin exited before the probe summary: {status}"),
        ),
        Completion::CaptureError => (
            "setup_error",
            format!(
                "page capture failed: {}",
                page_capture_error.unwrap_or("unknown error")
            ),
        ),
        Completion::Handoff if frames_valid && pages_valid && page_capture_error.is_none() && summary_count == 0 => (
            "pass",
            "observed System Menu title handoff and live destination code; no guest summary expected".to_string(),
        ),
        Completion::Handoff => ("setup_error", "incomplete relaunch capture evidence".to_string()),
        Completion::Summary => match (parsed, frames_valid, pages_valid, page_capture_error) {
            (Some(summary), true, true, None)
                if summary.reported_pass && summary.passes > 0 && summary.fails == 0 =>
            {
                (
                    "pass",
                    "probe summary reported completed checks and zero failures".to_string(),
                )
            }
            (Some(summary), true, true, None) => (
                "probe_fail",
                format!("probe summary is failing or inconsistent: {}", summary.line),
            ),
            (_, _, _, Some(error)) => ("setup_error", format!("page capture failed: {error}")),
            (_, false, _, _) => (
                "setup_error",
                "an incomplete supporting Dolphin frame dump was produced".to_string(),
            ),
            (_, _, false, _) => (
                "setup_error",
                format!(
                    "captured {page_frames} page PNG(s) for {page_markers} acknowledged page(s)"
                ),
            ),
            _ => (
                "missing_summary",
                format!("expected exactly one summary, found {summary_count}"),
            ),
        },
    }
}

struct ChildGuard(Child);

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self(child)
    }
}

impl Deref for ChildGuard {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ChildGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn relaunch_stop_pc(stop: &str) -> Option<u32> {
    let registers = stop.strip_prefix('T')?.get(2..)?;
    registers.split(';').find_map(|field| {
        let (register, value) = field.split_once(':')?;
        if u32::from_str_radix(register, 16).ok()? != 64 || value.len() != 8 {
            return None;
        }
        u32::from_str_radix(value, 16).ok()
    })
}

fn relaunch_source_ready(log: &str, background: bool) -> bool {
    let mode = if background {
        "INFO IPC relaunch mode=background"
    } else {
        "INFO IPC relaunch mode=foreground"
    };
    let mut lines = log.lines();
    let ticket = lines.any(|line| line == "PASS IPC relaunch system menu ticket view");
    let mode_ok = lines.any(|line| line == mode);
    let handoff = lines.any(|line| line == "INFO IPC relaunch handoff begins");
    let returned: Vec<_> = log
        .lines()
        .filter(|line| line.starts_with("INFO IPC relaunch returned="))
        .collect();
    ticket
        && mode_ok
        && handoff
        && !log.lines().any(|line| line.starts_with("FAIL "))
        && returned
            .iter()
            .all(|line| *line == "INFO IPC relaunch returned=0")
        && if background {
            returned.len() == 1
                && log
                    .lines()
                    .any(|line| line == "PASS IPC relaunch request accepted")
        } else {
            returned.len() <= 1
        }
}

fn dol_text_sections(bytes: &[u8]) -> Result<Vec<(std::ops::Range<u32>, usize)>, String> {
    let mut sections = Vec::new();
    for index in 0..7 {
        let offset = elf_u32(bytes, index * 4)? as usize;
        let address = elf_u32(bytes, 0x48 + index * 4)?;
        let size = elf_u32(bytes, 0x90 + index * 4)?;
        if size == 0 {
            continue;
        }
        let end = address
            .checked_add(size)
            .ok_or("DOL section address overflow")?;
        checked_elf_slice(bytes, offset, size as usize)?;
        sections.push((address..end, offset));
    }
    if sections.is_empty() {
        return Err("DOL has no text sections".into());
    }
    Ok(sections)
}

fn relaunch_destination(nand: &Path, source: &Path) -> Result<RelaunchDestination, String> {
    let dir = nand.join("title/00000001/00000002/content");
    let tmd = fs::read(dir.join("title.tmd")).map_err(|err| err.to_string())?;
    // Wii System Menu TMD uses an RSA-2048 signature and 36-byte records.
    if elf_u32(&tmd, 0)? != 0x0001_0001
        || checked_elf_slice(&tmd, 0x18c, 8)? != 0x0000_0001_0000_0002u64.to_be_bytes()
    {
        return Err("not an RSA-2048 System Menu TMD".into());
    }
    let count = elf_u16(&tmd, 0x1de)? as usize;
    let index = elf_u16(&tmd, 0x1e0)?;
    let record = (0..count)
        .map(|i| 0x1e4 + i * 36)
        .find(|&at| elf_u16(&tmd, at + 4) == Ok(index))
        .ok_or("System Menu boot content not found")?;
    let content = format!("{:08x}.app", elf_u32(&tmd, record)?);
    let image = fs::read(dir.join(&content)).map_err(|err| err.to_string())?;
    let size = u64::from_be_bytes(checked_elf_slice(&tmd, record + 8, 8)?.try_into().unwrap());
    if size != image.len() as u64 {
        return Err("System Menu boot content size mismatch".into());
    }
    let sections = dol_text_sections(&image)?;
    // The Wii menu bootstrap DOL may use a physical entry address.
    let entry = elf_u32(&image, 0xe0)? | 0x8000_0000;
    let (range, offset) = sections
        .iter()
        .find(|(range, _)| range.contains(&entry))
        .ok_or("System Menu entry is outside its text sections")?;
    let length = (range.end - entry).min(64) as usize;
    if length < 32 {
        return Err("System Menu entry signature is too short".into());
    }
    let signature =
        checked_elf_slice(&image, offset + (entry - range.start) as usize, length)?.to_vec();
    let source_bytes = fs::read(source).map_err(|err| err.to_string())?;
    let source_text = dol_text_sections(&source_bytes)?
        .into_iter()
        .map(|(range, _)| range)
        .collect();
    Ok(RelaunchDestination {
        content,
        content_sha256: format!("{:x}", Sha256::digest(&image)),
        signature_address: entry,
        signature,
        source_text,
    })
}

fn ipc_test_loader_bounds(mode: &str, lo: u32, hi: u32) -> Result<(u32, u32), String> {
    // Preserve at least 120 KiB of the 128 KiB Wii IPC arena. Never expand it.
    if lo & 31 != 0 || hi & 31 != 0 || hi.saturating_sub(lo) < 0x20000 {
        return Err("IPC test requires a complete aligned linker arena".to_string());
    }
    match mode {
        "low" => Ok((lo + 0x1000, 0)),
        "high" => Ok((0, hi - 0x1000)),
        "both" => Ok((lo + 0x1000, hi - 0x1000)),
        _ => Err("LIBOGC_DOLPHIN_IPC_BOUNDS must be low, high, or both".to_string()),
    }
}

fn configure_relaunch_backend(arguments: &mut [String], probe: &str, enabled: bool) {
    if enabled && probe == "ipc_relaunch_probe" {
        for argument in arguments {
            if argument == "--video_backend=OGL" {
                *argument = "--video_backend=Metal".to_string();
            }
        }
    }
}

fn configure_headless_backend(arguments: &mut Vec<String>, headless: bool) {
    if !headless {
        return;
    }
    arguments.retain(|argument| argument != "--batch");
    for argument in arguments.iter_mut() {
        if argument == "--video_backend=OGL" {
            // Null is suitable for the selected network probe, not rendering tests.
            *argument = "--video_backend=Null".to_string();
        }
    }
    arguments.push("--platform=headless".to_string());
}

fn configure_relaunch_panic_diagnostic(arguments: &mut [String], probe: &str, enabled: bool) {
    if enabled && probe == "ipc_relaunch_probe" {
        for argument in arguments {
            if argument == "Dolphin.Interface.AbortOnPanicAlert=True" {
                *argument = "Dolphin.Interface.AbortOnPanicAlert=False".to_string();
            }
        }
    }
}

struct SdLaunch<'a> {
    enabled: bool,
    image: Option<&'a Path>,
    allow_writes: bool,
}

impl Default for SdLaunch<'_> {
    fn default() -> Self {
        Self {
            enabled: true,
            image: None,
            allow_writes: false,
        }
    }
}

fn dolphin_arguments(
    user_dir: &Path,
    dol_path: &Path,
    gdb_port: u16,
    memory_card_path: Option<&Path>,
    disc_image: Option<&Path>,
    sd: SdLaunch<'_>,
) -> Vec<String> {
    let speed = "1.0";
    let mut arguments = [
        "--user".to_string(),
        user_dir.display().to_string(),
        "--batch".to_string(),
        "--video_backend=OGL".to_string(),
        // EFB reads return zero when CPU access is disabled in Dolphin.
        "-C".to_string(),
        "Graphics.Hacks.EFBAccessEnable=True".to_string(),
        // Texture-copy probes compare the copied bytes in emulated RAM.
        "-C".to_string(),
        "Graphics.Hacks.EFBToTextureEnable=False".to_string(),
        // Display-copy probes inspect the XFB bytes in emulated RAM.
        "-C".to_string(),
        "Graphics.Hacks.XFBToTextureEnable=False".to_string(),
        "-C".to_string(),
        "Dolphin.Analytics.PermissionAsked=True".to_string(),
        "-C".to_string(),
        "Dolphin.Analytics.Enabled=False".to_string(),
        "-C".to_string(),
        "Dolphin.Interface.ConfirmStop=False".to_string(),
        "-C".to_string(),
        "Dolphin.Interface.AbortOnPanicAlert=True".to_string(),
        "-C".to_string(),
        "Dolphin.Core.PauseOnPanic=False".to_string(),
        "-C".to_string(),
        "Dolphin.Input.BackgroundInput=True".to_string(),
        "-C".to_string(),
        format!("Dolphin.Core.EmulationSpeed={speed}"),
        "-C".to_string(),
        format!("Dolphin.General.GDBPort={gdb_port}"),
        "-C".to_string(),
        "Dolphin.DSP.Backend=No Audio Output".to_string(),
        "-C".to_string(),
        "Logger.Options.WriteToFile=True".to_string(),
        "-C".to_string(),
        "Logger.Options.Verbosity=4".to_string(),
        "-C".to_string(),
        "Logger.Options.WriteToWindow=False".to_string(),
        "-C".to_string(),
        "Logger.Logs.BOOT=True".to_string(),
        "-C".to_string(),
        "Logger.Logs.CORE=True".to_string(),
        "-C".to_string(),
        "Logger.Logs.CI=True".to_string(),
        "-C".to_string(),
        "Logger.Logs.OSREPORT=True".to_string(),
        "-C".to_string(),
        "Logger.Logs.IOS=True".to_string(),
        "-C".to_string(),
        "Logger.Logs.IOS_NET=True".to_string(),
        "-C".to_string(),
        "Logger.Logs.IOS_FS=True".to_string(),
        "-C".to_string(),
        "Logger.Logs.IOS_STM=True".to_string(),
        "-C".to_string(),
        "Dolphin.Core.SIDevice0=6".to_string(),
        "-C".to_string(),
        "Dolphin.Core.SlotB=7".to_string(),
        "-C".to_string(),
        format!("Dolphin.Core.WiiSDCard={}", sd.enabled),
        "-C".to_string(),
        format!(
            "Dolphin.Core.WiiSDCardAllowWrites={}",
            sd.enabled && (sd.image.is_none() || sd.allow_writes)
        ),
    ]
    .into_iter()
    .collect::<Vec<_>>();
    if let Some(path) = memory_card_path {
        arguments.extend([
            "-C".to_string(),
            "Dolphin.Core.SlotA=1".to_string(),
            "-C".to_string(),
            format!("Dolphin.Core.MemcardAPath={}", path.display()),
        ]);
    }
    if let Some(path) = sd.image {
        arguments.extend([
            "-C".to_string(),
            format!("Dolphin.General.WiiSDCardPath={}", path.display()),
        ]);
    }
    if let Some(path) = disc_image {
        arguments.extend([
            "-C".to_string(),
            format!("Dolphin.Core.DefaultISO={}", path.display()),
        ]);
    }
    arguments.extend(["--exec".to_string(), dol_path.display().to_string()]);
    arguments
}

fn strip_dolphin_protocol(contents: &str) -> String {
    let mut output = contents
        .lines()
        .filter(|line| !line.starts_with(PAGE_CAPTURE_PREFIX) && *line != CAPTURE_COMPLETE)
        .collect::<Vec<_>>()
        .join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    output
}

#[derive(Default)]
struct ProbeProgress {
    offset: usize,
    captures: Vec<PageCapture>,
    summaries: usize,
    complete: bool,
}

impl ProbeProgress {
    fn update(&mut self, log: &str) {
        for line in log[self.offset..].split_inclusive('\n') {
            let Some(line_body) = line.strip_suffix('\n') else {
                break;
            };
            self.offset += line.len();
            let line = line_body.strip_suffix('\r').unwrap_or(line_body);
            if let Some(capture) = parse_page_capture(line) {
                self.captures.push(capture);
            } else if parse_summary(line).is_some() {
                self.summaries += 1;
            } else if line == CAPTURE_COMPLETE {
                self.complete = true;
            }
        }
    }
}

fn parse_page_capture(line: &str) -> Option<PageCapture> {
    let fields = line.strip_prefix(PAGE_CAPTURE_PREFIX)?;
    let mut page = None;
    let mut capture = None;
    let mut address = None;
    let mut width = None;
    let mut height = None;
    let mut stride = None;
    for field in fields.split_ascii_whitespace() {
        let (key, value) = field.split_once('=')?;
        match key {
            "page" => page = value.parse().ok(),
            "capture" => capture = value.parse().ok(),
            "addr" => address = u32::from_str_radix(value.strip_prefix("0x")?, 16).ok(),
            "width" => width = value.parse().ok(),
            "height" => height = value.parse().ok(),
            "stride" => stride = value.parse().ok(),
            _ => return None,
        }
    }
    let capture = PageCapture {
        page: page?,
        capture: capture?,
        address: address?,
        width: width?,
        height: height?,
        stride: stride?,
        marker: line.to_string(),
    };
    (capture.width > 0
        && capture.height > 0
        && capture.stride >= capture.width
        && capture.address >= 0x8000_0000)
        .then_some(capture)
}

fn parse_elf32_symbols(bytes: &[u8]) -> Result<Vec<ElfSymbol>, String> {
    if bytes
        .get(..6)
        .is_none_or(|header| header[..4] != *b"\x7fELF" || header[4] != 1 || header[5] != 2)
    {
        return Err("symbol ELF is not a big-endian ELF32 image".to_string());
    }
    let section_offset = elf_u32(bytes, 0x20)? as usize;
    let section_entry_size = elf_u16(bytes, 0x2e)? as usize;
    let section_count = elf_u16(bytes, 0x30)? as usize;
    if section_entry_size < 40 || section_count == 0 {
        return Err("symbol ELF has an invalid section table".to_string());
    }
    let section_table_bytes = section_entry_size
        .checked_mul(section_count)
        .ok_or_else(|| "symbol ELF section table size overflows".to_string())?;
    checked_elf_slice(bytes, section_offset, section_table_bytes)?;

    let mut symbols = Vec::new();
    for section_index in 0..section_count {
        let section = section_offset + section_index * section_entry_size;
        if elf_u32(bytes, section + 4)? != 2 {
            continue;
        }
        let symbol_offset = elf_u32(bytes, section + 16)? as usize;
        let symbol_size = elf_u32(bytes, section + 20)? as usize;
        let string_index = elf_u32(bytes, section + 24)? as usize;
        let symbol_entry_size = elf_u32(bytes, section + 36)? as usize;
        if symbol_entry_size < 16
            || !symbol_size.is_multiple_of(symbol_entry_size)
            || string_index >= section_count
        {
            return Err("symbol ELF has an invalid symbol table".to_string());
        }
        let strings_section = section_offset + string_index * section_entry_size;
        let strings_offset = elf_u32(bytes, strings_section + 16)? as usize;
        let strings_size = elf_u32(bytes, strings_section + 20)? as usize;
        let strings = checked_elf_slice(bytes, strings_offset, strings_size)?;
        let table = checked_elf_slice(bytes, symbol_offset, symbol_size)?;
        for symbol in table.chunks_exact(symbol_entry_size) {
            let name_offset = u32::from_be_bytes(symbol[0..4].try_into().unwrap()) as usize;
            let address = u32::from_be_bytes(symbol[4..8].try_into().unwrap());
            if name_offset == 0 || name_offset >= strings.len() {
                continue;
            }
            let name_tail = &strings[name_offset..];
            let name_end = name_tail
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(|| "symbol ELF has an unterminated symbol name".to_string())?;
            let name = std::str::from_utf8(&name_tail[..name_end])
                .map_err(|_| "symbol ELF has a non-UTF-8 symbol name".to_string())?;
            symbols.push(ElfSymbol {
                address,
                name: name.to_string(),
            });
        }
    }
    if symbols.is_empty() {
        return Err("symbol ELF contains no symbols".to_string());
    }
    symbols.sort_by_key(|symbol| symbol.address);
    Ok(symbols)
}

fn elf_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = checked_elf_slice(bytes, offset, 2)?;
    Ok(u16::from_be_bytes([value[0], value[1]]))
}

fn elf_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = checked_elf_slice(bytes, offset, 4)?;
    Ok(u32::from_be_bytes([value[0], value[1], value[2], value[3]]))
}

fn checked_elf_slice(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], String> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| "symbol ELF extent overflows".to_string())?;
    bytes
        .get(offset..end)
        .ok_or_else(|| "symbol ELF section exceeds the file".to_string())
}

fn nearest_symbol(symbols: &[ElfSymbol], address: u32) -> Option<String> {
    let symbol = symbols
        .iter()
        .filter(|symbol| symbol.address <= address)
        .max_by_key(|symbol| symbol.address)?;
    Some(format!("{}+0x{:x}", symbol.name, address - symbol.address))
}

fn unused_tcp_port() -> Result<u16, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .map_err(|err| format!("failed to reserve a Dolphin GDB port: {err}"))?;
    listener
        .local_addr()
        .map(|address| address.port())
        .map_err(|err| format!("failed to read the Dolphin GDB port: {err}"))
}

struct GdbClient {
    stream: BufReader<TcpStream>,
    pending_packet: Option<String>,
}

impl GdbClient {
    fn connect(port: u16, timeout: Duration) -> Result<Self, String> {
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        let deadline = Instant::now() + timeout;
        loop {
            match TcpStream::connect_timeout(&address.into(), Duration::from_millis(100)) {
                Ok(stream) => {
                    stream
                        .set_nodelay(true)
                        .map_err(|err| format!("failed to configure Dolphin GDB: {err}"))?;
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .map_err(|err| format!("failed to configure Dolphin GDB: {err}"))?;
                    stream
                        .set_write_timeout(Some(Duration::from_secs(5)))
                        .map_err(|err| format!("failed to configure Dolphin GDB: {err}"))?;
                    return Ok(Self {
                        stream: BufReader::new(stream),
                        pending_packet: None,
                    });
                }
                Err(_) if Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => {
                    return Err(format!(
                        "Dolphin GDB did not listen on 127.0.0.1:{port}: {err}"
                    ));
                }
            }
        }
    }

    fn initialise(&mut self) -> Result<(), String> {
        self.send_packet("?")?;
        let stop = self.read_packet()?;
        if !stop.starts_with('T') && !stop.starts_with('S') {
            return Err(format!(
                "Dolphin GDB returned an unexpected initial stop {stop:?}"
            ));
        }
        Ok(())
    }

    fn write_memory(&mut self, address: u32, bytes: &[u8]) -> Result<(), String> {
        use std::fmt::Write as _;
        let mut payload = format!("M{address:x},{:x}:", bytes.len());
        for byte in bytes {
            write!(&mut payload, "{byte:02x}").map_err(|error| error.to_string())?;
        }
        self.send_packet(&payload)?;
        let reply = self.read_packet()?;
        if reply != "OK" {
            return Err(format!("Dolphin GDB rejected memory write: {reply}"));
        }
        Ok(())
    }

    fn continue_execution(&mut self) -> Result<(), String> {
        self.send_packet("c")
    }

    fn observe_relaunch(
        &mut self,
        destination: &RelaunchDestination,
        background: bool,
    ) -> Result<Option<RelaunchHandoff>, String> {
        self.stream
            .get_mut()
            .write_all(&[0x03])
            .map_err(|err| err.to_string())?;
        let stop = self.read_packet()?;
        if !stop.starts_with('T') && !stop.starts_with('S') {
            return Err(format!("unexpected relaunch stop: {stop}"));
        }
        let observation = (|| {
            let pc = match relaunch_stop_pc(&stop) {
                Some(pc) => pc,
                None => self.read_register(64)?,
            };
            let code =
                self.read_memory(destination.signature_address, destination.signature.len())?;
            if code != destination.signature
                || !(0x8000_0000..0x8180_0000).contains(&pc)
                || destination
                    .source_text
                    .iter()
                    .any(|range| range.contains(&pc))
            {
                return Ok(None);
            }
            Ok(Some(RelaunchHandoff {
                mode: if background {
                    "background"
                } else {
                    "foreground"
                }
                .to_string(),
                title: "0000000100000002".to_string(),
                boot_content: destination.content.clone(),
                boot_content_sha256: destination.content_sha256.clone(),
                signature_address: destination.signature_address,
                signature_sha256: format!("{:x}", Sha256::digest(&code)),
                program_counter: pc,
                stop_reply: stop,
            }))
        })();
        self.continue_execution()?;
        observation
    }

    fn capture_xfb(&mut self, capture: &PageCapture, path: &Path) -> Result<(), String> {
        self.stream
            .get_mut()
            .write_all(&[0x03])
            .map_err(|err| format!("failed to pause Dolphin for {}: {err}", capture.marker))?;
        let stop = self.read_packet()?;
        if !stop.starts_with('T') && !stop.starts_with('S') {
            return Err(format!(
                "Dolphin GDB returned an unexpected stop reply {stop:?}"
            ));
        }

        let byte_count = (capture.stride as usize)
            .checked_mul(capture.height as usize)
            .and_then(|pixels| pixels.checked_mul(2))
            .ok_or_else(|| "page framebuffer byte count overflowed".to_string())?;
        let memory_result = self.read_memory(capture.address, byte_count);
        let continue_result = self.continue_execution();
        let xfb = memory_result?;
        continue_result?;
        write_xfb_png(capture, &xfb, path)
    }

    fn capture_timeout_diagnostic(
        &mut self,
        boot_stage_address: u32,
        symbols: &Result<Vec<ElfSymbol>, String>,
    ) -> TimeoutDiagnostic {
        let mut diagnostic = TimeoutDiagnostic {
            boot_stage_address: Some(boot_stage_address),
            boot_stage: None,
            program_counter: None,
            program_counter_symbol: None,
            program_counter_reply: None,
            debugger_stop: None,
            stack_trace: Vec::new(),
            error: None,
        };
        let mut errors = Vec::new();

        if let Err(error) = self.stream.get_mut().write_all(&[0x03]) {
            diagnostic.error = Some(format!("failed to interrupt Dolphin: {error}"));
            return diagnostic;
        }
        match self.read_packet() {
            Ok(stop) if stop.starts_with('T') || stop.starts_with('S') => {
                diagnostic.debugger_stop = Some(stop);
            }
            Ok(reply) => errors.push(format!("unexpected debugger stop reply {reply:?}")),
            Err(error) => errors.push(error),
        }

        // Read this first. It remains useful when Dolphin does not return a PC register.
        match self.read_memory(boot_stage_address, 4) {
            Ok(bytes) if bytes.len() == 4 => {
                diagnostic.boot_stage = Some(u32::from_be_bytes(bytes.try_into().unwrap()));
            }
            Ok(bytes) => errors.push(format!(
                "Dolphin GDB returned {} boot-stage bytes, expected 4",
                bytes.len()
            )),
            Err(error) => errors.push(format!("could not read boot stage: {error}")),
        }

        // PowerPC GDB register 64 is the program counter. Keep its raw reply so an
        // unsupported or empty reply remains visible in the retained artifact.
        match self.send_packet("p40").and_then(|()| self.read_packet()) {
            Ok(reply) => {
                diagnostic.program_counter_reply = Some(reply.clone());
                match decode_hex(&reply) {
                    Ok(bytes) if bytes.len() == 4 => {
                        let pc = u32::from_be_bytes(bytes.try_into().unwrap());
                        diagnostic.program_counter = Some(pc);
                        if let Ok(symbols) = symbols {
                            diagnostic.program_counter_symbol = nearest_symbol(symbols, pc);
                        }
                    }
                    Ok(bytes) => errors.push(format!(
                        "Dolphin GDB returned {} PC bytes, expected 4",
                        bytes.len()
                    )),
                    Err(error) => errors.push(format!("invalid PC reply: {error}")),
                }
            }
            Err(error) => errors.push(format!("could not read program counter: {error}")),
        }
        match self.read_register(1) {
            Ok(mut stack) => {
                for _ in 0..16 {
                    if stack & 3 != 0 || !(0x8000_0000..0x8180_0000).contains(&stack) {
                        break;
                    }
                    match self.read_memory(stack, 8) {
                        Ok(bytes) => {
                            let next = u32::from_be_bytes(bytes[..4].try_into().unwrap());
                            let link = u32::from_be_bytes(bytes[4..].try_into().unwrap());
                            let symbol = symbols
                                .as_ref()
                                .ok()
                                .and_then(|symbols| nearest_symbol(symbols, link))
                                .unwrap_or_default();
                            diagnostic
                                .stack_trace
                                .push(format!("sp={stack:08x} lr={link:08x} {symbol}"));
                            if next <= stack {
                                break;
                            }
                            stack = next;
                        }
                        Err(error) => {
                            errors.push(error);
                            break;
                        }
                    }
                }
            }
            Err(error) => errors.push(error),
        }
        if !errors.is_empty() {
            diagnostic.error = Some(errors.join("; "));
        }
        diagnostic
    }

    fn read_register(&mut self, register: u32) -> Result<u32, String> {
        self.send_packet(&format!("p{register:x}"))?;
        let mut reply = self.read_packet()?;
        while reply.starts_with('T') || reply.starts_with('S') {
            reply = self.read_packet()?;
        }
        let bytes = decode_hex(&reply)?;
        let bytes: [u8; 4] = bytes
            .try_into()
            .map_err(|_| format!("invalid register {register} reply: {reply:?}"))?;
        Ok(u32::from_be_bytes(bytes))
    }

    fn read_memory(&mut self, address: u32, length: usize) -> Result<Vec<u8>, String> {
        // Dolphin's GDB reply buffer is slightly below 4096 bytes, so keep
        // the hex payload below half that size.
        const CHUNK_BYTES: usize = 1024;
        let mut bytes = Vec::with_capacity(length);
        while bytes.len() < length {
            let count = (length - bytes.len()).min(CHUNK_BYTES);
            let chunk_address = address
                .checked_add(bytes.len() as u32)
                .ok_or_else(|| "page framebuffer address overflowed".to_string())?;
            self.send_packet(&format!("m{chunk_address:x},{count:x}"))?;
            let mut reply = self.read_packet()?;
            while reply.is_empty() || reply.starts_with('T') || reply.starts_with('S') {
                reply = self.read_packet()?;
            }
            if is_gdb_error(&reply) {
                return Err(format!(
                    "Dolphin GDB rejected memory at 0x{chunk_address:08x}: {reply}"
                ));
            }
            let decoded =
                decode_hex(&reply).map_err(|err| format!("{err}; GDB reply was {reply:?}"))?;
            if decoded.len() != count {
                return Err(format!(
                    "Dolphin GDB returned {} bytes, expected {count}",
                    decoded.len()
                ));
            }
            bytes.extend_from_slice(&decoded);
        }
        Ok(bytes)
    }

    fn send_packet(&mut self, payload: &str) -> Result<(), String> {
        let checksum = payload
            .as_bytes()
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_add(*byte));
        let packet = format!("${payload}#{checksum:02x}");
        self.stream
            .get_mut()
            .write_all(packet.as_bytes())
            .map_err(|err| format!("Dolphin GDB packet write failed: {err}"))?;
        let mut ack = [0u8; 1];
        self.stream
            .read_exact(&mut ack)
            .map_err(|err| format!("Dolphin GDB acknowledgement read failed: {err}"))?;
        // Dolphin sends the protocol ACK and the reply in separate writes.
        // On Linux a delayed TCP ACK can stall the reply behind Nagle's
        // algorithm for each framebuffer chunk. Request an immediate ACK
        // without changing the debugger protocol or the captured bytes.
        #[cfg(target_os = "linux")]
        {
            use std::os::linux::net::TcpStreamExt;
            let _ = self.stream.get_ref().set_quickack(true);
        }
        match ack[0] {
            b'+' => Ok(()),
            b'-' => Err("Dolphin GDB rejected a packet checksum".to_string()),
            b'$' => {
                self.pending_packet = Some(self.read_packet_after_start()?);
                Ok(())
            }
            other => Err(format!(
                "Dolphin GDB returned an unexpected acknowledgement 0x{other:02x}"
            )),
        }
    }

    fn read_packet(&mut self) -> Result<String, String> {
        if let Some(packet) = self.pending_packet.take() {
            return Ok(packet);
        }
        let mut byte = [0u8; 1];
        loop {
            self.stream
                .read_exact(&mut byte)
                .map_err(|err| format!("Dolphin GDB reply read failed: {err}"))?;
            if byte[0] == b'$' {
                return self.read_packet_after_start();
            }
        }
    }

    fn read_packet_after_start(&mut self) -> Result<String, String> {
        let mut payload = Vec::new();
        self.stream
            .read_until(b'#', &mut payload)
            .map_err(|err| format!("Dolphin GDB reply read failed: {err}"))?;
        if payload.pop() != Some(b'#') {
            return Err("Dolphin GDB reply ended before its checksum".into());
        }
        let mut checksum_text = [0u8; 2];
        self.stream
            .read_exact(&mut checksum_text)
            .map_err(|err| format!("Dolphin GDB checksum read failed: {err}"))?;
        let expected = decode_hex_byte(checksum_text[0], checksum_text[1])?;
        let actual = payload
            .iter()
            .fold(0u8, |sum, value| sum.wrapping_add(*value));
        if actual != expected {
            let _ = self.stream.get_mut().write_all(b"-");
            return Err(format!(
                "Dolphin GDB reply checksum mismatch: {actual:02x} != {expected:02x}"
            ));
        }
        self.stream
            .get_mut()
            .write_all(b"+")
            .map_err(|err| format!("Dolphin GDB reply acknowledgement failed: {err}"))?;
        String::from_utf8(payload).map_err(|err| format!("Dolphin GDB reply was not UTF-8: {err}"))
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("Dolphin GDB returned an odd-length hex value".to_string());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| decode_hex_byte(pair[0], pair[1]))
        .collect()
}

fn is_gdb_error(reply: &str) -> bool {
    reply.len() == 3 && reply.starts_with('E') && decode_hex(&reply[1..]).is_ok()
}

fn decode_hex_byte(high: u8, low: u8) -> Result<u8, String> {
    fn nibble(value: u8) -> Option<u8> {
        match value {
            b'0'..=b'9' => Some(value - b'0'),
            b'a'..=b'f' => Some(value - b'a' + 10),
            b'A'..=b'F' => Some(value - b'A' + 10),
            _ => None,
        }
    }
    let high = nibble(high).ok_or_else(|| "Dolphin GDB returned invalid hex".to_string())?;
    let low = nibble(low).ok_or_else(|| "Dolphin GDB returned invalid hex".to_string())?;
    Ok((high << 4) | low)
}

fn write_xfb_png(capture: &PageCapture, xfb: &[u8], path: &Path) -> Result<(), String> {
    let width = capture.width as usize;
    let height = capture.height as usize;
    let stride = capture.stride as usize;
    let expected = stride
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| "page framebuffer byte count overflowed".to_string())?;
    if xfb.len() != expected || width > stride {
        return Err(format!(
            "invalid page framebuffer: {} bytes, expected {expected}",
            xfb.len()
        ));
    }

    let mut rgb = vec![0u8; width * height * 3];
    for y in 0..height {
        let source = &xfb[y * stride * 2..(y + 1) * stride * 2];
        for x in (0..width).step_by(2) {
            let offset = x * 2;
            if offset + 3 >= source.len() {
                break;
            }
            let y0 = source[offset];
            let cb = source[offset + 1];
            let y1 = source[offset + 2];
            let cr = source[offset + 3];
            write_rgb_pixel(&mut rgb, width, x, y, y0, cb, cr);
            if x + 1 < width {
                write_rgb_pixel(&mut rgb, width, x + 1, y, y1, cb, cr);
            }
        }
    }

    let first_pixel = rgb
        .get(..3)
        .ok_or_else(|| "page framebuffer has no pixels".to_string())?;
    if rgb.chunks_exact(3).all(|pixel| pixel == first_pixel) {
        return Err(format!(
            "page {} framebuffer is uniform and contains no visible text",
            capture.page
        ));
    }
    if blank_green_page(&rgb) {
        return Err(format!(
            "page {} framebuffer contains only black and green, with no visible text",
            capture.page
        ));
    }

    let file = File::create(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut encoder = png::Encoder::new(file, capture.width, capture.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|err| format!("{}: {err}", path.display()))?;
    writer
        .write_image_data(&rgb)
        .map_err(|err| format!("{}: {err}", path.display()))
}

fn blank_green_page(rgb: &[u8]) -> bool {
    let mut has_green = false;
    for pixel in rgb.chunks_exact(3) {
        if pixel == [0, 0, 0] {
            continue;
        }
        if pixel[0] == 0 && pixel[2] == 0 && pixel[1] > 0 {
            has_green = true;
        } else {
            return false;
        }
    }
    has_green
}

fn write_rgb_pixel(
    rgb: &mut [u8],
    width: usize,
    x: usize,
    y: usize,
    luminance: u8,
    cb: u8,
    cr: u8,
) {
    let c = i32::from(luminance) - 16;
    let d = i32::from(cb) - 128;
    let e = i32::from(cr) - 128;
    let red = (298 * c + 409 * e + 128) >> 8;
    let green = (298 * c - 100 * d - 208 * e + 128) >> 8;
    let blue = (298 * c + 516 * d + 128) >> 8;
    let offset = (y * width + x) * 3;
    rgb[offset] = red.clamp(0, 255) as u8;
    rgb[offset + 1] = green.clamp(0, 255) as u8;
    rgb[offset + 2] = blue.clamp(0, 255) as u8;
}

fn connect_usb_gecko() -> Option<TcpStream> {
    for offset in 0..USB_GECKO_PORT_ATTEMPTS {
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, USB_GECKO_FIRST_PORT + offset);
        if let Ok(stream) = TcpStream::connect_timeout(&address.into(), Duration::from_millis(10)) {
            return Some(stream);
        }
    }
    None
}

enum UsbGeckoRead {
    Data(Vec<u8>),
    Closed,
    Error(String),
}

fn spawn_usb_gecko_reader(mut stream: TcpStream) -> Result<Receiver<UsbGeckoRead>, String> {
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|err| format!("failed to configure USB Gecko reader: {err}"))?;
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("dolphin-usb-gecko-reader".to_string())
        .spawn(move || {
            let mut buffer = [0u8; 4096];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => {
                        let _ = sender.send(UsbGeckoRead::Closed);
                        return;
                    }
                    Ok(count) => {
                        if sender
                            .send(UsbGeckoRead::Data(buffer[..count].to_vec()))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(err)
                        if matches!(
                            err.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) => {}
                    Err(err) => {
                        let _ = sender.send(UsbGeckoRead::Error(format!(
                            "USB Gecko transcript read failed: {err}"
                        )));
                        return;
                    }
                }
            }
        })
        .map_err(|err| format!("failed to start USB Gecko reader: {err}"))?;
    Ok(receiver)
}

fn setup_input_pipe(user_dir: &Path) -> Result<File, String> {
    let config_dir = user_dir.join("Config");
    let pipes_dir = user_dir.join("Pipes");
    fs::create_dir_all(&config_dir).map_err(|err| format!("{}: {err}", config_dir.display()))?;
    fs::create_dir_all(&pipes_dir).map_err(|err| format!("{}: {err}", pipes_dir.display()))?;
    let config = format!(
        "[GCPad1]\nDevice = Pipe/0/{INPUT_PIPE_NAME}\nButtons/A = `Pipe/0/{INPUT_PIPE_NAME}:Button A`\nButtons/B = `Pipe/0/{INPUT_PIPE_NAME}:Button B`\nButtons/Start = `Pipe/0/{INPUT_PIPE_NAME}:Button START`\nMain Stick/Up = `Pipe/0/{INPUT_PIPE_NAME}:Axis MAIN Y -`\nMain Stick/Down = `Pipe/0/{INPUT_PIPE_NAME}:Axis MAIN Y +`\nMain Stick/Left = `Pipe/0/{INPUT_PIPE_NAME}:Axis MAIN X -`\nMain Stick/Right = `Pipe/0/{INPUT_PIPE_NAME}:Axis MAIN X +`\nD-Pad/Up = `Pipe/0/{INPUT_PIPE_NAME}:Button D_UP`\nD-Pad/Down = `Pipe/0/{INPUT_PIPE_NAME}:Button D_DOWN`\nD-Pad/Left = `Pipe/0/{INPUT_PIPE_NAME}:Button D_LEFT`\nD-Pad/Right = `Pipe/0/{INPUT_PIPE_NAME}:Button D_RIGHT`\nTriggers/L = `Pipe/0/{INPUT_PIPE_NAME}:Button L`\nTriggers/R = `Pipe/0/{INPUT_PIPE_NAME}:Button R`\nOptions/Always Connected = True\n"
    );
    fs::write(config_dir.join("GCPadNew.ini"), config)
        .map_err(|err| format!("failed to write GCPadNew.ini: {err}"))?;
    let pipe_path = pipes_dir.join(INPUT_PIPE_NAME);
    let c_path = CString::new(pipe_path.as_os_str().as_bytes())
        .map_err(|_| "input pipe path contains a NUL byte".to_string())?;
    let rc = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
    if rc != 0 {
        return Err(format!(
            "failed to create {}: {}",
            pipe_path.display(),
            std::io::Error::last_os_error()
        ));
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&pipe_path)
        .map_err(|err| format!("failed to open {}: {err}", pipe_path.display()))
}

fn stop_child(child: &mut Child) -> Result<Option<i32>, String> {
    if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
        return Ok(status.code());
    }
    unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    if wait_child(child, Duration::from_millis(750))? {
        return Ok(child
            .try_wait()
            .map_err(|err| err.to_string())?
            .and_then(|status| status.code()));
    }
    unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    if wait_child(child, Duration::from_millis(750))? {
        return Ok(child
            .try_wait()
            .map_err(|err| err.to_string())?
            .and_then(|status| status.code()));
    }
    child
        .kill()
        .map_err(|err| format!("failed to kill Dolphin: {err}"))?;
    child
        .wait()
        .map(|status| status.code())
        .map_err(|err| err.to_string())
}

fn wait_child(child: &mut Child, duration: Duration) -> Result<bool, String> {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if child.try_wait().map_err(|err| err.to_string())?.is_some() {
            return Ok(true);
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(false)
}

fn read_probe_log(session_path: &Path, configured_path: &Path) -> Result<Option<String>, String> {
    for path in [session_path, configured_path] {
        match fs::read_to_string(path) {
            Ok(contents) => return Ok(Some(contents)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(format!("{}: {err}", path.display())),
        }
    }
    Ok(None)
}

fn parse_summaries(contents: &str) -> Vec<Summary> {
    contents.lines().filter_map(parse_summary).collect()
}

fn parse_summary(line: &str) -> Option<Summary> {
    let (reported_pass, rest) = if let Some(rest) = line.strip_prefix(SUMMARY_PREFIX_PASS) {
        (true, rest)
    } else {
        (false, line.strip_prefix(SUMMARY_PREFIX_FAIL)?)
    };
    let (passes, fails) = rest.split_once(" fails=")?;
    if passes.is_empty() || fails.is_empty() || fails.contains(char::is_whitespace) {
        return None;
    }
    Some(Summary {
        line: line.to_string(),
        reported_pass,
        passes: passes.parse().ok()?,
        fails: fails.parse().ok()?,
    })
}

fn last_lines(contents: &str, count: usize) -> Vec<String> {
    let lines = contents.lines().collect::<Vec<_>>();
    lines[lines.len().saturating_sub(count)..]
        .iter()
        .map(|line| (*line).to_string())
        .collect()
}

fn list_frames(dir: &Path) -> Result<Vec<PathBuf>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut frames = fs::read_dir(dir)
        .map_err(|err| format!("{}: {err}", dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("png"))
        .collect::<Vec<_>>();
    frames.sort();
    Ok(frames)
}

fn valid_png(path: &Path) -> bool {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    const IEND: &[u8; 12] = b"\0\0\0\0IEND\xaeB`\x82";
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    bytes.starts_with(SIGNATURE) && bytes.ends_with(IEND)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|err| err.to_string())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|err| err.to_string())?;
    fs::write(path, bytes).map_err(|err| format!("{}: {err}", path.display()))
}

fn check_dolphin(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("Dolphin executable not found: {}", path.display()));
    }
    Ok(())
}

fn command_output(command: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|err| format!("failed to run {}: {err}", command.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} {:?} exited with {}",
            command.display(),
            args,
            output.status
        ));
    }
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(text.trim().to_string())
}

fn git_output(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!("git {:?} exited with {}", args, output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    #[test]
    fn exfat_probe_requires_a_private_read_only_sd_image() {
        assert!(
            super::select_probes(&[])
                .unwrap()
                .iter()
                .all(|spec| spec.name != "exfat_sd_probe")
        );
        let selected = super::select_probes(&["exfat_sd_probe".to_string()]).unwrap();
        assert_eq!(selected[0].name, "exfat_sd_probe");
        let options = super::parse_options(&["--probe".into(), "exfat_sd_probe".into()]).unwrap();
        assert!(super::validate_sd_image_selection(&options).is_err());

        let arguments = super::dolphin_arguments(
            std::path::Path::new("private-user"),
            std::path::Path::new("probe.dol"),
            1234,
            None,
            None,
            super::SdLaunch {
                image: Some(std::path::Path::new("private-user/Wii/libogc-probe-sd.raw")),
                ..Default::default()
            },
        );
        assert!(
            arguments
                .iter()
                .any(|arg| arg == "Dolphin.Core.WiiSDCardAllowWrites=false")
        );
        assert!(
            !arguments
                .iter()
                .any(|arg| arg == "Dolphin.Core.WiiSDCardAllowWrites=true")
        );
        assert!(
            arguments
                .iter()
                .any(|arg| arg
                    == "Dolphin.General.WiiSDCardPath=private-user/Wii/libogc-probe-sd.raw")
        );
    }

    #[test]
    fn polling_consumes_complete_lines_once() {
        let mut progress = super::ProbeProgress::default();
        let mut log = String::from("PASS summary passes=3 fails=0");
        progress.update(&log);
        assert_eq!(progress.summaries, 0);
        log.push_str("\r\nOGC_DOLPHIN_CAPTURE_COMP");
        progress.update(&log);
        progress.update(&log);
        assert_eq!(progress.summaries, 1);
        assert!(!progress.complete);
        log.push_str("LETE\nFAIL summary passes=3 fails=1\n");
        progress.update(&log);
        assert!(progress.complete);
        assert_eq!(progress.summaries, 2);
    }

    #[test]
    fn card_format_evidence_requires_marker_removal_and_cleanup() {
        let old_log = "PASS card destructive format final unmount\n";
        assert!(!super::card_format_evidence_complete(old_log));
        let incomplete = "PASS card format marker removed after remount\n";
        assert!(!super::card_format_evidence_complete(incomplete));
        let complete = "PASS card format marker removed after remount\nPASS card destructive format final unmount\n";
        assert!(super::card_format_evidence_complete(complete));
    }
    #[test]
    fn private_card_format_requires_explicit_isolated_probe() {
        let options = super::parse_options(&[
            "--format-private-card".into(),
            "--probe".into(),
            "storage_probe".into(),
        ])
        .unwrap();
        assert!(options.format_private_card);
        assert!(super::validate_format_selection(&options, true).is_ok());
        let gamecube_options = super::parse_options(&[
            "--format-private-card".into(),
            "--probe".into(),
            "card_probe".into(),
        ])
        .unwrap();
        assert!(super::validate_format_selection(&gamecube_options, true).is_ok());
        assert!(super::validate_format_selection(&options, false).is_err());
        let default_suite = super::parse_options(&["--format-private-card".into()]).unwrap();
        assert!(super::validate_format_selection(&default_suite, true).is_err());
        let wrong_probe = super::parse_options(&[
            "--format-private-card".into(),
            "--probe".into(),
            "dvd_probe".into(),
        ])
        .unwrap();
        assert!(super::validate_format_selection(&wrong_probe, true).is_err());
        let (header, command) = super::format_card_boot_arguments(0x8132_fc00, "storage_probe");
        assert_eq!(
            header,
            [0x5f, 0x61, 0x72, 0x67, 0x81, 0x32, 0xfc, 0x00, 0, 0, 0, 57]
        );
        assert_eq!(
            command,
            b"storage_probe.dol\0--destructive-card-format=ERASE-SLOT-A\0"
        );
        let (gamecube_header, gamecube_command) =
            super::format_card_boot_arguments(0x8132_fc00, "card_probe");
        assert_eq!(
            gamecube_command,
            b"card_probe.dol\0--destructive-card-format=ERASE-SLOT-A\0"
        );
        assert_eq!(
            u32::from_be_bytes(gamecube_header[8..].try_into().unwrap()) as usize,
            gamecube_command.len()
        );
    }
    use super::*;

    #[test]
    fn card_hotplug_boot_arguments_select_the_opt_in_probe() {
        let (header, command) = hotplug_card_boot_arguments(0x8132_fc00);
        assert_eq!(
            &header[..8],
            &[0x5f, 0x61, 0x72, 0x67, 0x81, 0x32, 0xfc, 0x00]
        );
        assert_eq!(
            u32::from_be_bytes(header[8..].try_into().unwrap()) as usize,
            command.len()
        );
        assert_eq!(command, b"card_probe.dol\0--card-hotplug\0");
    }

    #[test]
    fn rejects_blank_green_pages_but_accepts_visible_content() {
        assert!(blank_green_page(&[0, 0, 0, 0, 31, 0]));
        assert!(!blank_green_page(&[0, 0, 0, 0, 31, 0, 255, 255, 255]));
        assert!(!blank_green_page(&[0, 0, 0, 0, 0, 0]));
    }

    #[test]
    fn selects_the_standard_fat_probe_with_its_own_dol_and_symbols() {
        let selected = select_probes(&["wii_std_fs_probe".to_string()]).unwrap();
        assert_eq!(selected.len(), 1);
        let root = Path::new("/repo");
        assert_eq!(
            root.join("target/xtask/probes-dolphin/wii_std_fs_probe.dol"),
            dolphin_probe_dol_path(root, selected[0]),
        );
        assert_eq!(
            root.join(
                "target/xtask/wii-std-fs-dolphin-build/powerpc-nintendo-none/release/wii_std_fs_probe"
            ),
            dolphin_probe_elf_path(root, selected[0]),
        );
    }

    #[test]
    fn selects_the_standard_thread_probe_with_its_own_dol_and_elf() {
        let selected = select_probes(&["wii_std_thread_probe".to_string()]).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].timeout_seconds, 120);
        let root = Path::new("/repo");
        assert_eq!(
            root.join("target/xtask/probes-dolphin/wii_std_thread_probe.dol"),
            dolphin_probe_dol_path(root, selected[0]),
        );
        assert_eq!(
            root.join(
                "target/xtask/wii-std-fs-dolphin-build/powerpc-nintendo-none/release/wii_std_thread_probe"
            ),
            dolphin_probe_elf_path(root, selected[0]),
        );
    }

    #[test]
    fn white_framebuffer_pixels_are_not_gdb_error_packets() {
        assert!(!is_gdb_error("EB80EB8010801080"));
        assert!(!is_gdb_error("EB"));
        assert!(is_gdb_error("E01"));
        assert!(is_gdb_error("Eab"));
        assert!(!is_gdb_error("Ezz"));
    }

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "xtask-dolphin-{name}-{}-{}",
            std::process::id(),
            unix_seconds()
        ))
    }

    #[test]
    fn copies_a_valid_wii_nand_into_the_isolated_profile() {
        let root = test_directory("nand-copy");
        let source = root.join("source");
        let destination = root.join("profile/Wii");
        let system_menu = source.join("title/00000001/00000002/content/title.tmd");
        fs::create_dir_all(system_menu.parent().unwrap()).unwrap();
        fs::write(&system_menu, b"menu metadata").unwrap();
        fs::create_dir_all(source.join("shared2/sys")).unwrap();
        fs::write(source.join("shared2/sys/SYSCONF"), b"nand data").unwrap();
        fs::create_dir_all(destination.parent().unwrap()).unwrap();

        copy_nand(&source, &destination).unwrap();

        assert_eq!(
            fs::read(destination.join("title/00000001/00000002/content/title.tmd")).unwrap(),
            b"menu metadata"
        );
        assert_eq!(
            fs::read(destination.join("shared2/sys/SYSCONF")).unwrap(),
            b"nand data"
        );
        assert_eq!(
            fs::read(source.join("shared2/sys/SYSCONF")).unwrap(),
            b"nand data"
        );
        NandProfileCopy::new(destination.clone()).cleanup().unwrap();
        assert!(!destination.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_to_overwrite_or_copy_inside_the_nand_source() {
        let root = test_directory("nand-overlap");
        let system_menu = root.join("title/00000001/00000002/content/title.tmd");
        fs::create_dir_all(system_menu.parent().unwrap()).unwrap();
        fs::write(&system_menu, b"menu metadata").unwrap();

        assert!(copy_nand(&root, &root).unwrap_err().contains("overlap"));
        assert!(system_menu.is_file());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_a_nand_without_system_menu_content() {
        let root = test_directory("nand-missing-menu");
        fs::create_dir_all(&root).unwrap();
        assert!(
            validate_nand_source(&root)
                .unwrap_err()
                .contains("no installed System Menu")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn locates_dolphin_symbols_in_the_probe_build_directory() {
        assert_eq!(
            dolphin_probe_elf_path(
                Path::new("repo"),
                ProbeSpec {
                    name: "aesnd_native_probe",
                    timeout_seconds: 90,
                    input_pipe: false,
                }
            ),
            PathBuf::from(
                "repo/target/xtask/system-probe-build/powerpc-nintendo-none/release/aesnd_native_probe"
            )
        );
    }

    #[test]
    fn parses_pass_and_fail_summaries() {
        assert_eq!(
            parse_summary("PASS summary passes=12 fails=0"),
            Some(Summary {
                line: "PASS summary passes=12 fails=0".to_string(),
                reported_pass: true,
                passes: 12,
                fails: 0
            })
        );
        assert_eq!(
            parse_summary("FAIL summary passes=3 fails=2")
                .unwrap()
                .fails,
            2
        );
    }

    #[test]
    fn rejects_incomplete_or_trailing_summary_data() {
        assert!(parse_summary("PASS summary passes=12").is_none());
        assert!(parse_summary("PASS summary passes=12 fails=0 trailing").is_none());
        assert!(parse_summary("INFO PASS summary passes=12 fails=0").is_none());
    }

    #[test]
    fn exited_process_keeps_signal_detail() {
        #[cfg(unix)]
        let exit_status = {
            use std::os::unix::process::ExitStatusExt;
            std::process::ExitStatus::from_raw(6).to_string()
        };
        #[cfg(not(unix))]
        let exit_status = "signal: 6 (SIGABRT)".to_string();
        let (status, detail) = classify_probe_completion(
            &Completion::Exited(exit_status.clone()),
            30,
            CompletionEvidence {
                parsed: None,
                frames_valid: false,
                pages_valid: false,
                page_capture_error: None,
                page_frames: 0,
                page_markers: 0,
                summary_count: 0,
            },
        );
        assert_eq!(status, "crash");
        assert!(detail.contains(&exit_status));
        #[cfg(unix)]
        assert!(detail.contains("6"));
    }

    #[test]
    fn timeout_status_takes_priority_over_missing_page_captures() {
        let (status, detail) = classify_probe_completion(
            &Completion::TimedOut,
            30,
            CompletionEvidence {
                parsed: None,
                frames_valid: true,
                pages_valid: false,
                page_capture_error: None,
                page_frames: 0,
                page_markers: 0,
                summary_count: 0,
            },
        );
        assert_eq!(status, "timeout");
        assert!(detail.contains("30 seconds"));
        assert!(detail.contains("captured 0 page PNG(s) for 0 acknowledged page(s)"));
    }

    #[test]
    fn completed_probe_still_fails_when_page_evidence_is_missing() {
        let summary = Summary {
            line: "PASS summary passes=1 fails=0".to_string(),
            reported_pass: true,
            passes: 1,
            fails: 0,
        };
        let (status, _) = classify_probe_completion(
            &Completion::Summary,
            30,
            CompletionEvidence {
                parsed: Some(&summary),
                frames_valid: true,
                pages_valid: false,
                page_capture_error: None,
                page_frames: 0,
                page_markers: 1,
                summary_count: 1,
            },
        );
        assert_eq!(status, "setup_error");
    }

    #[test]
    fn completion_requires_valid_capture_and_a_passing_summary() {
        for (fails, frames_valid, capture_error, expected_status) in [
            (0, true, None, "pass"),
            (1, true, None, "probe_fail"),
            (0, false, None, "setup_error"),
            (0, true, Some("capture failed"), "setup_error"),
        ] {
            let summary = Summary {
                line: "summary".into(),
                reported_pass: fails == 0,
                passes: 1,
                fails,
            };
            let (status, detail) = classify_probe_completion(
                &Completion::Summary,
                30,
                CompletionEvidence {
                    parsed: Some(&summary),
                    frames_valid,
                    pages_valid: true,
                    page_capture_error: capture_error,
                    page_frames: 1,
                    page_markers: 1,
                    summary_count: 1,
                },
            );
            assert_eq!(status, expected_status);
            if let Some(error) = capture_error {
                assert!(detail.contains(error));
            }
        }
    }

    #[test]
    fn completion_rejects_inconsistent_or_empty_summaries() {
        for (line, expected) in [
            ("FAIL summary passes=0 fails=0", "probe_fail"),
            ("FAIL summary passes=3 fails=0", "probe_fail"),
            ("PASS summary passes=3 fails=1", "probe_fail"),
            ("PASS summary passes=0 fails=0", "probe_fail"),
            ("PASS summary passes=1 fails=0", "pass"),
            ("FAIL summary passes=0 fails=1", "probe_fail"),
        ] {
            let summary = parse_summary(line).unwrap();
            let (status, _) = classify_probe_completion(
                &Completion::Summary,
                30,
                CompletionEvidence {
                    parsed: Some(&summary),
                    frames_valid: true,
                    pages_valid: true,
                    page_capture_error: None,
                    page_frames: 1,
                    page_markers: 1,
                    summary_count: 1,
                },
            );
            assert_eq!(status, expected, "{line}");
        }
    }

    #[test]
    fn resolves_boot_stage_from_big_endian_powerpc_elf_symbols() {
        let mut elf = vec![0u8; 228];
        elf[..6].copy_from_slice(b"\x7fELF\x01\x02");
        elf[0x20..0x24].copy_from_slice(&52u32.to_be_bytes());
        elf[0x2e..0x30].copy_from_slice(&40u16.to_be_bytes());
        elf[0x30..0x32].copy_from_slice(&3u16.to_be_bytes());
        let strings_offset = 172u32;
        let symbols_offset = 196u32;
        let strings = b"\0OGC_PROBE_BOOT_STAGE\0";
        let symbols = 32u32;

        let strtab = 52 + 40;
        elf[strtab + 4..strtab + 8].copy_from_slice(&3u32.to_be_bytes());
        elf[strtab + 16..strtab + 20].copy_from_slice(&strings_offset.to_be_bytes());
        elf[strtab + 20..strtab + 24].copy_from_slice(&(strings.len() as u32).to_be_bytes());
        let symtab = 52 + 80;
        elf[symtab + 4..symtab + 8].copy_from_slice(&2u32.to_be_bytes());
        elf[symtab + 16..symtab + 20].copy_from_slice(&symbols_offset.to_be_bytes());
        elf[symtab + 20..symtab + 24].copy_from_slice(&symbols.to_be_bytes());
        elf[symtab + 24..symtab + 28].copy_from_slice(&1u32.to_be_bytes());
        elf[symtab + 36..symtab + 40].copy_from_slice(&16u32.to_be_bytes());

        elf[strings_offset as usize..strings_offset as usize + strings.len()]
            .copy_from_slice(strings);
        let symbol = symbols_offset as usize + 16;
        elf[symbol..symbol + 4].copy_from_slice(&1u32.to_be_bytes());
        elf[symbol + 4..symbol + 8].copy_from_slice(&0x8000_1234u32.to_be_bytes());

        let parsed = parse_elf32_symbols(&elf).unwrap();
        assert_eq!(
            parsed
                .iter()
                .find(|symbol| symbol.name == "OGC_PROBE_BOOT_STAGE"),
            Some(&ElfSymbol {
                address: 0x8000_1234,
                name: "OGC_PROBE_BOOT_STAGE".to_string(),
            })
        );
        assert_eq!(
            nearest_symbol(&parsed, 0x8000_1240),
            Some("OGC_PROBE_BOOT_STAGE+0xc".to_string())
        );
    }

    #[test]
    fn selects_named_probes_in_requested_order() {
        let selected =
            select_probes(&["font_probe".to_string(), "arena_probe".to_string()]).unwrap();
        assert_eq!(
            selected.iter().map(|spec| spec.name).collect::<Vec<_>>(),
            ["font_probe", "arena_probe"]
        );
    }

    #[test]
    fn aggregate_requires_explicit_probe_selection() {
        let selected = select_probes(&["ogc_system_probe".to_string()]).unwrap();
        assert_eq!(selected[0].name, "ogc_system_probe");
        assert!(
            select_probes(&[])
                .unwrap()
                .iter()
                .all(|spec| spec.name != "ogc_system_probe")
        );
    }

    #[test]
    fn dolphin_args_attach_an_explicit_disc_image() {
        let profile = Path::new("/run/profile with spaces");
        let image = Path::new("/run/disc image.iso");
        let args = dolphin_arguments(
            profile,
            Path::new("/run/dvd_probe.dol"),
            1234,
            None,
            Some(image),
            super::SdLaunch::default(),
        );
        assert!(args.windows(2).any(|pair| {
            pair[0] == "-C" && pair[1] == format!("Dolphin.Core.DefaultISO={}", image.display())
        }));
        let options = parse_options(&["--disc-image".into(), image.display().to_string()]).unwrap();
        assert_eq!(options.disc_image.as_deref(), Some(image));
        let without_disc = dolphin_arguments(
            profile,
            Path::new("/run/dvd_probe.dol"),
            1234,
            None,
            None,
            super::SdLaunch::default(),
        );
        assert!(
            !without_disc
                .iter()
                .any(|argument| argument.starts_with("Dolphin.Core.DefaultISO="))
        );
        assert!(parse_options(&["--disc-image".into()]).is_err());
        let error = validate(
            Path::new("."),
            &["--disc-image".into(), image.display().to_string()],
        )
        .unwrap_err();
        assert!(error.contains("explicit --probe"));
        let error = validate(
            Path::new("."),
            &[
                "--disc-image".into(),
                image.display().to_string(),
                "--probe".into(),
                "dvd_probe".into(),
            ],
        )
        .unwrap_err();
        assert!(error.contains("disc image"));
    }

    #[test]
    fn dolphin_args_can_attach_a_memory_card_in_the_private_profile() {
        let profile = Path::new("/run/profile with spaces");
        let card_path = profile.join("GC/MemoryCardA.USA.raw");
        let args = dolphin_arguments(
            profile,
            Path::new("/run/storage_probe.dol"),
            1234,
            Some(&card_path),
            None,
            super::SdLaunch::default(),
        );

        assert!(
            args.windows(2)
                .any(|pair| { pair[0] == "-C" && pair[1] == "Dolphin.Core.SlotA=1" })
        );
        assert!(args.windows(2).any(|pair| {
            pair[0] == "-C"
                && pair[1] == format!("Dolphin.Core.MemcardAPath={}", card_path.display())
        }));

        let args_without_card = dolphin_arguments(
            profile,
            Path::new("/run/storage_probe.dol"),
            1234,
            None,
            None,
            super::SdLaunch::default(),
        );
        assert!(
            !args_without_card
                .iter()
                .any(|argument| argument == "Dolphin.Core.SlotA=1")
        );
        assert!(
            !args_without_card
                .iter()
                .any(|argument| argument.starts_with("Dolphin.Core.MemcardAPath="))
        );
    }

    #[test]
    fn dolphin_args_enable_cpu_efb_access_for_readback_probes() {
        let args = dolphin_arguments(
            Path::new("profile"),
            Path::new("video_probe.dol"),
            1234,
            None,
            None,
            super::SdLaunch::default(),
        );
        assert!(
            args.windows(2).any(|pair| {
                pair[0] == "-C" && pair[1] == "Graphics.Hacks.EFBAccessEnable=True"
            })
        );
    }

    #[test]
    fn dolphin_args_enable_efb_copy_to_ram_for_texture_probes() {
        let args = dolphin_arguments(
            Path::new("profile"),
            Path::new("video_probe.dol"),
            1234,
            None,
            None,
            super::SdLaunch::default(),
        );
        assert!(args.windows(2).any(|pair| {
            pair[0] == "-C" && pair[1] == "Graphics.Hacks.EFBToTextureEnable=False"
        }));
        assert!(args.windows(2).any(|pair| {
            pair[0] == "-C" && pair[1] == "Graphics.Hacks.XFBToTextureEnable=False"
        }));
    }

    #[test]
    fn dolphin_args_disable_virtual_sd_only_when_requested() {
        let profile = Path::new("/run/profile");
        let dol = Path::new("/run/wiisd_probe.dol");
        let enabled = dolphin_arguments(profile, dol, 1234, None, None, super::SdLaunch::default());
        let disabled = dolphin_arguments(
            profile,
            dol,
            1234,
            None,
            None,
            super::SdLaunch {
                enabled: false,
                ..Default::default()
            },
        );
        assert!(
            enabled
                .windows(2)
                .any(|pair| { pair[0] == "-C" && pair[1] == "Dolphin.Core.WiiSDCard=true" })
        );
        assert!(enabled.windows(2).any(|pair| {
            pair[0] == "-C" && pair[1] == "Dolphin.Core.WiiSDCardAllowWrites=true"
        }));
        assert!(
            disabled
                .windows(2)
                .any(|pair| { pair[0] == "-C" && pair[1] == "Dolphin.Core.WiiSDCard=false" })
        );
        assert!(disabled.windows(2).any(|pair| {
            pair[0] == "-C" && pair[1] == "Dolphin.Core.WiiSDCardAllowWrites=false"
        }));
        assert!(
            parse_options(&["--disable-wii-sd-card".to_string()])
                .unwrap()
                .disable_wii_sd_card
        );
        assert!(!parse_options(&[]).unwrap().disable_wii_sd_card);
    }

    #[test]
    fn copies_card_template_into_private_profile_without_changing_source() {
        let root = std::env::temp_dir().join(format!(
            "xtask-card-copy-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("template.raw");
        let profile = root.join("profile");
        let contents = vec![0x5a; 6 * GAMECUBE_CARD_BLOCK_SIZE as usize];
        fs::create_dir_all(&root).unwrap();
        fs::write(&source, &contents).unwrap();

        let destination = copy_card_image_to_profile(&source, &profile).unwrap();

        assert_eq!(destination, profile.join("GC/MemoryCardA.USA.raw"));
        assert_eq!(fs::read(&destination).unwrap(), contents);
        assert_eq!(fs::read(&source).unwrap(), contents);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_non_raw_card_template_sizes() {
        let root = std::env::temp_dir().join(format!(
            "xtask-invalid-card-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("invalid.raw");
        fs::create_dir_all(&root).unwrap();
        fs::write(&source, [0; 64]).unwrap();

        let error = copy_card_image_to_profile(&source, &root.join("profile")).unwrap_err();

        assert!(error.contains("not a raw GameCube memory card image"));
        assert!(!root.join("profile/GC/MemoryCardA.USA.raw").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
