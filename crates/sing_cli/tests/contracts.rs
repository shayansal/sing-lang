use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should work")
        .as_nanos();
    std::env::temp_dir().join(format!("sing-cli-contract-{name}-{nonce}"))
}

fn write_file(dir: &Path, name: &str, src: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("temp dir should be writable");
    let path = dir.join(name);
    fs::write(&path, src).expect("temp file should be writable");
    path
}

fn json_output(command: &mut Command) -> serde_json::Value {
    let output = command.output().expect("command should run");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout should be JSON")
}

#[test]
fn contract_command_reports_single_version_authority() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let json = json_output(Command::new(bin).arg("contract"));

    assert_eq!(json["language_version"], "sing-v1-native-draft.1");
    assert_eq!(json["contract_version"], "contract.v1");
    assert_eq!(json["schemas"]["diagnostic"], "diagnostic.v1");
    assert_eq!(json["schemas"]["check"], "check.v1");
    assert_eq!(json["schemas"]["build"], "build.v1");
    assert_eq!(json["schemas"]["package"], "package.v1");
    assert_eq!(json["schemas"]["token"], "token.v1");
    assert_eq!(
        json["token_economy"]["default_machine_output"],
        "compact-json"
    );
    assert_eq!(json["token_economy"]["feature_gate"], true);
}

#[test]
fn machine_outputs_reference_the_same_contract() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let contract = json_output(Command::new(bin).arg("contract"));
    let language = contract["language_version"].clone();
    let contract_version = contract["contract_version"].clone();

    let dir = temp_dir("surfaces");
    let file = write_file(&dir, "main.sg", "m M;f main()>i4:1+2");
    fs::write(
        dir.join("Sing.toml"),
        "name=\"tiny\"\nversion=\"0.1.0\"\nentry=\"main.sg\"\n",
    )
    .expect("manifest should be writable");

    let check = json_output(Command::new(bin).arg("check").arg(&file));
    assert_eq!(check["contract"]["language_version"], language);
    assert_eq!(check["contract"]["contract_version"], contract_version);
    assert_eq!(check["contract"]["schema_version"], "check.v1");

    let explain = json_output(Command::new(bin).args(["explain", "E0401"]));
    assert_eq!(explain["contract"]["language_version"], language);
    assert_eq!(explain["contract"]["contract_version"], contract_version);
    assert_eq!(explain["contract"]["schema_version"], "diagnostic.v1");

    let build = json_output(
        Command::new(bin)
            .arg("build")
            .arg(&file)
            .arg("--out")
            .arg(dir.join("out")),
    );
    assert_eq!(build["contract"]["language_version"], language);
    assert_eq!(build["contract"]["contract_version"], contract_version);
    assert_eq!(build["contract"]["schema_version"], "build.v1");

    let pkg = json_output(Command::new(bin).arg("pkg").arg(&dir));
    assert_eq!(pkg["contract"]["language_version"], language);
    assert_eq!(pkg["contract"]["contract_version"], contract_version);
    assert_eq!(pkg["contract"]["schema_version"], "package.v1");

    let _ = fs::remove_dir_all(dir);
}
