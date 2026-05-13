use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sing_ast::{Expr, File, ItemKind, Type};
use sing_parse::{parse_file, ParseError};
use sing_token::{token_cost, TokenCost};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpandedFile {
    pub ok: bool,
    pub generated_items: usize,
    pub source: String,
    pub original_cost: TokenCost,
    pub expanded_cost: TokenCost,
    pub expansion_ratio_x100: usize,
    pub expansions: Vec<MacroExpansion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroExpansion {
    pub name: String,
    pub call_source: String,
    pub generated_source: Vec<String>,
    pub generated_items: usize,
    pub call_cost: TokenCost,
    pub expanded_cost: TokenCost,
    pub expansion_ratio_x100: usize,
}

#[derive(Debug, Error)]
pub enum MacroError {
    #[error("parse failed: {0:?}")]
    Parse(Vec<ParseError>),
    #[error("unknown macro `{0}`")]
    UnknownMacro(String),
    #[error("macro `{name}` expects {expected}")]
    BadArgs { name: String, expected: String },
}

#[derive(Debug, Clone)]
struct TypeInfo {
    fields: HashMap<String, Type>,
}

pub fn expand_source(src: &str) -> Result<ExpandedFile, MacroError> {
    let file = parse_file(src).map_err(MacroError::Parse)?;
    expand_file(&file, src)
}

pub fn expand_file(file: &File, src: &str) -> Result<ExpandedFile, MacroError> {
    let types = collect_types(file);
    let mut base = File { items: Vec::new() };
    let mut expansions = Vec::new();
    let mut generated = Vec::new();

    for item in &file.items {
        match &item.kind {
            ItemKind::Macro(call) => {
                let expansion = expand_macro(&call.name.0, &call.args, &types)?;
                generated.extend(expansion.generated_source.clone());
                expansions.push(expansion);
            }
            _ => base.items.push(item.clone()),
        }
    }

    let mut source = sing_fmt::minify_file(&base);
    for item in &generated {
        if !source.is_empty() {
            source.push(';');
        }
        source.push_str(item);
    }
    let generated_items = expansions
        .iter()
        .map(|expansion| expansion.generated_items)
        .sum();
    let original_cost = token_cost(src);
    let expanded_cost = token_cost(&source);
    let expansion_ratio_x100 = ratio_x100(expanded_cost.total, original_cost.total);

    Ok(ExpandedFile {
        ok: true,
        generated_items,
        source,
        original_cost,
        expanded_cost,
        expansion_ratio_x100,
        expansions,
    })
}

fn expand_macro(
    name: &str,
    args: &[Expr],
    types: &HashMap<String, TypeInfo>,
) -> Result<MacroExpansion, MacroError> {
    let generated_source = match name {
        "store" => {
            let ty = expect_ident_arg(name, args)?;
            vec![
                format!("x {ty}_get(id:u8)>{ty}? !fr"),
                format!("x {ty}_put(v:{ty})>v !fw"),
            ]
        }
        "idx" => {
            let (ty, field) = expect_field_arg(name, args)?;
            let field_ty = types
                .get(&ty)
                .and_then(|info| info.fields.get(&field))
                .map(type_s)
                .unwrap_or_else(|| "*".to_string());
            vec![format!("x {ty}_by_{field}(x:{field_ty})>{ty}[] !fr")]
        }
        "crud" => {
            let ty = expect_ident_arg(name, args)?;
            vec![
                format!("x {ty}_create(v:{ty})>{ty} !fw"),
                format!("x {ty}_read(id:u8)>{ty}? !fr"),
                format!("x {ty}_update(v:{ty})>{ty} !fw"),
                format!("x {ty}_delete(id:u8)>b !fw"),
            ]
        }
        "rest" => {
            let ty = expect_ident_arg(name, args)?;
            vec![format!("x {ty}_rest()>v !sy")]
        }
        "rbac" => {
            expect_one_arg(name, args)?;
            vec![
                "x rbac_allow(a:*,u:*)>b !ir".to_string(),
                "x rbac_role(u:*)>* !ir".to_string(),
                "x rbac_deny()>v !ir".to_string(),
            ]
        }
        other => return Err(MacroError::UnknownMacro(other.to_string())),
    };
    let call_source = call_source(name, args);
    let generated_joined = generated_source.join(";");
    let call_cost = token_cost(&call_source);
    let expanded_cost = token_cost(&generated_joined);
    let generated_items = generated_source.len();
    let expansion_ratio_x100 = ratio_x100(expanded_cost.total, call_cost.total);
    Ok(MacroExpansion {
        name: name.to_string(),
        call_source,
        generated_source,
        generated_items,
        call_cost,
        expanded_cost,
        expansion_ratio_x100,
    })
}

fn collect_types(file: &File) -> HashMap<String, TypeInfo> {
    let mut types = HashMap::new();
    for item in &file.items {
        if let ItemKind::Type(decl) = &item.kind {
            let mut fields = HashMap::new();
            for field in &decl.fields {
                for name in &field.names {
                    fields.insert(name.0.clone(), field.ty.clone());
                }
            }
            types.insert(decl.name.0.clone(), TypeInfo { fields });
        }
    }
    types
}

fn expect_one_arg(name: &str, args: &[Expr]) -> Result<(), MacroError> {
    if args.len() == 1 {
        Ok(())
    } else {
        Err(MacroError::BadArgs {
            name: name.to_string(),
            expected: "one argument".to_string(),
        })
    }
}

fn expect_ident_arg(name: &str, args: &[Expr]) -> Result<String, MacroError> {
    expect_one_arg(name, args)?;
    match &args[0] {
        Expr::Ident(ident) => Ok(ident.0.clone()),
        Expr::Path(path) => Ok(path_s(path)),
        _ => Err(MacroError::BadArgs {
            name: name.to_string(),
            expected: "a type name".to_string(),
        }),
    }
}

fn expect_field_arg(name: &str, args: &[Expr]) -> Result<(String, String), MacroError> {
    expect_one_arg(name, args)?;
    match &args[0] {
        Expr::Field { base, name: field } => match base.as_ref() {
            Expr::Ident(ty) => Ok((ty.0.clone(), field.0.clone())),
            Expr::Path(path) => Ok((path_s(path), field.0.clone())),
            _ => Err(MacroError::BadArgs {
                name: name.to_string(),
                expected: "a type.field path".to_string(),
            }),
        },
        _ => Err(MacroError::BadArgs {
            name: name.to_string(),
            expected: "a type.field path".to_string(),
        }),
    }
}

fn call_source(name: &str, args: &[Expr]) -> String {
    if name == "rbac" {
        if let Some(expr) = args.first() {
            return format!("p {}", expr_with_macro_name(name, expr));
        }
    }
    let args = args
        .iter()
        .map(sing_fmt::print_expr)
        .collect::<Vec<_>>()
        .join(",");
    if args.is_empty() {
        format!("p {name}")
    } else {
        format!("p {name} {args}")
    }
}

fn expr_with_macro_name(name: &str, expr: &Expr) -> String {
    match expr {
        Expr::Object { fields, .. } => format!(
            "{name}{{{}}}",
            fields
                .iter()
                .map(|(field, value)| {
                    if matches!(value, Expr::Missing) {
                        format!("{}:*", field.0)
                    } else {
                        format!("{}:{}", field.0, sing_fmt::print_expr(value))
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => sing_fmt::print_expr(other),
    }
}

fn type_s(ty: &Type) -> String {
    match ty {
        Type::Prim(prim) => match prim {
            sing_ast::Prim::B => "b",
            sing_ast::Prim::I1 => "i1",
            sing_ast::Prim::I2 => "i2",
            sing_ast::Prim::I4 => "i4",
            sing_ast::Prim::I8 => "i8",
            sing_ast::Prim::Iz => "iz",
            sing_ast::Prim::U1 => "u1",
            sing_ast::Prim::U2 => "u2",
            sing_ast::Prim::U4 => "u4",
            sing_ast::Prim::U8 => "u8",
            sing_ast::Prim::Uz => "uz",
            sing_ast::Prim::F4 => "f4",
            sing_ast::Prim::F8 => "f8",
            sing_ast::Prim::C => "c",
            sing_ast::Prim::S => "s",
            sing_ast::Prim::Y => "y",
            sing_ast::Prim::V => "v",
            sing_ast::Prim::Any => "*",
        }
        .to_string(),
        Type::Path(path) => path_s(path),
        Type::Ref { mutable, inner } => {
            let bang = if *mutable { "!" } else { "" };
            format!("&{bang}{}", type_s(inner))
        }
        Type::Raw(inner) => format!("^{}", type_s(inner)),
        Type::Slice(inner) => format!("{}[]", type_s(inner)),
        Type::Array { inner, .. } => format!("{}[]", type_s(inner)),
        Type::Option(inner) => format!("{}?", type_s(inner)),
        Type::Result { ok, err } => format!("{}/{}", type_s(ok), type_s(err)),
        Type::Tuple(types) => format!(
            "({})",
            types.iter().map(type_s).collect::<Vec<_>>().join(",")
        ),
        Type::Fn { params, ret } => format!(
            "({})>{}",
            params.iter().map(type_s).collect::<Vec<_>>().join(","),
            type_s(ret)
        ),
        Type::Dyn(Some(path)) => format!("*{}", path_s(path)),
        Type::Dyn(None) => "*".to_string(),
    }
}

fn path_s(path: &sing_ast::Path) -> String {
    path.0
        .iter()
        .map(|part| part.0.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn ratio_x100(num: usize, den: usize) -> usize {
    num.saturating_mul(100).checked_div(den).unwrap_or(0)
}
