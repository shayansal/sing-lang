use sing_ast::*;
use sing_parse::{parse_file, ParseError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("parse failed: {0:?}")]
    Parse(Vec<ParseError>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pretty,
    Min,
}

pub fn format_source(src: &str) -> Result<String, FormatError> {
    let file = parse_file(src).map_err(FormatError::Parse)?;
    Ok(print_file(&file, Mode::Pretty))
}

pub fn minify_source(src: &str) -> Result<String, FormatError> {
    let file = parse_file(src).map_err(FormatError::Parse)?;
    Ok(print_file(&file, Mode::Min))
}

pub fn minify_file(file: &File) -> String {
    print_file(file, Mode::Min)
}

pub fn format_file(file: &File) -> String {
    print_file(file, Mode::Pretty)
}

pub fn print_expr(expr: &Expr) -> String {
    expr_with_prec(expr, 0, Mode::Min)
}

fn print_file(file: &File, mode: Mode) -> String {
    let mut out = file
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            print_item_ctx(item, mode, mode == Mode::Min && idx + 1 < file.items.len())
        })
        .collect::<Vec<_>>()
        .join(match mode {
            Mode::Pretty => ";\n",
            Mode::Min => ";",
        });
    if mode == Mode::Pretty && !out.is_empty() {
        out.push('\n');
    }
    out
}

fn print_item_ctx(item: &Item, mode: Mode, force_block_body: bool) -> String {
    let attrs = item
        .attrs
        .iter()
        .map(|attr| attr.0.as_str())
        .collect::<String>();
    let prefix = if attrs.is_empty() {
        String::new()
    } else {
        format!("{attrs} ")
    };

    match &item.kind {
        ItemKind::Module(path) => format!("{prefix}m {}", path_s(path)),
        ItemKind::Use(paths) => format!(
            "{prefix}u {}",
            paths.iter().map(path_s).collect::<Vec<_>>().join(sep(mode))
        ),
        ItemKind::Alias { name, ty } => format!("{prefix}a {}={}", name.0, type_s(ty, mode)),
        ItemKind::Const { name, ty, expr } => {
            let ty = ty
                .as_ref()
                .map(|ty| format!(":{}", type_s(ty, mode)))
                .unwrap_or_default();
            format!("{prefix}k {}{ty}={}", name.0, expr_s(expr, mode))
        }
        ItemKind::Type(decl) => format!(
            "{prefix}t {}{}{}",
            decl.name.0,
            generics_s(&decl.generics, mode),
            fields_s(&decl.fields, mode)
        ),
        ItemKind::Enum(decl) => format!(
            "{prefix}e {}{}{{{}}}",
            decl.name.0,
            generics_s(&decl.generics, mode),
            decl.variants
                .iter()
                .map(|variant| variant_s(variant, mode))
                .collect::<Vec<_>>()
                .join(sep(mode))
        ),
        ItemKind::Trait(decl) => format!(
            "{prefix}q {}{}{{{}}}",
            decl.name.0,
            generics_s(&decl.generics, mode),
            decl.fns
                .iter()
                .map(|sig| format!("f {}", fn_sig_s(sig, mode)))
                .collect::<Vec<_>>()
                .join(match mode {
                    Mode::Pretty => "; ",
                    Mode::Min => ";",
                })
        ),
        ItemKind::Impl(decl) => format!(
            "{prefix}i {} {}{{{}}}",
            decl.trait_name.0,
            type_s(&decl.for_type, mode),
            decl.items
                .iter()
                .map(|item| print_item_ctx(item, mode, false))
                .collect::<Vec<_>>()
                .join(match mode {
                    Mode::Pretty => "; ",
                    Mode::Min => ";",
                })
        ),
        ItemKind::Fn(decl) => {
            let body = decl
                .body
                .as_ref()
                .map(|body| {
                    if force_block_body {
                        block_s(body, mode)
                    } else {
                        block_body_s(body, mode)
                    }
                })
                .unwrap_or_default();
            format!("{prefix}f {}{body}", fn_sig_s(&decl.sig, mode))
        }
        ItemKind::Extern(sig) => format!("{prefix}x {}", fn_sig_s(sig, mode)),
        ItemKind::Macro(call) => {
            let args = if call.args.is_empty() {
                String::new()
            } else {
                format!(
                    " {}",
                    call.args
                        .iter()
                        .map(|expr| expr_s(expr, mode))
                        .collect::<Vec<_>>()
                        .join(sep(mode))
                )
            };
            format!("{prefix}p {}{args}", call.name.0)
        }
        ItemKind::Test(test) => {
            let name = test.name.as_ref().map(|name| name.0.as_str()).unwrap_or("");
            format!("{prefix}#{name}:{}", expr_s(&test.expr, mode))
        }
    }
}

