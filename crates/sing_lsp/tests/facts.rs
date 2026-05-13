use sing_lsp::{completion_labels, document_facts};

#[test]
fn document_facts_include_symbols_diagnostics_and_token_cost() {
    let facts = document_facts("mem://hello.sg", r#"m H;u io;f main()>v !iw:out("hi")"#);

    assert_eq!(facts.uri, "mem://hello.sg");
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "main"));
    assert!(facts.diagnostics.is_empty());
    assert!(facts.token_cost.total > 0);
}

#[test]
fn completions_are_token_minimal_language_atoms() {
    let labels = completion_labels();

    assert!(labels.contains(&"f".to_string()));
    assert!(labels.contains(&"iw".to_string()));
    assert!(labels.contains(&"out".to_string()));
}
