use std::path::Path;
use std::process::Command;

use crate::workspace;

const EXTRA_HOST_TEST_PACKAGES: &[(&str, &str)] = &[
    ("libmad-rs", "vendor/libmad-rs/Cargo.toml"),
    ("mad-sys-host-tests", "tests/mad-sys-host-tests/Cargo.toml"),
    ("ogc-di-host-tests", "tests/ogc-di-host-tests/Cargo.toml"),
    ("ogc-gu-host-tests", "tests/ogc-gu-host-tests/Cargo.toml"),
    ("ogc-gx-host-tests", "tests/ogc-gx-host-tests/Cargo.toml"),
    (
        "ogc-video-host-tests",
        "tests/ogc-video-host-tests/Cargo.toml",
    ),
    (
        "ogc-wiisd-host-tests",
        "tests/ogc-wiisd-host-tests/Cargo.toml",
    ),
];

const STANDALONE_RUST_TESTS: &[&str] = &[
    "tests/wii-std-errors/mappings.rs",
    "tests/wii-std-identity/settings.rs",
    "tests/wii-std-ntlm/vectors.rs",
    "tests/wii-std-paths/adapter.rs",
    "tests/wii-std-stdio/adapter.rs",
];

const PYTHON_TEST_DIRS: &[&str] = &["scripts/tests", "tests/modplay-target-table"];

pub fn run(root: &Path) -> Result<(), String> {
    let packages = host_test_packages(root)?;
    println!("testing {} host-capable package(s)", packages.len());

    for (package, manifest) in packages {
        println!("testing {package}");
        let mut command = Command::new("cargo");
        command
            .arg("test")
            .arg("--manifest-path")
            .arg(root.join(manifest))
            .env("RUSTFLAGS", "--check-cfg=cfg(gamecube) -D warnings")
            .current_dir(root);
        super::run_command(&mut command)?;
    }
    let output_dir = root.join("target/xtask/standalone-host-tests");
    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    for source in STANDALONE_RUST_TESTS {
        println!("testing {source}");
        let name = Path::new(source)
            .parent()
            .and_then(Path::file_name)
            .ok_or_else(|| format!("invalid standalone test path: {source}"))?;
        let binary = output_dir.join(name);
        let mut compile = Command::new("rustc");
        compile
            .args(["--edition=2024", "--test"])
            .arg(root.join(source))
            .arg("-o")
            .arg(&binary)
            .current_dir(root);
        super::run_command(&mut compile)?;
        super::run_command(Command::new(&binary).current_dir(root))?;
    }
    for directory in PYTHON_TEST_DIRS {
        println!("testing Python suite {directory}");
        super::run_command(
            Command::new("python3")
                .args([
                    "-W",
                    "error::ResourceWarning",
                    "-m",
                    "unittest",
                    "discover",
                    "-s",
                    directory,
                ])
                .current_dir(root),
        )?;
    }
    Ok(())
}

fn host_test_packages(root: &Path) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    let mut packages = workspace::load_roles(root)?
        .into_iter()
        .filter(|role| matches!(role.host_policy.as_str(), "abi-only" | "tool"))
        .map(|role| (role.package, role.path.join("Cargo.toml")))
        .collect::<Vec<_>>();
    if packages.is_empty() {
        return Err("crate role manifest has no host-testable packages".to_string());
    }
    packages.extend(
        EXTRA_HOST_TEST_PACKAGES
            .iter()
            .map(|(name, manifest)| (name.to_string(), (*manifest).into())),
    );
    packages.sort();
    packages.dedup_by(|left, right| left.0 == right.0);
    Ok(packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_manifest_selects_all_host_test_packages() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let packages = host_test_packages(root).unwrap();

        let package_names = packages
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        assert!(package_names.contains(&"ogc-card-metadata"));
        assert!(package_names.contains(&"ogc-usb-events"));
        assert!(package_names.contains(&"ogc-usbstorage-lifecycle"));
        assert!(package_names.contains(&"ogc-wiilaunch-state"));
        assert!(package_names.contains(&"xtask"));
        assert!(package_names.contains(&"ogc-gu-host-tests"));
        assert!(package_names.contains(&"ogc-wiisd-host-tests"));
        assert!(!package_names.contains(&"ogc-wiilaunch"));

        let event_tests = packages
            .iter()
            .find(|(name, _)| name == "action-event-tests")
            .unwrap();
        assert_eq!(
            event_tests.1,
            Path::new("examples/action-event-tests/Cargo.toml")
        );
    }

    #[test]
    fn every_standalone_test_suite_is_selected() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let packages = host_test_packages(root).unwrap();
        for entry in std::fs::read_dir(root.join("tests")).unwrap() {
            let entry = entry.unwrap();
            let manifest = entry.path().join("Cargo.toml");
            if manifest.is_file() {
                let relative = manifest.strip_prefix(root).unwrap();
                assert!(
                    packages.iter().any(|(_, selected)| selected == relative),
                    "standalone test suite is absent from test-host: {}",
                    relative.display()
                );
            }
            if entry.file_type().unwrap().is_dir() && !manifest.is_file() {
                for source in std::fs::read_dir(entry.path()).unwrap() {
                    let source = source.unwrap().path();
                    if source.extension().is_some_and(|ext| ext == "rs") {
                        let relative = source.strip_prefix(root).unwrap();
                        assert!(
                            STANDALONE_RUST_TESTS
                                .iter()
                                .any(|selected| Path::new(selected) == relative),
                            "standalone Rust test is absent from test-host: {}",
                            relative.display()
                        );
                    }
                    if source.file_name().is_some_and(|name| {
                        name.to_string_lossy().starts_with("test_")
                            && name.to_string_lossy().ends_with(".py")
                    }) {
                        let relative = source.strip_prefix(root).unwrap();
                        assert!(
                            PYTHON_TEST_DIRS
                                .iter()
                                .any(|dir| relative.parent() == Some(Path::new(dir))),
                            "Python test is absent from test-host: {}",
                            relative.display()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn tinysmb_backend_regressions_are_each_registered_once() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let source = std::fs::read_to_string(
            root.join("crates/compatibility/tinysmb-sys/src/powerpc_backend.rs"),
        )
        .unwrap();
        let file = syn::parse_file(&source).unwrap();
        let tests = file
            .items
            .iter()
            .find_map(|item| match item {
                syn::Item::Mod(module) if module.ident == "tests" => {
                    module.content.as_ref().map(|(_, items)| items)
                }
                _ => None,
            })
            .expect("TinySMB backend test module");
        for name in [
            "connection_lookup_stops_at_first_colon_like_c",
            "duplicate_mount_names_are_detected_only_for_used_mounts",
        ] {
            let function = tests
                .iter()
                .find_map(|item| match item {
                    syn::Item::Fn(function) if function.sig.ident == name => Some(function),
                    _ => None,
                })
                .expect("TinySMB regression function");
            assert_eq!(
                function
                    .attrs
                    .iter()
                    .filter(|attr| attr.path().is_ident("test"))
                    .count(),
                1,
                "{name} must have exactly one test attribute"
            );
        }
    }
}
