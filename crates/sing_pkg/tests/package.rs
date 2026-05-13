use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use sing_pkg::{load_package, parse_manifest};

fn temp_dir() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should work")
        .as_nanos();
    std::env::temp_dir().join(format!("sing-pkg-{nonce}"))
}

#[test]
fn manifest_parser_uses_token_minimal_keys() {
    let manifest = parse_manifest("name=\"tiny\"\nversion=\"0.1.0\"\nentry=\"src/main.sg\"\n")
        .expect("manifest should parse");

    assert_eq!(manifest.name, "tiny");
    assert_eq!(manifest.version, "0.1.0");
    assert_eq!(manifest.entry.as_deref(), Some("src/main.sg"));
}

#[test]
fn package_report_is_reproducible_and_infers_runtime_contract() {
    let dir = temp_dir();
    fs::create_dir_all(dir.join("src")).expect("temp dir should be writable");
    fs::write(
        dir.join("Sing.toml"),
        "name=\"tiny\"\nversion=\"0.1.0\"\nentry=\"src/main.sg\"\n",
    )
    .expect("manifest should be writable");
    fs::write(dir.join("src/main.sg"), "RZ f main()>v:0").expect("source should be writable");

    let first = load_package(Path::new(&dir)).expect("package should load");
    let second = load_package(Path::new(&dir)).expect("package should load deterministically");

    assert_eq!(first.lock, second.lock);
    assert_eq!(first.runtime.runtime, "none");
    assert_eq!(first.runtime.panic, "forbidden");
    assert_eq!(first.lock.sources[0].path, "src/main.sg");

    let _ = fs::remove_dir_all(dir);
}
