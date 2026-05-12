use sing_parse::{parse_file, parse_file_recovering, parse_file_spanned};

#[test]
fn parse_errors_include_expected_tokens() {
    let errors = parse_file("f main(>v:0").expect_err("source should not parse");
    assert_eq!(errors[0].code, "P0001");
    assert!(
        errors[0].expected.iter().any(|token| token == "identifier"),
        "expected tokens should be stable and machine-readable: {errors:#?}"
    );
}

#[test]
fn spanned_parse_keeps_ast_and_records_core_node_spans() {
    let parsed = parse_file_spanned(r#"m H;f main()>v:out("hi")"#).expect("source should parse");
    assert_eq!(parsed.file.items.len(), 2);
    assert!(parsed.spans.iter().any(|span| span.kind == "file"));
    assert_eq!(
        parsed
            .spans
            .iter()
            .filter(|span| span.kind == "item")
            .count(),
        2
    );
    assert!(parsed.spans.iter().any(|span| span.kind == "fn_sig"));
    assert!(parsed.spans.iter().any(|span| span.kind == "block"));
    assert!(parsed.spans.iter().any(|span| span.kind == "expr"));
}

#[test]
fn string_and_char_escapes_are_decoded() {
    let parsed = parse_file(r#"k S="a\n\"b";k C='\n'"#).expect("escapes should parse");
    let json = serde_json::to_value(parsed).expect("AST should serialize");
    assert_eq!(
        json["items"][0]["kind"]["Const"]["expr"]["String"],
        "a\n\"b"
    );
    assert_eq!(json["items"][1]["kind"]["Const"]["expr"]["Char"], "\n");
}

#[test]
fn byte_strings_parse_without_extra_tokens() {
    let parsed = parse_file(r#"k B=b"abc\n""#).expect("byte strings should parse");
    let json = serde_json::to_value(parsed).expect("AST should serialize");
    assert_eq!(json["items"][0]["kind"]["Const"]["expr"]["String"], "abc\n");
}

#[test]
fn recovering_parse_keeps_valid_items_after_bad_item() {
    let recovered = parse_file_recovering("f bad(>v:0;m Good");
    assert_eq!(recovered.diagnostics.len(), 1);
    assert_eq!(recovered.file.items.len(), 1);
    let json = serde_json::to_value(recovered.file).expect("AST should serialize");
    assert_eq!(json["items"][0]["kind"]["Module"][0], "Good");
}
