use sing_interp::{run_source, Value};

#[test]
fn runs_hello_main_and_captures_output() {
    let run = run_source(r#"m H;u io;f main()>v !iw:out("hi")"#).expect("program should run");
    assert_eq!(run.output, "hi");
    assert_eq!(run.value, Value::Void);
}

#[test]
fn evaluates_arithmetic_and_local_binds() {
    let run = run_source("f main()>i4:x=2*3;x+1").expect("program should run");
    assert_eq!(run.value, Value::Int(7));
}

#[test]
fn returns_early_from_function() {
    let run = run_source("f main()>i4:>4;9").expect("program should run");
    assert_eq!(run.value, Value::Int(4));
}
