use sing_sem::{check_package, Namespace};

#[test]
fn package_imports_make_module_functions_visible() {
    let checked = check_package([
        ("a.sg", "m A;f foo()>i4:1"),
        ("b.sg", "m B;u A;f bar()>i4:foo()"),
    ]);

    assert!(
        checked.diagnostics.is_empty(),
        "diagnostics: {:#?}",
        checked.diagnostics
    );
    assert!(checked.ok);
    assert!(checked
        .symbols
        .iter()
        .any(|symbol| symbol.qualified == "A.foo" && symbol.namespace == Namespace::Value));
    assert!(checked
        .symbols
        .iter()
        .any(|symbol| symbol.qualified == "B.bar" && symbol.namespace == Namespace::Value));
}

#[test]
fn package_imports_make_types_consts_and_variants_visible() {
    let checked = check_package([
        ("a.sg", "m A;t V{x:i4};e E{Ok};k num:i4=3"),
        ("b.sg", "m B;u A;f value()>i4{v=V{x:num};Ok;v.x}"),
    ]);

    assert!(
        checked.diagnostics.is_empty(),
        "diagnostics: {:#?}",
        checked.diagnostics
    );
    assert!(checked.ok);
    assert!(checked
        .symbols
        .iter()
        .any(|symbol| symbol.qualified == "A.V" && symbol.namespace == Namespace::Type));
    assert!(checked
        .symbols
        .iter()
        .any(|symbol| symbol.qualified == "A.num" && symbol.namespace == Namespace::Value));
    assert!(checked
        .symbols
        .iter()
        .any(|symbol| symbol.qualified == "A.Ok" && symbol.namespace == Namespace::Variant));
}

#[test]
fn symbol_ids_are_stable_across_runs() {
    let sources = [
        ("a.sg", "m A;t V{x:f4};f foo()>i4:1"),
        ("b.sg", "m B;u A;f bar()>i4:foo()"),
    ];

    let first = check_package(sources);
    let second = check_package(sources);
    let first_ids = first
        .symbols
        .iter()
        .map(|symbol| (symbol.qualified.clone(), symbol.id))
        .collect::<Vec<_>>();
    let second_ids = second
        .symbols
        .iter()
        .map(|symbol| (symbol.qualified.clone(), symbol.id))
        .collect::<Vec<_>>();

    assert_eq!(first_ids, second_ids);
}

#[test]
fn package_reports_import_cycles() {
    let checked = check_package([
        ("a.sg", "m A;u B;f a()>i4:1"),
        ("b.sg", "m B;u A;f b()>i4:1"),
    ]);

    assert!(!checked.ok);
    assert_eq!(checked.diagnostics[0].code, "E0801");
}
