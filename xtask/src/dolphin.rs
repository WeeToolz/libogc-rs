use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::ops::{Deref, DerefMut};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEFAULT_DOLPHIN: &str = "/Applications/Dolphin.app/Contents/MacOS/Dolphin";
const DEFAULT_DOLPHIN_NAND: &str = "Library/Application Support/Dolphin/Wii";
const SUMMARY_PREFIX_PASS: &str = "PASS summary passes=";
const SUMMARY_PREFIX_FAIL: &str = "FAIL summary passes=";
const INPUT_PIPE_NAME: &str = "libogc-validation";
const PAGE_CAPTURE_PREFIX: &str = "OGC_DOLPHIN_PAGE_CAPTURE ";
const CAPTURE_COMPLETE: &str = "OGC_DOLPHIN_CAPTURE_COMPLETE";
const USB_GECKO_FIRST_PORT: u16 = 0xd6ec;
const USB_GECKO_PORT_ATTEMPTS: u16 = 11;

#[derive(Clone, Copy)]
struct ProbeSpec {
    name: &'static str,
    timeout_seconds: u64,
    input_pipe: bool,
}

const PROBES: &[ProbeSpec] = &[
    ProbeSpec {
        name: "modplay_probe",
        timeout_seconds: 45,
        input_pipe: false,
    },
    ProbeSpec {
        name: "usbstorage_capacity_probe",
        timeout_seconds: 45,
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
        name: "debugger_probe",
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
        name: "wii_std_fs_probe",
        timeout_seconds: 240,
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
    dolphin: Option<PathBuf>,
    nand_dir: Option<PathBuf>,
    probes: Vec<String>,
    artifact_dir: Option<PathBuf>,
    timeout_seconds: Option<u64>,
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
    dolphin_log: String,
    probe_log_tail: Vec<String>,
    dolphin_log_tail: Vec<String>,
    detail: String,
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
    let nand_source = options
        .nand_dir
        .or_else(|| std::env::var_os("DOLPHIN_WII_NAND").map(PathBuf::from))
        .unwrap_or_else(default_nand_path);
    validate_nand_source(&nand_source)?;
    let dolphin = options
        .dolphin
        .or_else(|| std::env::var_os("DOLPHIN_BIN").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DOLPHIN));
    check_dolphin(&dolphin)?;
    let dolphin_version = command_output(&dolphin, &["--version"])?;
    let help = command_output(&dolphin, &["--help"])?;
    for required in ["--user", "--exec", "--batch", "--video_backend", "--config"] {
        if !help.contains(required) {
            return Err(format!(
                "Dolphin {} does not advertise required option {required}",
                dolphin.display()
            ));
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

    let selected = select_probes(&options.probes)?;
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
        let timeout = options.timeout_seconds.unwrap_or(spec.timeout_seconds);
        let result = run_probe(root, &artifact_dir, &dolphin, &nand_source, spec, timeout)?;
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

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--no-build" => index += 1,
            "--dolphin" | "--nand-dir" | "--probe" | "--artifact-dir" | "--timeout-seconds" => {
                let key = args[index].as_str();
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{key} requires a value"))?;
                match key {
                    "--dolphin" => options.dolphin = Some(PathBuf::from(value)),
                    "--nand-dir" => options.nand_dir = Some(PathBuf::from(value)),
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
    eprintln!("  --artifact-dir PATH     result directory");
    eprintln!("  --timeout-seconds N     override every probe timeout");
}

fn select_probes(names: &[String]) -> Result<Vec<ProbeSpec>, String> {
    if names.is_empty() {
        return Ok(PROBES.to_vec());
    }
    names
        .iter()
        .map(|name| {
            PROBES
                .iter()
                .find(|spec| spec.name == name)
                .copied()
                .ok_or_else(|| format!("unknown Dolphin probe: {name}"))
        })
        .collect()
}

fn default_nand_path() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(DEFAULT_DOLPHIN_NAND)
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
    let directory = if spec.name == "wii_std_fs_probe" {
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

fn run_probe(
    root: &Path,
    artifact_dir: &Path,
    dolphin: &Path,
    nand_source: &Path,
    spec: ProbeSpec,
    timeout_seconds: u64,
) -> Result<ProbeResult, String> {
    let probe_dir = artifact_dir.join(spec.name);
    let user_dir = probe_dir.join("user");
    fs::create_dir_all(&user_dir).map_err(|err| format!("{}: {err}", user_dir.display()))?;
    let mut nand_copy = NandProfileCopy::new(user_dir.join("Wii"));
    copy_nand(nand_source, &nand_copy.path)?;
    let isfs_tmp = user_dir.join("Wii/tmp");
    fs::create_dir_all(&isfs_tmp).map_err(|err| format!("{}: {err}", isfs_tmp.display()))?;
    let dol_path = dolphin_probe_dol_path(root, spec);
    if !dol_path.is_file() {
        return Err(format!(
            "Dolphin probe DOL is missing: {}",
            dol_path.display()
        ));
    }
    let symbol_elf = dolphin_probe_elf_path(root, spec);
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
    let arguments = dolphin_arguments(&user_dir, &dol_path, gdb_port);
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
    let mut page_frames = Vec::new();
    let mut page_capture_error = None;
    let mut timeout_diagnostic = None;
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
        let captures = parse_page_captures(&captured_probe_log);
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
        let summaries = parse_summaries(&captured_probe_log);
        if summaries.len() > 1 {
            return Err(format!("probe {} wrote duplicate summaries", spec.name));
        }
        if summaries.len() == 1
            && captured_probe_log
                .lines()
                .any(|line| line == CAPTURE_COMPLETE)
            && page_frames.len() == captures.len()
        {
            completion = Completion::Summary;
            break;
        }
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            completion = Completion::Exited(status.code());
            break;
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
    let (status, detail) = classify_probe_completion(
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
    let result = ProbeResult {
        schema_version: 1,
        probe: spec.name.to_string(),
        status: status.to_string(),
        dol_path: dol_path.display().to_string(),
        dol_sha256: sha256_file(&dol_path)?,
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
    CaptureError,
    Exited(Option<i32>),
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
        Completion::Exited(code) => (
            "crash",
            format!("Dolphin exited before the probe summary with code {code:?}"),
        ),
        Completion::CaptureError => (
            "setup_error",
            format!(
                "page capture failed: {}",
                page_capture_error.unwrap_or("unknown error")
            ),
        ),
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

fn dolphin_arguments(user_dir: &Path, dol_path: &Path, gdb_port: u16) -> Vec<String> {
    let speed = "1.0";
    [
        "--user".to_string(),
        user_dir.display().to_string(),
        "--batch".to_string(),
        "--video_backend=OGL".to_string(),
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
        "Dolphin.Core.WiiSDCard=True".to_string(),
        "-C".to_string(),
        "Dolphin.Core.WiiSDCardAllowWrites=True".to_string(),
        "--exec".to_string(),
        dol_path.display().to_string(),
    ]
    .into_iter()
    .collect()
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

fn parse_page_captures(contents: &str) -> Vec<PageCapture> {
    contents.lines().filter_map(parse_page_capture).collect()
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
    stream: TcpStream,
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
                        stream,
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
        self.continue_execution()
    }

    fn continue_execution(&mut self) -> Result<(), String> {
        self.send_packet("c")
    }

    fn capture_xfb(&mut self, capture: &PageCapture, path: &Path) -> Result<(), String> {
        self.stream
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

        if let Err(error) = self.stream.write_all(&[0x03]) {
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
            .write_all(packet.as_bytes())
            .map_err(|err| format!("Dolphin GDB packet write failed: {err}"))?;
        let mut ack = [0u8; 1];
        self.stream
            .read_exact(&mut ack)
            .map_err(|err| format!("Dolphin GDB acknowledgement read failed: {err}"))?;
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
        let mut byte = [0u8; 1];

        let mut payload = Vec::new();
        loop {
            self.stream
                .read_exact(&mut byte)
                .map_err(|err| format!("Dolphin GDB reply read failed: {err}"))?;
            if byte[0] == b'#' {
                break;
            }
            payload.push(byte[0]);
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
            let _ = self.stream.write_all(b"-");
            return Err(format!(
                "Dolphin GDB reply checksum mismatch: {actual:02x} != {expected:02x}"
            ));
        }
        self.stream
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
    use super::*;

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
}
