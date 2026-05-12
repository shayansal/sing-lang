use sing_sem::check_source;

fn assert_clean(src: &str) {
    let checked = check_source(src);
    assert!(
        checked.diagnostics.is_empty(),
        "unexpected diagnostics for `{src}`: {:#?}",
        checked.diagnostics
    );
}

fn assert_has(src: &str, code: &str) {
    let checked = check_source(src);
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code),
        "expected {code} in {:#?} for `{src}`",
        checked.diagnostics
    );
}

#[test]
fn non_copy_values_move_once() {
    assert_has(
        "t Box{x:s};f take(b:Box)>v{};f main()>v{x=Box{x:\"a\"};take(x);take(x)}",
        "E0503",
    );
}

#[test]
fn copy_values_can_be_reused() {
    assert_clean("f take(x:i4)>v{};f main()>v{x=1;take(x);take(x)}");
}

#[test]
fn shared_borrows_can_alias_but_conflict_with_mutable_borrows() {
    assert_clean("f main()>v{x=1;a=&x;b=&x}");
    assert_has("f main()>v{x=1;a=&!x;b=&x}", "E0504");
}

#[test]
fn mutation_while_borrowed_is_rejected() {
    assert_has("f main()>v{x=1;a=&x;x:=2}", "E0505");
}

#[test]
fn references_to_locals_cannot_escape() {
    assert_has("f leak()>&i4{x=1;>&x}", "E0506");
    assert_has("f leak()>&i4{x=1;&x}", "E0506");
}

#[test]
fn parallel_attr_rejects_mutation_and_mutable_borrows() {
    assert_has("P f main()>v{x=1;x:=2}", "E0507");
    assert_has("P f main()>v{x=1;a=&!x}", "E0507");
}
