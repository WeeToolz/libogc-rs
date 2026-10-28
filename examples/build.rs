use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let voice_dir = manifest_dir.join("letter-flip/mp3");
    let catalog_path = manifest_dir.join("letter-flip/voice-lines.tsv");
    println!("cargo:rerun-if-changed={}", catalog_path.display());
    let catalog = fs::read_to_string(&catalog_path).expect("read Letter Flip voice text");
    let mut expected = std::collections::BTreeSet::new();
    for line in catalog
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#') && *line != "name\ttext")
    {
        let (name, text) = line
            .split_once('\t')
            .expect("voice text needs name and text separated by one tab");
        assert!(
            !text.trim().is_empty() && !text.contains('\t') && valid_voice_name(name),
            "invalid Letter Flip voice text: {line}"
        );
        assert!(
            expected.insert(name),
            "duplicate Letter Flip voice name: {name}"
        );
    }
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("output dir"));
    let generated_voices = out_dir.join("letter_flip_voices.rs");

    println!("cargo:rerun-if-changed={}", voice_dir.display());
    let mut voices = Vec::new();
    if voice_dir.is_dir() {
        for entry in fs::read_dir(&voice_dir).expect("read Letter Flip MP3 directory") {
            let entry = entry.expect("read Letter Flip MP3 entry");
            let path = entry.path();
            if is_mp3(&path) && path.is_file() {
                println!("cargo:rerun-if-changed={}", path.display());
                let name = voice_name(&path);
                if !expected.contains(name.as_str()) {
                    panic!("unsupported Letter Flip voice MP3 name: {}", path.display());
                }
                voices.push((name, path.canonicalize().expect("resolve voice MP3 path")));
            }
        }
    }
    voices.sort_by(|left, right| left.0.cmp(&right.0));
    for pair in voices.windows(2) {
        if pair[0].0 == pair[1].0 {
            panic!("duplicate Letter Flip voice MP3 name: {}", pair[0].0);
        }
    }
    let mut file = fs::File::create(generated_voices).expect("create generated voice list");
    writeln!(file, "static VOICE_CLIPS: &[VoiceClip] = &[").expect("write voice list");
    for (name, path) in voices {
        writeln!(
            file,
            "    VoiceClip {{ name: {:?}, data: include_bytes!({:?}) }},",
            name,
            path.to_string_lossy()
        )
        .expect("write voice MP3 entry");
    }
    writeln!(file, "]; ").expect("finish voice list");
}

fn is_mp3(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp3"))
}

fn voice_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .expect("voice MP3 file name")
        .to_ascii_lowercase()
}

fn valid_voice_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}