fn fn_sig_s(sig: &FnSig, mode: Mode) -> String {
    let ret = sig
        .ret
        .as_ref()
        .map(|ty| format!(">{}", type_s(ty, mode)))
        .unwrap_or_default();
    let effects = if sig.effects.is_empty() {
        String::new()
    } else {
        format!(
            " !{}",
            sig.effects
                .iter()
                .map(|effect| effect.0.as_str())
                .collect::<Vec<_>>()
                .join(sep(mode))
        )
    };
    format!(
        "{}{}({}){ret}{effects}",
        path_s(&sig.name),
        generics_s(&sig.generics, mode),
        sig.params
            .iter()
            .map(|param| param_s(param, mode))
            .collect::<Vec<_>>()
            .join(sep(mode))
    )
}

fn param_s(param: &Param, mode: Mode) -> String {
    format!(
        "{}:{}",
        id_list_s(&param.names, mode),
        type_s(&param.ty, mode)
    )
}

fn field_s(field: &Field, mode: Mode) -> String {
    format!(
        "{}:{}",
        id_list_s(&field.names, mode),
        type_s(&field.ty, mode)
    )
}

fn fields_s(fields: &[Field], mode: Mode) -> String {
    format!(
        "{{{}}}",
        fields
            .iter()
            .map(|field| field_s(field, mode))
            .collect::<Vec<_>>()
            .join(sep(mode))
    )
}

fn variant_s(variant: &Variant, mode: Mode) -> String {
    match variant {
        Variant::Unit(name) => name.0.clone(),
        Variant::Tuple(name, ty) => format!("{}:{}", name.0, type_s(ty, mode)),
        Variant::Struct(name, fields) => format!("{}{}", name.0, fields_s(fields, mode)),
    }
}

fn generics_s(generics: &[Generic], mode: Mode) -> String {
    if generics.is_empty() {
        return String::new();
    }
    format!(
        "[{}]",
        generics
            .iter()
            .map(|generic| match &generic.bound {
                Some(bound) => format!("{}:{}", generic.name.0, type_s(bound, mode)),
                None => generic.name.0.clone(),
            })
            .collect::<Vec<_>>()
            .join(sep(mode))
    )
}

fn block_body_s(block: &Block, mode: Mode) -> String {
    if block.stmts.len() == 1 {
        return format!(":{}", stmt_s(&block.stmts[0], mode));
    }
    match mode {
        Mode::Pretty => format!(
            "{{\n  {}\n}}",
            block
                .stmts
                .iter()
                .map(|stmt| stmt_s(stmt, mode))
                .collect::<Vec<_>>()
                .join(";\n  ")
        ),
        Mode::Min => format!(
            "{{{}}}",
            block
                .stmts
                .iter()
                .map(|stmt| stmt_s(stmt, mode))
                .collect::<Vec<_>>()
                .join(";")
        ),
    }
}

fn block_s(block: &Block, mode: Mode) -> String {
    match mode {
        Mode::Pretty => format!(
            "{{\n  {}\n}}",
            block
                .stmts
                .iter()
                .map(|stmt| stmt_s(stmt, mode))
                .collect::<Vec<_>>()
                .join(";\n  ")
        ),
        Mode::Min => format!(
            "{{{}}}",
            block
                .stmts
                .iter()
                .map(|stmt| stmt_s(stmt, mode))
                .collect::<Vec<_>>()
                .join(";")
        ),
    }
}

fn stmt_s(stmt: &Stmt, mode: Mode) -> String {
    match stmt {
        Stmt::Require(expr) => format!("?{}", expr_s(expr, mode)),
        Stmt::Ensure(expr) => format!("~{}", expr_s(expr, mode)),
        Stmt::Return(expr) => format!(">{}", expr_s(expr, mode)),
        Stmt::LoopRange {
            var,
            start,
            end,
            body,
        } => format!(
            "l {}:{}..{} {}",
            var.0,
            expr_s(start, mode),
            expr_s(end, mode),
            stmt_s(body, mode)
        ),
        Stmt::LoopWhile { cond, body } => {
            format!("l {} {}", expr_s(cond, mode), block_s(body, mode))
        }
        Stmt::Break => "b".to_string(),
        Stmt::Continue => "c".to_string(),
        Stmt::Bind { name, ty, expr } => {
            let ty = ty
                .as_ref()
                .map(|ty| format!(":{}", type_s(ty, mode)))
                .unwrap_or_default();
            format!("{}{ty}={}", name.0, expr_s(expr, mode))
        }
        Stmt::Mut { place, expr } => format!("{}:={}", place_s(place, mode), expr_s(expr, mode)),
        Stmt::Expr(expr) => expr_s(expr, mode),
    }
}

