use sing_hir::{Item, Type};
use sing_sem::check_source;

fn body_type(src: &str, name: &str) -> Type {
    let checked = check_source(src);
    assert!(
        checked.diagnostics.is_empty(),
        "unexpected diagnostics: {:#?}",
        checked.diagnostics
    );

    let hir = checked.hir.expect("expected HIR");
    hir.items
        .iter()
        .find_map(|item| match item {
            Item::Fn(func) if func.sig.name == vec![name.to_string()] => func.body_type.clone(),
            _ => None,
        })
        .expect("expected function body type")
}

fn has_diagnostic(src: &str, code: &str) -> bool {
    let checked = check_source(src);
    checked
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == code)
}

fn const_type(src: &str, name: &str) -> Type {
    let checked = check_source(src);
    assert!(
        checked.diagnostics.is_empty(),
        "unexpected diagnostics: {:#?}",
        checked.diagnostics
    );

    let hir = checked.hir.expect("expected HIR");
    hir.items
        .iter()
        .find_map(|item| match item {
            Item::Const {
                name: item_name,
                ty,
            } if item_name == name => Some(ty.clone()),
            _ => None,
        })
        .expect("expected const type")
}

#[test]
fn generic_identity_instantiates_from_argument() {
    let ty = body_type("f id[T](x:T)>T{x};f main()>i4:id(1)", "main");

    assert_eq!(ty, Type::Prim("i4".to_string()));
}

#[test]
fn generic_refs_instantiate_nested_types() {
    let ty = body_type("f keep[T](x:&T)>&T{x};f main()>&i4:keep(&1)", "main");

    assert_eq!(
        ty,
        Type::Ref {
            mutable: false,
            inner: Box::new(Type::Prim("i4".to_string())),
        }
    );
}

#[test]
fn generic_parameter_mismatch_is_diagnostic() {
    assert!(has_diagnostic(
        "f same[T](a:T,b:T)>T{a};f main()>i4:same(1,\"x\")",
        "E0410"
    ));
}

#[test]
fn generic_bounds_are_resolved_at_declaration() {
    assert!(has_diagnostic("f id[T:Missing](x:T)>T{x}", "E0202"));
}

#[test]
fn integer_literals_use_contextual_numeric_type() {
    let ty = body_type("f main()>u8:1", "main");

    assert_eq!(ty, Type::Prim("u8".to_string()));
}

#[test]
fn numeric_suffixes_infer_literal_types() {
    assert_eq!(const_type("k X=42u4", "X"), Type::Prim("u4".to_string()));
    assert_eq!(const_type("k F=3.14f8", "F"), Type::Prim("f8".to_string()));
}

#[test]
fn tuple_literals_use_contextual_element_types() {
    let ty = body_type("f main()>(u8,i4):(1,2)", "main");

    assert_eq!(
        ty,
        Type::Tuple(vec![
            Type::Prim("u8".to_string()),
            Type::Prim("i4".to_string())
        ])
    );
}

#[test]
fn result_ok_expression_uses_contextual_result_type() {
    let ty = body_type("e E{Bad};f main()>i4/E:1", "main");

    assert_eq!(
        ty,
        Type::Result {
            ok: Box::new(Type::Prim("i4".to_string())),
            err: Box::new(Type::Enum("E".to_string())),
        }
    );
}

#[test]
fn option_none_uses_contextual_option_type() {
    let ty = body_type("f main()>i4?:none", "main");

    assert_eq!(ty, Type::Option(Box::new(Type::Prim("i4".to_string()))));
}

#[test]
fn result_propagation_flows_into_contextual_result_type() {
    let ty = body_type("e E{Bad};f maybe()>i4/E{1};f main()>i4/E:maybe()/", "main");

    assert_eq!(
        ty,
        Type::Result {
            ok: Box::new(Type::Prim("i4".to_string())),
            err: Box::new(Type::Enum("E".to_string())),
        }
    );
}

#[test]
fn raw_pointer_creation_requires_unsafe_attr() {
    assert!(has_diagnostic("f main()>^i4:^1", "E0501"));

    let checked = check_source("U f main()>^i4:^1");
    assert!(
        checked.diagnostics.is_empty(),
        "unexpected diagnostics: {:#?}",
        checked.diagnostics
    );
}

#[test]
fn return_makes_following_statement_unreachable() {
    assert!(has_diagnostic("f main()>i4:>1;2", "E0502"));
}

#[test]
fn star_type_lowers_to_any() {
    let ty = body_type("f main()>*:1", "main");

    assert_eq!(ty, Type::Any);
}
