use std::process::Command;

#[test]
fn run_command_executes_hello() {
    let bin = env!("CARGO_BIN_EXE_sing");
    let output = Command::new(bin)
        .args(["run", "../../examples/hello.sg"])
        .output()
        .expect("sing binary should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi");
}