fn type_s(ty: &Type, mode: Mode) -> String {
    match ty {
        Type::Prim(prim) => prim_s(prim).to_string(),
        Type::Path(path) => path_s(path),
        Type::Ref { mutable, inner } => {
            let bang = if *mutable { "!" } else { "" };
            format!("&{bang}{}", type_s(inner, mode))
        }
        Type::Raw(inner) => format!("^{}", type_s(inner, mode)),
        Type::Slice(inner) => format!("{}[]", type_s(inner, mode)),
        Type::Array { inner, len } => format!("{}[{}]", type_s(inner, mode), expr_s(len, mode)),
        Type::Option(inner) => format!("{}?", type_s(inner, mode)),
        Type::Result { ok, err } => format!("{}/{}", type_s(ok, mode), type_s(err, mode)),
        Type::Tuple(types) => format!(
            "({})",
            types
                .iter()
                .map(|ty| type_s(ty, mode))
                .collect::<Vec<_>>()
                .join(sep(mode))
        ),
        Type::Fn { params, ret } => format!(
            "({})>{}",
            params
                .iter()
                .map(|ty| type_s(ty, mode))
                .collect::<Vec<_>>()
                .join(sep(mode)),
            type_s(ret, mode)
        ),
        Type::Dyn(Some(path)) => format!("*{}", path_s(path)),
        Type::Dyn(None) => "*".to_string(),
    }
}

fn expr_s(expr: &Expr, mode: Mode) -> String {
    expr_with_prec(expr, 0, mode)
}

fn expr_with_prec(expr: &Expr, parent: u8, mode: Mode) -> String {
    let prec = expr_prec(expr);
    let text = match expr {
        Expr::Missing => "_".to_string(),
        Expr::Ident(name) => name.0.clone(),
        Expr::Path(path) => path_s(path),
        Expr::Int(value) | Expr::Float(value) => value.clone(),
        Expr::String(value) => format!("\"{}\"", escape_string(value)),
        Expr::Char(value) => format!("'{}'", escape_char(*value)),
        Expr::Bool(value) => value.to_string(),
        Expr::None => "_".to_string(),
        Expr::Tuple(values) => format!(
            "({})",
            values
                .iter()
                .map(|expr| expr_s(expr, mode))
                .collect::<Vec<_>>()
                .join(sep(mode))
        ),
        Expr::Object { ty, fields } => format!(
            "{}{{{}}}",
            path_s(ty),
            fields
                .iter()
                .map(|(name, expr)| {
                    if matches!(expr, Expr::Ident(ident) if ident == name) {
                        name.0.clone()
                    } else {
                        format!("{}:{}", name.0, expr_s(expr, mode))
                    }
                })
                .collect::<Vec<_>>()
                .join(sep(mode))
        ),
        Expr::Field { base, name } => {
            format!("{}.{}", expr_with_prec(base, prec, mode), name.0)
        }
        Expr::Index { base, index } => {
            format!(
                "{}[{}]",
                expr_with_prec(base, prec, mode),
                expr_s(index, mode)
            )
        }
        Expr::Call { callee, args } => format!(
            "{}({})",
            expr_with_prec(callee, prec, mode),
            args.iter()
                .map(|arg| expr_s(arg, mode))
                .collect::<Vec<_>>()
                .join(sep(mode))
        ),
        Expr::Unary { op, expr } => {
            format!("{}{}", unary_op_s(op), expr_with_prec(expr, prec, mode))
        }
        Expr::Binary { op, lhs, rhs } => format!(
            "{}{}{}",
            expr_with_prec(lhs, prec, mode),
            binary_op_s(op),
            expr_with_prec(rhs, prec + 1, mode)
        ),
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "{}?{}:{}",
            expr_with_prec(cond, prec, mode),
            expr_s(then_expr, mode),
            expr_s(else_expr, mode)
        ),
        Expr::Propagate(expr) => format!("{}/", expr_with_prec(expr, prec, mode)),
        Expr::Lambda { params, body } => format!(
            "\\({})>{}",
            params
                .iter()
                .map(|param| param_s(param, mode))
                .collect::<Vec<_>>()
                .join(sep(mode)),
            expr_s(body, mode)
        ),
    };
    if prec < parent {
        format!("({text})")
    } else {
        text
    }
}

