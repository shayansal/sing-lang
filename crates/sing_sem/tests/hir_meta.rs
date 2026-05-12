use sing_sem::check_source;

#[test]
fn checked_hir_contains_spans_and_stable_symbol_metadata() {
    let checked = check_source("m M;t V{x:i4};f main()>i4:1");
    assert!(
        checked.diagnostics.is_empty(),
        "unexpected diagnostics: {:#?}",
        checked.diagnostics
    );

    let hir = checked.hir.expect("expected HIR");
    let names = hir
        .meta
        .iter()
        .filter_map(|meta| meta.name.as_deref())
        .collect::<Vec<_>>();

    assert!(names.contains(&"V"));
    assert!(names.contains(&"main"));
    assert!(hir.meta.iter().all(|meta| meta.symbol.0 > 0));
    assert!(hir
        .meta
        .iter()
        .any(|meta| meta.name.as_deref() == Some("main") && meta.span.start < meta.span.end));
}
