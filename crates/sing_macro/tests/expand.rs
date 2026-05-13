use sing_macro::expand_source;
use sing_parse::parse_file;
use sing_token::token_cost;

#[test]
fn web_macros_expand_to_parseable_rich_source() {
    let src = include_str!("../../../examples/web.sg");
    let expanded = expand_source(src).expect("macro expansion should succeed");

    assert_eq!(expanded.generated_items, 11);
    assert!(expanded.source.contains("x U_create(v:U)>U !fw"));
    assert!(expanded.source.contains("x U_by_e(x:s)>U[] !fr"));
    assert!(expanded.source.contains("x U_rest()>v !sy"));
    assert!(expanded.source.contains("x rbac_allow(a:*,u:*)>b !ir"));
    parse_file(&expanded.source).expect("expanded source should parse");
    assert!(token_cost(&expanded.source).total > token_cost(src).total);
}

#[test]
fn macro_report_keeps_ai_token_economy_metrics() {
    let src = "t U{id:u8,e:s};p crud U";
    let expanded = expand_source(src).expect("macro expansion should succeed");

    assert_eq!(expanded.contract.language_version, "sing-v1-native-draft.1");
    assert_eq!(expanded.contract.schema_version, "macro.v1");

    let crud = expanded
        .expansions
        .iter()
        .find(|expansion| expansion.name == "crud")
        .expect("crud expansion exists");

    assert_eq!(crud.generated_items, 4);
    assert!(crud.expanded_cost.total > crud.call_cost.total);
    assert!(crud.expansion_ratio_x100 > 100);
}
