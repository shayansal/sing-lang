use std::ops::Range;

use sing_ast::*;
use thiserror::Error;

use crate::{
    lex,
    lexer::{Tok, Token},
    pratt,
};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message} at {start}..{end}")]
pub struct ParseError {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

type PResult<T> = Result<T, ParseError>;

pub fn parse_file(src: &str) -> Result<File, Vec<ParseError>> {
    let tokens = lex(src).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| ParseError {
                message: error.message,
                start: error.span.start,
                end: error.span.end,
            })
            .collect::<Vec<_>>()
    })?;

    Parser::new(tokens)
        .parse_file()
        .map_err(|error| vec![error])
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_file(mut self) -> PResult<File> {
        let mut items = Vec::new();
        self.skip_semis();
        while !self.at_eof() {
            items.push(self.parse_item()?);
            self.skip_semis();
        }
        Ok(File { items })
    }

    fn parse_item(&mut self) -> PResult<Item> {
        let mut attrs = Vec::new();
        while let Tok::Attr(attr) = self.peek().clone() {
            self.bump();
            attrs.push(Attr(attr));
        }

        let kind = match self.peek().clone() {
            Tok::Id(keyword) if keyword == "m" => {
                self.bump();
                ItemKind::Module(self.parse_path()?)
            }
            Tok::Id(keyword) if keyword == "u" => {
                self.bump();
                let mut paths = vec![self.parse_path()?];
                while self.eat_sym(',') {
                    paths.push(self.parse_path()?);
                }
                ItemKind::Use(paths)
            }
            Tok::Id(keyword) if keyword == "a" => {
                self.bump();
                let name = self.expect_ident()?;
                self.expect_sym('=')?;
                let ty = self.parse_type()?;
                ItemKind::Alias { name, ty }
            }
            Tok::Id(keyword) if keyword == "k" => {
                self.bump();
                let name = self.expect_ident()?;
                let ty = if self.eat_sym(':') {
                    Some(self.parse_type()?)
                } else {
                    None
                };
                self.expect_sym('=')?;
                let expr = self.parse_expr()?;
                ItemKind::Const { name, ty, expr }
            }
            Tok::Id(keyword) if keyword == "t" => {
                self.bump();
                ItemKind::Type(self.parse_type_decl()?)
            }
            Tok::Id(keyword) if keyword == "e" => {
                self.bump();
                ItemKind::Enum(self.parse_enum_decl()?)
            }
            Tok::Id(keyword) if keyword == "q" => {
                self.bump();
                ItemKind::Trait(self.parse_trait_decl()?)
            }
            Tok::Id(keyword) if keyword == "i" => {
                self.bump();
                ItemKind::Impl(self.parse_impl_decl()?)
            }
            Tok::Id(keyword) if keyword == "f" => {
                self.bump();
                let sig = self.parse_fn_sig()?;
                let body = if self.check_sym(':') || self.check_sym('{') {
                    Some(self.parse_body()?)
                } else {
                    None
                };
                ItemKind::Fn(FnDecl { sig, body })
            }
            Tok::Id(keyword) if keyword == "x" => {
                self.bump();
                ItemKind::Extern(self.parse_fn_sig()?)
            }
            Tok::Id(keyword) if keyword == "p" => {
                self.bump();
                ItemKind::Macro(self.parse_macro_call()?)
            }
            Tok::Sym('#') => {
                self.bump();
                ItemKind::Test(self.parse_test_decl()?)
            }
            _ => return self.err("expected top-level item"),
        };

        Ok(Item { attrs, kind })
    }

    fn parse_type_decl(&mut self) -> PResult<TypeDecl> {
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        let fields = self.parse_fields()?;
        Ok(TypeDecl {
            name,
            generics,
            fields,
        })
    }

    fn parse_enum_decl(&mut self) -> PResult<EnumDecl> {
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        self.expect_sym('{')?;
        let mut variants = Vec::new();
        if !self.eat_sym('}') {
            loop {
                variants.push(self.parse_variant()?);
                if self.eat_sym(',') {
                    if self.eat_sym('}') {
                        break;
                    }
                } else {
                    self.expect_sym('}')?;
                    break;
                }
            }
        }
        Ok(EnumDecl {
            name,
            generics,
            variants,
        })
    }

    fn parse_trait_decl(&mut self) -> PResult<TraitDecl> {
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        self.expect_sym('{')?;
        let mut fns = Vec::new();
        while !self.check_sym('}') && !self.at_eof() {
            self.skip_semis();
            self.eat_id("f");
            if self.check_sym('}') {
                break;
            }
            fns.push(self.parse_fn_sig()?);
            self.skip_semis();
        }
        self.expect_sym('}')?;
        Ok(TraitDecl {
            name,
            generics,
            fns,
        })
    }

    fn parse_impl_decl(&mut self) -> PResult<ImplDecl> {
        let trait_name = self.expect_ident()?;
        let for_type = self.parse_type()?;
        self.expect_sym('{')?;
        let mut items = Vec::new();
        self.skip_semis();
        while !self.check_sym('}') && !self.at_eof() {
            items.push(self.parse_item()?);
            self.skip_semis();
        }
        self.expect_sym('}')?;
        Ok(ImplDecl {
            trait_name,
            for_type,
            items,
        })
    }

    fn parse_macro_call(&mut self) -> PResult<MacroCall> {
        let name = self.expect_ident()?;
        let mut args = Vec::new();

        if self.check_sym('{') {
            args.push(self.parse_object_for_path(Path(vec![name.clone()]))?);
        } else {
            while !self.at_item_boundary() {
                args.push(self.parse_expr()?);
                if !self.eat_sym(',') {
                    break;
                }
            }
        }

        Ok(MacroCall { name, args })
    }

    fn parse_test_decl(&mut self) -> PResult<TestDecl> {
        let name = if matches!(self.peek(), Tok::Id(_)) && !self.check_next_sym(':') {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect_sym(':')?;
        let expr = self.parse_expr()?;
        Ok(TestDecl { name, expr })
    }

    fn parse_fn_sig(&mut self) -> PResult<FnSig> {
        let name = self.parse_path()?;
        let generics = self.parse_generics()?;
        let params = self.parse_params()?;
        let ret = if self.eat_sym('>') {
            Some(self.parse_type()?)
        } else {
            None
        };
        let effects = if self.eat_sym('!') {
            let mut effects = vec![Effect(self.expect_ident()?.0)];
            while self.eat_sym(',') {
                effects.push(Effect(self.expect_ident()?.0));
            }
            effects
        } else {
            Vec::new()
        };
        Ok(FnSig {
            name,
            generics,
            params,
            ret,
            effects,
        })
    }

    fn parse_params(&mut self) -> PResult<Vec<Param>> {
        self.expect_sym('(')?;
        let mut params = Vec::new();
        if self.eat_sym(')') {
            return Ok(params);
        }
        loop {
            let names = self.parse_id_list()?;
            self.expect_sym(':')?;
            let ty = self.parse_type()?;
            params.push(Param { names, ty });
            if self.eat_sym(',') {
                if self.eat_sym(')') {
                    break;
                }
            } else {
                self.expect_sym(')')?;
                break;
            }
        }
        Ok(params)
    }

    fn parse_body(&mut self) -> PResult<Block> {
        if self.eat_sym(':') {
            let stmts = self.parse_stmt_seq(false)?;
            Ok(Block { stmts })
        } else {
            self.parse_block()
        }
    }

    fn parse_block(&mut self) -> PResult<Block> {
        self.expect_sym('{')?;
        let stmts = self.parse_stmt_seq(true)?;
        self.expect_sym('}')?;
        Ok(Block { stmts })
    }

    fn parse_stmt_seq(&mut self, in_block: bool) -> PResult<Vec<Stmt>> {
        let mut stmts = Vec::new();
        self.skip_semis();
        while !self.at_eof() && !self.check_sym('}') {
            stmts.push(self.parse_stmt()?);
            if self.eat_sym(';') {
                self.skip_semis();
                continue;
            }
            if !in_block {
                break;
            }
            if !self.check_sym('}') {
                return self.err("expected `;` between statements");
            }
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> PResult<Stmt> {
        if self.eat_sym('?') {
            return Ok(Stmt::Require(self.parse_expr()?));
        }
        if self.eat_sym('~') {
            return Ok(Stmt::Ensure(self.parse_expr()?));
        }
        if self.eat_sym('>') {
            return Ok(Stmt::Return(self.parse_expr()?));
        }
        if self.check_id("l") {
            return self.parse_loop_stmt();
        }
        if self.check_id("b") && self.next_is_stmt_boundary() {
            self.bump();
            return Ok(Stmt::Break);
        }
        if self.check_id("c") && self.next_is_stmt_boundary() {
            self.bump();
            return Ok(Stmt::Continue);
        }

        if matches!(self.peek(), Tok::Id(_)) && self.check_next_sym(':') {
            let name = self.expect_ident()?;
            self.expect_sym(':')?;
            let ty = self.parse_type()?;
            self.expect_sym('=')?;
            let expr = self.parse_expr()?;
            return Ok(Stmt::Bind {
                name,
                ty: Some(ty),
                expr,
            });
        }

        let saved = self.pos;
        if let Ok(place) = self.parse_place() {
            if self.eat_op(":=") {
                let expr = self.parse_expr()?;
                return Ok(Stmt::Mut { place, expr });
            }
        }
        self.pos = saved;

        if matches!(self.peek(), Tok::Id(_)) && self.check_next_sym('=') {
            let name = self.expect_ident()?;
            self.expect_sym('=')?;
            let expr = self.parse_expr()?;
            return Ok(Stmt::Bind {
                name,
                ty: None,
                expr,
            });
        }

        Ok(Stmt::Expr(self.parse_expr()?))
    }

    fn parse_loop_stmt(&mut self) -> PResult<Stmt> {
        self.expect_id("l")?;
        if matches!(self.peek(), Tok::Id(_)) && self.check_next_sym(':') {
            let var = self.expect_ident()?;
            self.expect_sym(':')?;
            let start = self.parse_expr()?;
            self.expect_op("..")?;
            let end = self.parse_expr()?;
            let body = self.parse_stmt()?;
            Ok(Stmt::LoopRange {
                var,
                start,
                end,
                body: Box::new(body),
            })
        } else {
            let cond = self.parse_expr()?;
            let body = self.parse_block()?;
            Ok(Stmt::LoopWhile { cond, body })
        }
    }

    fn parse_place(&mut self) -> PResult<Place> {
        let ident = self.expect_ident()?;
        let mut place = Place::Ident(ident);
        loop {
            if self.eat_sym('.') {
                let name = self.expect_ident()?;
                let base = Box::new(place_to_expr(place));
                place = Place::Field { base, name };
            } else if self.eat_sym('[') {
                let index = self.parse_expr()?;
                self.expect_sym(']')?;
                let base = Box::new(place_to_expr(place));
                place = Place::Index { base, index };
            } else {
                break;
            }
        }
        Ok(place)
    }

    fn parse_type(&mut self) -> PResult<Type> {
        let mut ty = self.parse_type_prefix()?;
        loop {
            if self.eat_sym('[') {
                if self.eat_sym(']') {
                    ty = Type::Slice(Box::new(ty));
                } else {
                    let len = self.parse_expr()?;
                    self.expect_sym(']')?;
                    ty = Type::Array {
                        inner: Box::new(ty),
                        len,
                    };
                }
            } else if self.eat_sym('?') {
                ty = Type::Option(Box::new(ty));
            } else if self.eat_sym('/') {
                let err = self.parse_type()?;
                ty = Type::Result {
                    ok: Box::new(ty),
                    err: Box::new(err),
                };
            } else {
                break;
            }
        }
        Ok(ty)
    }

    fn parse_type_prefix(&mut self) -> PResult<Type> {
        if self.eat_sym('&') {
            let mutable = self.eat_sym('!');
            return Ok(Type::Ref {
                mutable,
                inner: Box::new(self.parse_type()?),
            });
        }
        if self.eat_sym('^') {
            return Ok(Type::Raw(Box::new(self.parse_type()?)));
        }
        if self.eat_sym('*') {
            if matches!(self.peek(), Tok::Id(_)) {
                return Ok(Type::Dyn(Some(self.parse_path()?)));
            }
            return Ok(Type::Prim(Prim::Any));
        }
        if self.eat_sym('(') {
            let types = if self.eat_sym(')') {
                Vec::new()
            } else {
                let mut types = vec![self.parse_type()?];
                while self.eat_sym(',') {
                    if self.check_sym(')') {
                        break;
                    }
                    types.push(self.parse_type()?);
                }
                self.expect_sym(')')?;
                types
            };
            if self.eat_sym('>') {
                let ret = self.parse_type()?;
                return Ok(Type::Fn {
                    params: types,
                    ret: Box::new(ret),
                });
            }
            return Ok(Type::Tuple(types));
        }

        let path = self.parse_path()?;
        if path.0.len() == 1 {
            if let Some(prim) = primitive_for(&path.0[0].0) {
                return Ok(Type::Prim(prim));
            }
        }
        Ok(Type::Path(path))
    }

    fn parse_generics(&mut self) -> PResult<Vec<Generic>> {
        let mut generics = Vec::new();
        if !self.eat_sym('[') {
            return Ok(generics);
        }
        if self.eat_sym(']') {
            return Ok(generics);
        }
        loop {
            let name = self.expect_ident()?;
            let bound = if self.eat_sym(':') {
                Some(self.parse_type()?)
            } else {
                None
            };
            generics.push(Generic { name, bound });
            if self.eat_sym(',') {
                if self.eat_sym(']') {
                    break;
                }
            } else {
                self.expect_sym(']')?;
                break;
            }
        }
        Ok(generics)
    }

    fn parse_fields(&mut self) -> PResult<Vec<Field>> {
        self.expect_sym('{')?;
        let mut fields = Vec::new();
        if self.eat_sym('}') {
            return Ok(fields);
        }
        loop {
            let names = self.parse_id_list()?;
            self.expect_sym(':')?;
            let ty = self.parse_type()?;
            fields.push(Field { names, ty });
            if self.eat_sym(',') {
                if self.eat_sym('}') {
                    break;
                }
            } else {
                self.expect_sym('}')?;
                break;
            }
        }
        Ok(fields)
    }

    fn parse_variant(&mut self) -> PResult<Variant> {
        let name = self.expect_ident()?;
        if self.eat_sym(':') {
            Ok(Variant::Tuple(name, self.parse_type()?))
        } else if self.check_sym('{') {
            Ok(Variant::Struct(name, self.parse_fields()?))
        } else {
            Ok(Variant::Unit(name))
        }
    }

    fn parse_id_list(&mut self) -> PResult<Vec<Ident>> {
        let mut ids = vec![self.expect_ident()?];
        while self.eat_sym(',') {
            ids.push(self.expect_ident()?);
        }
        Ok(ids)
    }

    fn parse_expr(&mut self) -> PResult<Expr> {
        self.parse_expr_bp(0)
    }

    fn parse_expr_bp(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut lhs = self.parse_prefix_expr()?;
        lhs = self.parse_postfix_expr(lhs)?;

        loop {
            if self.check_sym('?') && pratt::PREC_TERNARY >= min_prec {
                self.bump();
                let then_expr = self.parse_expr_bp(0)?;
                self.expect_sym(':')?;
                let else_expr = self.parse_expr_bp(pratt::PREC_TERNARY)?;
                lhs = Expr::Ternary {
                    cond: Box::new(lhs),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                };
                continue;
            }

            let Some((op_text, is_slash)) = self.current_infix_text() else {
                break;
            };
            if is_slash && !self.next_starts_expr() {
                break;
            }
            let Some(infix) = pratt::infix_for(&op_text) else {
                break;
            };
            if infix.precedence < min_prec {
                break;
            }
            self.bump();
            let rhs = self.parse_expr_bp(infix.precedence + 1)?;
            lhs = Expr::Binary {
                op: infix.op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        if self.check_sym('/') && !self.next_starts_expr() {
            self.bump();
            lhs = Expr::Propagate(Box::new(lhs));
        }

        Ok(lhs)
    }

    fn parse_prefix_expr(&mut self) -> PResult<Expr> {
        if self.eat_sym('-') {
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(self.parse_expr_bp(pratt::PREC_MUL + 1)?),
            });
        }
        if self.eat_sym('~') || self.eat_sym('!') {
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(self.parse_expr_bp(pratt::PREC_MUL + 1)?),
            });
        }
        if self.eat_sym('&') {
            let op = if self.eat_sym('!') {
                UnaryOp::RefMut
            } else {
                UnaryOp::Ref
            };
            return Ok(Expr::Unary {
                op,
                expr: Box::new(self.parse_expr_bp(pratt::PREC_MUL + 1)?),
            });
        }
        if self.eat_sym('^') {
            return Ok(Expr::Unary {
                op: UnaryOp::Raw,
                expr: Box::new(self.parse_expr_bp(pratt::PREC_MUL + 1)?),
            });
        }
        if self.eat_sym('/') {
            if matches!(self.peek(), Tok::Id(_)) {
                return Ok(Expr::Path(self.parse_path()?));
            }
            return Ok(Expr::Missing);
        }
        if self.eat_sym('*') {
            return Ok(Expr::Missing);
        }

        self.parse_primary_expr()
    }

    fn parse_primary_expr(&mut self) -> PResult<Expr> {
        let expr = match self.bump().tok {
            Tok::Id(id) if id == "true" => Expr::Bool(true),
            Tok::Id(id) if id == "false" => Expr::Bool(false),
            Tok::Id(id) if id == "none" || id == "n" => Expr::None,
            Tok::Id(id) => {
                let ident = Ident(id);
                let base = Expr::Ident(ident.clone());
                if self.check_sym('{') {
                    return self.parse_object_for_path(Path(vec![ident]));
                }
                base
            }
            Tok::Int(value) => Expr::Int(value),
            Tok::Float(value) => Expr::Float(value),
            Tok::Str(value) => Expr::String(value),
            Tok::Char(value) => Expr::Char(value),
            Tok::Sym('(') => {
                if self.eat_sym(')') {
                    Expr::Tuple(Vec::new())
                } else {
                    let first = self.parse_expr()?;
                    if self.eat_sym(',') {
                        let mut values = vec![first];
                        while !self.check_sym(')') {
                            values.push(self.parse_expr()?);
                            if !self.eat_sym(',') {
                                break;
                            }
                        }
                        self.expect_sym(')')?;
                        Expr::Tuple(values)
                    } else {
                        self.expect_sym(')')?;
                        first
                    }
                }
            }
            other => return self.err_at(format!("expected expression, found {}", fmt_tok(&other))),
        };
        Ok(expr)
    }

    fn parse_postfix_expr(&mut self, mut expr: Expr) -> PResult<Expr> {
        loop {
            if self.eat_sym('(') {
                let mut args = Vec::new();
                if !self.eat_sym(')') {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.eat_sym(',') {
                            if self.eat_sym(')') {
                                break;
                            }
                        } else {
                            self.expect_sym(')')?;
                            break;
                        }
                    }
                }
                expr = Expr::Call {
                    callee: Box::new(expr),
                    args,
                };
            } else if self.eat_sym('[') {
                let index = self.parse_expr()?;
                self.expect_sym(']')?;
                expr = Expr::Index {
                    base: Box::new(expr),
                    index: Box::new(index),
                };
            } else if self.eat_sym('.') {
                let name = self.expect_ident()?;
                expr = Expr::Field {
                    base: Box::new(expr),
                    name,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_object_for_path(&mut self, ty: Path) -> PResult<Expr> {
        self.expect_sym('{')?;
        let mut fields = Vec::new();
        if self.eat_sym('}') {
            return Ok(Expr::Object { ty, fields });
        }
        loop {
            let name = self.expect_ident()?;
            let value = if self.eat_sym(':') {
                self.parse_expr()?
            } else {
                Expr::Ident(name.clone())
            };
            fields.push((name, value));
            if self.eat_sym(',') {
                if self.eat_sym('}') {
                    break;
                }
            } else {
                self.expect_sym('}')?;
                break;
            }
        }
        Ok(Expr::Object { ty, fields })
    }

    fn parse_path(&mut self) -> PResult<Path> {
        let mut parts = vec![self.expect_ident()?];
        while self.eat_sym('.') {
            parts.push(self.expect_ident()?);
        }
        Ok(Path(parts))
    }

    fn skip_semis(&mut self) {
        while self.eat_sym(';') {}
    }

    fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }

    fn at_item_boundary(&self) -> bool {
        self.at_eof() || self.check_sym(';') || self.check_sym('}')
    }

    fn next_is_stmt_boundary(&self) -> bool {
        matches!(
            self.peek_n(1),
            Tok::Eof | Tok::Sym(';') | Tok::Sym('}') | Tok::Sym(',')
        )
    }

    fn next_starts_expr(&self) -> bool {
        matches!(
            self.peek_n(1),
            Tok::Id(_)
                | Tok::Int(_)
                | Tok::Float(_)
                | Tok::Str(_)
                | Tok::Char(_)
                | Tok::Sym('(' | '-' | '~' | '&' | '^' | '!' | '/' | '*')
        )
    }

    fn current_infix_text(&self) -> Option<(String, bool)> {
        match self.peek() {
            Tok::Sym(ch @ ('*' | '/' | '%' | '+' | '-' | '<' | '>' | '=' | '&' | '|')) => {
                Some((ch.to_string(), *ch == '/'))
            }
            Tok::Op(op)
                if matches!(
                    op.as_str(),
                    "<=" | ">="
                        | "=="
                        | "!="
                        | "+%"
                        | "-%"
                        | "*%"
                        | "+|"
                        | "-|"
                        | "*|"
                        | "+?"
                        | "-?"
                        | "*?"
                ) =>
            {
                Some((op.clone(), false))
            }
            _ => None,
        }
    }

    fn eat_id(&mut self, expected: &str) -> bool {
        if self.check_id(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_id(&mut self, expected: &str) -> PResult<()> {
        if self.eat_id(expected) {
            Ok(())
        } else {
            self.err(format!("expected `{expected}`"))
        }
    }

    fn check_id(&self, expected: &str) -> bool {
        matches!(self.peek(), Tok::Id(id) if id == expected)
    }

    fn expect_ident(&mut self) -> PResult<Ident> {
        match self.bump().tok {
            Tok::Id(id) => Ok(Ident(id)),
            other => self.err_at(format!("expected identifier, found {}", fmt_tok(&other))),
        }
    }

    fn eat_sym(&mut self, expected: char) -> bool {
        if self.check_sym(expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_sym(&mut self, expected: char) -> PResult<()> {
        if self.eat_sym(expected) {
            Ok(())
        } else {
            self.err(format!("expected `{expected}`"))
        }
    }

    fn check_sym(&self, expected: char) -> bool {
        matches!(self.peek(), Tok::Sym(ch) if *ch == expected)
    }

    fn check_next_sym(&self, expected: char) -> bool {
        matches!(self.peek_n(1), Tok::Sym(ch) if *ch == expected)
    }

    fn eat_op(&mut self, expected: &str) -> bool {
        if matches!(self.peek(), Tok::Op(op) if op == expected) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, expected: &str) -> PResult<()> {
        if self.eat_op(expected) {
            Ok(())
        } else {
            self.err(format!("expected `{expected}`"))
        }
    }

    fn peek(&self) -> &Tok {
        &self.tokens[self.pos].tok
    }

    fn peek_n(&self, n: usize) -> &Tok {
        self.tokens
            .get(self.pos + n)
            .map(|token| &token.tok)
            .unwrap_or(&Tok::Eof)
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if !matches!(token.tok, Tok::Eof) {
            self.pos += 1;
        }
        token
    }

    fn err<T>(&self, message: impl Into<String>) -> PResult<T> {
        let span = self.current_span();
        Err(ParseError {
            message: message.into(),
            start: span.start,
            end: span.end,
        })
    }

    fn err_at<T>(&self, message: impl Into<String>) -> PResult<T> {
        self.err(message)
    }

    fn current_span(&self) -> Range<usize> {
        self.tokens
            .get(self.pos)
            .map(|token| token.span.clone())
            .unwrap_or_else(|| 0..0)
    }
}

fn primitive_for(name: &str) -> Option<Prim> {
    match name {
        "b" => Some(Prim::B),
        "i1" => Some(Prim::I1),
        "i2" => Some(Prim::I2),
        "i4" => Some(Prim::I4),
        "i8" => Some(Prim::I8),
        "iz" => Some(Prim::Iz),
        "u1" => Some(Prim::U1),
        "u2" => Some(Prim::U2),
        "u4" => Some(Prim::U4),
        "u8" => Some(Prim::U8),
        "uz" => Some(Prim::Uz),
        "f4" => Some(Prim::F4),
        "f8" => Some(Prim::F8),
        "c" => Some(Prim::C),
        "s" => Some(Prim::S),
        "y" => Some(Prim::Y),
        "v" => Some(Prim::V),
        _ => None,
    }
}

fn place_to_expr(place: Place) -> Expr {
    match place {
        Place::Ident(ident) => Expr::Ident(ident),
        Place::Field { base, name } => Expr::Field { base, name },
        Place::Index { base, index } => Expr::Index {
            base,
            index: Box::new(index),
        },
    }
}

fn fmt_tok(tok: &Tok) -> String {
    match tok {
        Tok::Id(id) => format!("identifier `{id}`"),
        Tok::Attr(attr) => format!("attribute `{attr}`"),
        Tok::Int(value) => format!("int `{value}`"),
        Tok::Float(value) => format!("float `{value}`"),
        Tok::Str(_) => "string".to_string(),
        Tok::Char(ch) => format!("char `{ch}`"),
        Tok::Sym(ch) => format!("`{ch}`"),
        Tok::Op(op) => format!("`{op}`"),
        Tok::Eof => "end of file".to_string(),
    }
}
