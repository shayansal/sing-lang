use std::collections::BTreeMap;

use sing_interp::{run_source, Value};

#[test]
fn executes_struct_literals_and_field_access() {
    let run = run_source("t V{x,y,z:i4};f main()>i4{v=V{x:1,y:2,z:3};v.x+v.y+v.z}")
        .expect("program should run");

    assert_eq!(run.value, Value::Int(6));
}

#[test]
fn executes_enum_unit_values() {
    let run = run_source("e E{Div0};f main()>E:Div0").expect("program should run");

    assert_eq!(
        run.value,
        Value::Enum {
            name: "Div0".to_string(),
            payload: None
        }
    );
}

#[test]
fn executes_range_and_while_loops() {
    let range = run_source("f main()>i4{s=0;l i:1..4 s:=s+i;s}").expect("range loop should run");
    assert_eq!(range.value, Value::Int(6));

    let while_loop =
        run_source("f main()>i4{i=0;s=0;l i<3 {i:=i+1;s:=s+i};s}").expect("while loop should run");
    assert_eq!(while_loop.value, Value::Int(6));
}

#[test]
fn executes_break_and_continue() {
    let run = run_source("f main()>i4{s=0;l 1 {s:=s+1;b;s:=99};s}").expect("break should run");
    assert_eq!(run.value, Value::Int(1));

    let run = run_source("f main()>i4{s=0;l i:0..4 c;s}").expect("continue should run");
    assert_eq!(run.value, Value::Int(0));
}

#[test]
fn executes_option_and_result_flow() {
    let none = run_source("f main()>i4?:none").expect("none should run");
    assert_eq!(none.value, Value::Option(None));

    let ok = run_source("e E{Bad};f maybe()>i4/E{7};f main()>i4/E:maybe()/")
        .expect("result propagation should run");
    assert_eq!(ok.value, Value::ResultOk(Box::new(Value::Int(7))));

    let err = run_source("e E{Bad};f fail()>i4/E{Bad};f main()>i4/E:fail()/")
        .expect("result error propagation should run");
    assert_eq!(
        err.value,
        Value::ResultErr(Box::new(Value::Enum {
            name: "Bad".to_string(),
            payload: None
        }))
    );
}

#[test]
fn executes_sum_builtin_for_tuple_values() {
    let run = run_source("f main()>i4:sum((1,2,3,4))").expect("sum should run");

    assert_eq!(run.value, Value::Int(10));
}

#[test]
fn struct_values_are_stable_json_like_maps() {
    let run = run_source("t V{x,y:i4};f main()>V:V{x:1,y:2}").expect("program should run");

    assert_eq!(
        run.value,
        Value::Struct {
            ty: "V".to_string(),
            fields: BTreeMap::from([
                ("x".to_string(), Value::Int(1)),
                ("y".to_string(), Value::Int(2))
            ])
        }
    );
}