fn expr_prec(expr: &Expr) -> u8 {
    match expr {
        Expr::Propagate(_) => 10,
        Expr::Ternary { .. } => 20,
        Expr::Binary { op, .. } => match op {
            BinaryOp::Or => 30,
            BinaryOp::And => 40,
            BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
            | BinaryOp::Eq
            | BinaryOp::Ne => 50,
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::AddWrap
            | BinaryOp::SubWrap
            | BinaryOp::AddSat
            | BinaryOp::SubSat
            | BinaryOp::AddChk
            | BinaryOp::SubChk => 60,
            BinaryOp::Mul
            | BinaryOp::Div
            | BinaryOp::Mod
            | BinaryOp::MulWrap
            | BinaryOp::MulSat
            | BinaryOp::MulChk => 70,
        },
        Expr::Unary { .. } => 80,
        Expr::Field { .. } | Expr::Index { .. } | Expr::Call { .. } => 90,
        Expr::Missing
        | Expr::Ident(_)
        | Expr::Path(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::String(_)
        | Expr::Char(_)
        | Expr::Bool(_)
        | Expr::None
        | Expr::Tuple(_)
        | Expr::Object { .. }
        | Expr::Lambda { .. } => 100,
    }
}

fn place_s(place: &Place, mode: Mode) -> String {
    match place {
        Place::Ident(name) => name.0.clone(),
        Place::Field { base, name } => format!("{}.{}", expr_s(base, mode), name.0),
        Place::Index { base, index } => format!("{}[{}]", expr_s(base, mode), expr_s(index, mode)),
    }
}

fn path_s(path: &Path) -> String {
    path.0
        .iter()
        .map(|ident| ident.0.as_str())
        .collect::<Vec<_>>()
        .join(".")
}

fn id_list_s(ids: &[Ident], mode: Mode) -> String {
    ids.iter()
        .map(|ident| ident.0.as_str())
        .collect::<Vec<_>>()
        .join(sep(mode))
}

fn sep(mode: Mode) -> &'static str {
    match mode {
        Mode::Pretty => ",",
        Mode::Min => ",",
    }
}

fn prim_s(prim: &Prim) -> &'static str {
    match prim {
        Prim::B => "b",
        Prim::I1 => "i1",
        Prim::I2 => "i2",
        Prim::I4 => "i4",
        Prim::I8 => "i8",
        Prim::Iz => "iz",
        Prim::U1 => "u1",
        Prim::U2 => "u2",
        Prim::U4 => "u4",
        Prim::U8 => "u8",
        Prim::Uz => "uz",
        Prim::F4 => "f4",
        Prim::F8 => "f8",
        Prim::C => "c",
        Prim::S => "s",
        Prim::Y => "y",
        Prim::V => "v",
        Prim::Any => "*",
    }
}

fn unary_op_s(op: &UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "~",
        UnaryOp::Ref => "&",
        UnaryOp::RefMut => "&!",
        UnaryOp::Raw => "^",
    }
}

fn binary_op_s(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Mod => "%",
        BinaryOp::AddWrap => "+%",
        BinaryOp::SubWrap => "-%",
        BinaryOp::MulWrap => "*%",
        BinaryOp::AddSat => "+|",
        BinaryOp::SubSat => "-|",
        BinaryOp::MulSat => "*|",
        BinaryOp::AddChk => "+?",
        BinaryOp::SubChk => "-?",
        BinaryOp::MulChk => "*?",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::Eq => "=",
        BinaryOp::Ne => "!=",
        BinaryOp::And => "&",
        BinaryOp::Or => "|",
    }
}

fn escape_string(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect(),
            '\n' => "\\n".chars().collect(),
            '\r' => "\\r".chars().collect(),
            '\t' => "\\t".chars().collect(),
            other => vec![other],
        })
        .collect()
}

fn escape_char(value: char) -> String {
    match value {
        '\\' => "\\\\".to_string(),
        '\'' => "\\'".to_string(),
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        other => other.to_string(),
    }
}
