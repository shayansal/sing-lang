use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use sing_codegen::{build_source_to_dir, BuildOptions};

fn temp_dir(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should work")
        .as_nanos();
    std::env::temp_dir().join(format!("sing-codegen-{name}-{nonce}"))
}

#[test]
fn build_report_selects_cranelift_and_emits_artifacts() {
    let dir = temp_dir("report");
    let report = build_source_to_dir(
        "f main()>i4:1+2",
        &dir,
        BuildOptions {
            target: None,
            emit_debug: true,
        },
    )
    .expect("build should succeed");

    assert_eq!(report.backend, "cranelift-alpha");
    assert_eq!(report.abi, "sing-v1-alpha");
    assert!(!report.target.is_empty());
    assert!(report.backend_ir.contains("fn main"));
    assert!(report.object_path.exists());
    assert!(report.binary_path.exists());
    assert!(report.debug_path.as_ref().is_some_and(|path| path.exists()));
    assert!(report
        .layouts
        .iter()
        .any(|layout| layout.name == "i4" && layout.size == 4 && layout.align == 4));
}

#[test]
fn linked_binary_runs_hello_output() {
    let dir = temp_dir("hello");
    let report = build_source_to_dir(
        r#"m H;u io;f main()>v !iw:out("hi")"#,
        &dir,
        BuildOptions {
            target: None,
            emit_debug: true,
        },
    )
    .expect("build should succeed");

    let output = Command::new(&report.binary_path)
        .output()
        .expect("linked binary should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hi");

    let debug = fs::read_to_string(report.debug_path.unwrap()).expect("debug info exists");
    assert!(debug.contains("target"));
    assert!(debug.contains("source_hash"));
}

#[test]
fn direct_cranelift_object_emits_for_constant_integer_main() {
    let dir = temp_dir("direct");
    let report = build_source_to_dir(
        "f main()>i4:1+2*3",
        &dir,
        BuildOptions {
            target: None,
            emit_debug: true,
        },
    )
    .expect("build should succeed");

    assert_eq!(report.codegen_strategy, "cranelift-object-alpha");
    assert!(report.direct_native);
    let direct_object = report
        .direct_object_path
        .as_ref()
        .expect("direct object path should be reported");
    assert!(direct_object.exists());
    assert!(fs::metadata(direct_object).unwrap().len() > 0);
    assert_eq!(report.fallback_reason, None);
    assert!(report
        .backend_ir
        .contains("direct cranelift-object-alpha main=7"));
}

#[test]
fn direct_cranelift_object_emits_for_integer_binds_and_ops() {
    let dir = temp_dir("direct-binds");
    let report = build_source_to_dir(
        "f main()>i4{x=6;y=2;x/y+x%y}",
        &dir,
        BuildOptions {
            target: None,
            emit_debug: true,
        },
    )
    .expect("build should succeed");

    assert_eq!(report.codegen_strategy, "cranelift-object-alpha");
    assert!(report.direct_native);
    assert!(report.direct_object_path.as_ref().is_some_and(|path| {
        path.exists() && fs::metadata(path).is_ok_and(|meta| meta.len() > 0)
    }));
    assert_eq!(report.fallback_reason, None);
    assert!(report
        .backend_ir
        .contains("direct cranelift-object-alpha subset=int-main-v2"));
    assert!(report.backend_ir.contains("ops=2"));
}

#[test]
fn direct_cranelift_object_emits_for_ternary_integer_branches() {
    let dir = temp_dir("direct-ternary");
    let report = build_source_to_dir(
        "f main()>i4:x=4;x>3?9:1",
        &dir,
        BuildOptions {
            target: None,
            emit_debug: true,
        },
    )
    .expect("build should succeed");

    assert_eq!(report.codegen_strategy, "cranelift-object-alpha");
    assert!(report.direct_native);
    assert!(report.direct_object_path.as_ref().is_some_and(|path| {
        path.exists() && fs::metadata(path).is_ok_and(|meta| meta.len() > 0)
    }));
    assert_eq!(report.fallback_reason, None);
    assert!(report
        .backend_ir
        .contains("direct cranelift-object-alpha subset=int-main-v2"));
    assert!(report.backend_ir.contains("blocks=3"));
}

#[test]
fn unsupported_direct_codegen_reports_oracle_fallback_reason() {
    let dir = temp_dir("fallback");
    let report = build_source_to_dir(
        r#"m H;u io;f main()>v !iw:out("hi")"#,
        &dir,
        BuildOptions {
            target: None,
            emit_debug: true,
        },
    )
    .expect("build should succeed");

    assert_eq!(report.codegen_strategy, "oracle-linked-launcher");
    assert!(!report.direct_native);
    assert!(report.direct_object_path.is_none());
    assert!(report
        .fallback_reason
        .as_deref()
        .is_some_and(|reason| reason.contains("unsupported direct Cranelift subset")));
}
