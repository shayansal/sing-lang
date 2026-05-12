use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use sing_ast::{
    Attr, BinaryOp, Block, Expr, File, FnDecl, FnSig, Ident, Item, ItemKind, Path, Prim, Stmt,
    Type as AstType, UnaryOp, Variant,
};
use sing_diag::{Diagnostic, Span};
use sing_hir::{
    Field as HirField, Fn as HirFn, FnSig as HirFnSig, Item as HirItem, Param as HirParam, Program,
    Type as HirType, Variant as HirVariant,
};
use sing_parse::{lex, parse_file, Tok};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Namespace {
    Module,
    Type,
    Trait,
    Value,
    Variant,
    Macro,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Symbol {
    pub id: SymbolId,
    pub namespace: Namespace,
    pub module: String,
    pub name: String,
    pub qualified: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedPackage {
    pub ok: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub modules: Vec<String>,
    pub symbols: Vec<Symbol>,
    pub files: Vec<CheckedProgram>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedProgram {
    pub ok: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub hir: Option<Program>,
}

pub fn check_source(src: &str) -> CheckedProgram {
    let spans = SpanIndex::new(src);
    let file = match parse_file(src) {
        Ok(file) => file,
        Err(errors) => {
            let diagnostics = errors
                .into_iter()
                .map(|error| {
                    Diagnostic::error(
                        "E0001",
                        "parse error",
                        Span::new(error.start, error.end),
                        error.message,
                    )
                })
                .collect::<Vec<_>>();
            return CheckedProgram {
                ok: false,
                diagnostics,
                hir: None,
            };
        }
    };

    let mut checker = Checker::new(file, spans);
    let hir = checker.check();
    let ok = checker.diagnostics.is_empty();
    CheckedProgram {
        ok,
        diagnostics: checker.diagnostics,
        hir: ok.then_some(hir),
    }
}

pub fn check_package<const N: usize>(sources: [(&str, &str); N]) -> CheckedPackage {
    let parsed = sources
        .into_iter()
        .map(|(name, src)| {
            let spans = SpanIndex::new(src);
            let file = parse_file(src).map_err(|errors| {
                errors
                    .into_iter()
                    .map(|error| {
                        Diagnostic::error(
                            "E0001",
                            "parse error",
                            Span::new(error.start, error.end),
                            error.message,
                        )
                    })
                    .collect::<Vec<_>>()
            });
            (name.to_string(), src.to_string(), spans, file)
        })
        .collect::<Vec<_>>();

    let mut diagnostics = Vec::new();
    let mut modules = Vec::new();
    let mut imports_by_module: HashMap<String, Vec<String>> = HashMap::new();
    let mut fn_exports: HashMap<String, HashMap<String, FnInfo>> = HashMap::new();
    let mut symbols = Vec::new();

    for (filename, _, _, file_result) in &parsed {
        let Ok(file) = file_result else {
            continue;
        };
        let module =
            module_name(file).unwrap_or_else(|| filename.trim_end_matches(".sg").to_string());
        modules.push(module.clone());
        symbols.push(raw_symbol(&module, Namespace::Module, &module));

        for item in &file.items {
            match &item.kind {
                ItemKind::Use(paths) => {
                    imports_by_module
                        .entry(module.clone())
                        .or_default()
                        .extend(paths.iter().map(path_name));
                }
                ItemKind::Type(decl) => {
                    symbols.push(raw_symbol(&module, Namespace::Type, &decl.name.0));
                }
                ItemKind::Enum(decl) => {
                    symbols.push(raw_symbol(&module, Namespace::Type, &decl.name.0));
                    for variant in &decl.variants {
                        let name = match variant {
                            Variant::Unit(name)
                            | Variant::Tuple(name, _)
                            | Variant::Struct(name, _) => &name.0,
                        };
                        symbols.push(raw_symbol(&module, Namespace::Variant, name));
                    }
                }
                ItemKind::Trait(decl) => {
                    symbols.push(raw_symbol(&module, Namespace::Trait, &decl.name.0));
                }
                ItemKind::Fn(decl) => {
                    let name = path_name(&decl.sig.name);
                    symbols.push(raw_symbol(&module, Namespace::Value, &name));
                    fn_exports
                        .entry(module.clone())
                        .or_default()
                        .insert(name, fn_info_from_sig(&decl.sig));
                }
                ItemKind::Extern(sig) => {
                    let name = path_name(&sig.name);
                    symbols.push(raw_symbol(&module, Namespace::Value, &name));
                    fn_exports
                        .entry(module.clone())
                        .or_default()
                        .insert(name, fn_info_from_sig(sig));
                }
                ItemKind::Const { name, .. } => {
                    symbols.push(raw_symbol(&module, Namespace::Value, &name.0));
                }
                ItemKind::Macro(call) => {
                    symbols.push(raw_symbol(&module, Namespace::Macro, &call.name.0));
                }
                ItemKind::Module(_)
                | ItemKind::Alias { .. }
                | ItemKind::Impl(_)
                | ItemKind::Test(_) => {}
            }
        }
    }

    modules.sort();
    modules.dedup();
    symbols = assign_symbol_ids(symbols);

    for cycle in import_cycles(&imports_by_module) {
        diagnostics.push(Diagnostic::error(
            "E0801",
            format!("import cycle involving `{cycle}`"),
            Span::new(0, 0),
            "module participates in an import cycle",
        ));
    }

    let mut files = Vec::new();
    for (_, _, spans, file_result) in parsed {
        match file_result {
            Ok(file) => {
                let module = module_name(&file).unwrap_or_default();
                let mut imported_fns = HashMap::new();
                for import in imports_by_module.get(&module).into_iter().flatten() {
                    if let Some(exports) = fn_exports.get(import) {
                        imported_fns.extend(exports.clone());
                    }
                }
                let mut checker = Checker::new_with_imports(file, spans, imported_fns);
                let hir = checker.check();
                let ok = checker.diagnostics.is_empty();
                files.push(CheckedProgram {
                    ok,
                    diagnostics: checker.diagnostics.clone(),
                    hir: ok.then_some(hir),
                });
                diagnostics.extend(checker.diagnostics);
            }
            Err(errors) => diagnostics.extend(errors),
        }
    }

    let ok = diagnostics.is_empty();
    CheckedPackage {
        ok,
        diagnostics,
        modules,
        symbols,
        files,
    }
}

#[derive(Debug, Clone)]
struct SpanIndex {
    names: Vec<(String, Span)>,
}

impl SpanIndex {
    fn new(src: &str) -> Self {
        let names = lex(src)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|token| match token.tok {
                Tok::Id(name) | Tok::Attr(name) => {
                    Some((name, Span::new(token.span.start, token.span.end)))
                }
                _ => None,
            })
            .collect();
        Self { names }
    }

    fn name(&self, name: &str) -> Span {
        self.names
            .iter()
            .find_map(|(text, span)| (text == name).then_some(*span))
            .unwrap_or(Span::new(0, 0))
    }
}

#[derive(Debug, Clone)]
enum TypeDef {
    Alias(HirType),
    Struct(HashMap<String, HirType>),
    Enum,
    Trait,
}

#[derive(Debug, Clone)]
struct FnInfo {
    params: Vec<HirType>,
    ret: HirType,
    effects: HashSet<String>,
    generics: Vec<String>,
}

#[derive(Debug, Clone)]
struct Typed {
    ty: HirType,
    effects: HashSet<String>,
}

impl Typed {
    fn pure(ty: HirType) -> Self {
        Self {
            ty,
            effects: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct Scope {
    values: HashMap<String, HirType>,
    generics: HashSet<String>,
    unsafe_allowed: bool,
}

struct Checker {
    file: File,
    spans: SpanIndex,
    diagnostics: Vec<Diagnostic>,
    module: Option<String>,
    imports: Vec<Vec<String>>,
    types: HashMap<String, TypeDef>,
    fns: HashMap<String, FnInfo>,
    consts: HashMap<String, HirType>,
    enum_variants: HashMap<String, HirType>,
}

impl Checker {
    fn new(file: File, spans: SpanIndex) -> Self {
        Self::new_with_imports(file, spans, HashMap::new())
    }

    fn new_with_imports(
        file: File,
        spans: SpanIndex,
        imported_fns: HashMap<String, FnInfo>,
    ) -> Self {
        let mut checker = Self {
            file,
            spans,
            diagnostics: Vec::new(),
            module: None,
            imports: Vec::new(),
            types: HashMap::new(),
            fns: HashMap::new(),
            consts: HashMap::new(),
            enum_variants: HashMap::new(),
        };
        checker.seed_std();
        checker.fns.extend(imported_fns);
        checker
    }

    fn check(&mut self) -> Program {
        self.collect_type_names();
        self.collect_symbols();
        self.lower_program()
    }

    fn seed_std(&mut self) {
        self.fns.insert(
            "out".to_string(),
            FnInfo {
                params: vec![HirType::Prim("s".to_string())],
                ret: HirType::v(),
                effects: HashSet::from(["iw".to_string()]),
                generics: Vec::new(),
            },
        );
        self.fns.insert(
            "sqrt".to_string(),
            FnInfo {
                params: vec![HirType::Prim("f4".to_string())],
                ret: HirType::Prim("f4".to_string()),
                effects: HashSet::new(),
                generics: Vec::new(),
            },
        );
        self.fns.insert(
            "sum".to_string(),
            FnInfo {
                params: vec![HirType::Any],
                ret: HirType::Any,
                effects: HashSet::new(),
                generics: Vec::new(),
            },
        );
    }

    fn collect_type_names(&mut self) {
        let items = self.file.items.clone();
        for item in &items {
            match &item.kind {
                ItemKind::Module(path) => self.module = Some(path_name(path)),
                ItemKind::Use(paths) => {
                    self.imports
                        .extend(paths.iter().map(|path| path.0.iter().map(id).collect()));
                }
                ItemKind::Type(decl) => {
                    self.insert_type(decl.name.0.clone(), TypeDef::Struct(HashMap::new()));
                }
                ItemKind::Enum(decl) => {
                    self.insert_type(decl.name.0.clone(), TypeDef::Enum);
                }
                ItemKind::Trait(decl) => {
                    self.insert_type(decl.name.0.clone(), TypeDef::Trait);
                }
                _ => {}
            }
        }
    }

    fn collect_symbols(&mut self) {
        let items = self.file.items.clone();
        for item in &items {
            match &item.kind {
                ItemKind::Alias { name, ty } => {
                    let scope = Scope {
                        values: HashMap::new(),
                        generics: HashSet::new(),
                        unsafe_allowed: false,
                    };
                    let hir_ty = self.lower_type(ty, &scope);
                    self.insert_type(name.0.clone(), TypeDef::Alias(hir_ty));
                }
                ItemKind::Const { name, ty, expr } => {
                    let scope = Scope {
                        values: HashMap::new(),
                        generics: HashSet::new(),
                        unsafe_allowed: false,
                    };
                    let expected = ty.as_ref().map(|ty| self.lower_type(ty, &scope));
                    let typed = self.check_expr(expr, &scope, expected.as_ref());
                    if let Some(expected) = &expected {
                        self.expect_assignable(expected, &typed.ty, &name.0, "E0401");
                    }
                    self.consts
                        .insert(name.0.clone(), expected.unwrap_or(typed.ty));
                }
                ItemKind::Type(decl) => {
                    let scope = scope_for_generics(&decl.generics);
                    self.validate_generic_bounds(&decl.generics, &scope);
                    let mut fields = HashMap::new();
                    for field in &decl.fields {
                        let ty = self.lower_type(&field.ty, &scope);
                        for name in &field.names {
                            if fields.insert(name.0.clone(), ty.clone()).is_some() {
                                self.error(
                                    "E0102",
                                    format!("duplicate field `{}`", name.0),
                                    &name.0,
                                    "field already exists",
                                );
                            }
                        }
                    }
                    self.types
                        .insert(decl.name.0.clone(), TypeDef::Struct(fields));
                }
                ItemKind::Enum(decl) => {
                    let scope = scope_for_generics(&decl.generics);
                    self.validate_generic_bounds(&decl.generics, &scope);
                    let enum_ty = HirType::Enum(decl.name.0.clone());
                    for variant in &decl.variants {
                        let name = match variant {
                            Variant::Unit(name) => (name.0.clone(), None),
                            Variant::Tuple(name, ty) => {
                                (name.0.clone(), Some(self.lower_type(ty, &scope)))
                            }
                            Variant::Struct(name, fields) => {
                                let payload =
                                    HirType::Struct(format!("{}::{}", decl.name.0, name.0));
                                for field in fields {
                                    self.lower_type(&field.ty, &scope);
                                }
                                (name.0.clone(), Some(payload))
                            }
                        }
                        .0;
                        if self.enum_variants.contains_key(&name) {
                            self.error(
                                "E0103",
                                format!("duplicate enum variant `{name}`"),
                                &name,
                                "variant already exists",
                            );
                        }
                        self.enum_variants.insert(name, enum_ty.clone());
                    }
                    self.types.insert(decl.name.0.clone(), TypeDef::Enum);
                }
                ItemKind::Trait(decl) => {
                    let scope = scope_for_generics(&decl.generics);
                    self.validate_generic_bounds(&decl.generics, &scope);
                    for sig in &decl.fns {
                        self.fn_info(sig);
                    }
                }
                ItemKind::Fn(decl) => {
                    let info = self.fn_info(&decl.sig);
                    self.insert_fn(path_name(&decl.sig.name), info);
                }
                ItemKind::Extern(sig) => {
                    let info = self.fn_info(sig);
                    self.insert_fn(path_name(&sig.name), info);
                }
                _ => {}
            }
        }
    }

    fn lower_program(&mut self) -> Program {
        let mut items = Vec::new();
        let ast_items = self.file.items.clone();
        for item in &ast_items {
            match &item.kind {
                ItemKind::Const { name, ty, expr } => {
                    let scope = Scope {
                        values: HashMap::new(),
                        generics: HashSet::new(),
                        unsafe_allowed: false,
                    };
                    let hir_ty = ty
                        .as_ref()
                        .map(|ty| self.lower_type(ty, &scope))
                        .unwrap_or_else(|| self.check_expr(expr, &scope, None).ty);
                    items.push(HirItem::Const {
                        name: name.0.clone(),
                        ty: hir_ty,
                    });
                }
                ItemKind::Type(decl) => {
                    let fields = self.struct_fields(&decl.name.0);
                    items.push(HirItem::Struct {
                        name: decl.name.0.clone(),
                        fields,
                    });
                }
                ItemKind::Enum(decl) => {
                    let variants = decl
                        .variants
                        .iter()
                        .map(|variant| match variant {
                            Variant::Unit(name) => HirVariant {
                                name: name.0.clone(),
                                payload: None,
                            },
                            Variant::Tuple(name, ty) => HirVariant {
                                name: name.0.clone(),
                                payload: Some(
                                    self.lower_type(ty, &scope_for_generics(&decl.generics)),
                                ),
                            },
                            Variant::Struct(name, _) => HirVariant {
                                name: name.0.clone(),
                                payload: Some(HirType::Struct(format!(
                                    "{}::{}",
                                    decl.name.0, name.0
                                ))),
                            },
                        })
                        .collect();
                    items.push(HirItem::Enum {
                        name: decl.name.0.clone(),
                        variants,
                    });
                }
                ItemKind::Trait(decl) => {
                    let fns = decl.fns.iter().map(|sig| self.lower_fn_sig(sig)).collect();
                    items.push(HirItem::Trait {
                        name: decl.name.0.clone(),
                        fns,
                    });
                }
                ItemKind::Impl(decl) => {
                    let scope = Scope {
                        values: HashMap::new(),
                        generics: HashSet::new(),
                        unsafe_allowed: false,
                    };
                    if !self.types.contains_key(&decl.trait_name.0) {
                        self.error(
                            "E0202",
                            format!("unknown trait `{}`", decl.trait_name.0),
                            &decl.trait_name.0,
                            "trait is not declared",
                        );
                    }
                    items.push(HirItem::Impl {
                        trait_name: decl.trait_name.0.clone(),
                        for_type: self.lower_type(&decl.for_type, &scope),
                        items: decl.items.len(),
                    });
                }
                ItemKind::Fn(decl) => items.push(HirItem::Fn(self.check_fn(item, decl))),
                ItemKind::Extern(sig) => items.push(HirItem::Extern(self.lower_fn_sig(sig))),
                ItemKind::Macro(call) => items.push(HirItem::Macro {
                    name: call.name.0.clone(),
                    arg_count: call.args.len(),
                }),
                ItemKind::Test(test) => {
                    let scope = Scope {
                        values: HashMap::new(),
                        generics: HashSet::new(),
                        unsafe_allowed: false,
                    };
                    let typed = self.check_expr(&test.expr, &scope, None);
                    items.push(HirItem::Test {
                        name: test.name.as_ref().map(id),
                        ty: typed.ty,
                    });
                }
                ItemKind::Module(_) | ItemKind::Use(_) | ItemKind::Alias { .. } => {}
            }
        }

        Program {
            module: self.module.clone(),
            imports: self.imports.clone(),
            items,
        }
    }

    fn check_fn(&mut self, item: &Item, decl: &FnDecl) -> HirFn {
        let sig = self.lower_fn_sig(&decl.sig);
        let mut scope = scope_for_generics(&decl.sig.generics);
        scope.unsafe_allowed = has_attr(&item.attrs, "U");
        for param in &sig.params {
            if scope
                .values
                .insert(param.name.clone(), param.ty.clone())
                .is_some()
            {
                self.error(
                    "E0104",
                    format!("duplicate parameter `{}`", param.name),
                    &param.name,
                    "parameter already exists",
                );
            }
        }

        let body_type = decl.body.as_ref().map(|body| {
            let typed = self.check_block(body, &mut scope, Some(&sig.ret));
            for effect in &typed.effects {
                if !sig.effects.iter().any(|declared| declared == effect) {
                    self.error(
                        "E0602",
                        format!("effect `{effect}` is used but not declared"),
                        &path_name(&decl.sig.name),
                        "add this effect to the function signature",
                    );
                }
            }
            self.enforce_attrs(&item.attrs, body, &path_name(&decl.sig.name));
            let assignable = self.is_assignable(&sig.ret, &typed.ty);
            self.expect_assignable(&sig.ret, &typed.ty, &path_name(&decl.sig.name), "E0401");
            if assignable {
                sig.ret.clone()
            } else {
                typed.ty
            }
        });

        HirFn { sig, body_type }
    }

    fn enforce_attrs(&mut self, attrs: &[Attr], body: &Block, fn_name: &str) {
        let attrs = attrs
            .iter()
            .map(|attr| attr.0.as_str())
            .collect::<HashSet<_>>();
        if attrs.contains("H") && block_contains_string(body) {
            self.error(
                "E0701",
                format!("function `{fn_name}` is marked H but uses a string literal"),
                fn_name,
                "H means no heap-facing literals in v1-alpha.2",
            );
        }
    }

    fn check_block(
        &mut self,
        block: &Block,
        scope: &mut Scope,
        expected: Option<&HirType>,
    ) -> Typed {
        let mut ty = HirType::v();
        let mut effects = HashSet::new();
        let last = block.stmts.len().saturating_sub(1);
        let mut terminated = false;
        for (idx, stmt) in block.stmts.iter().enumerate() {
            if terminated {
                self.error(
                    "E0502",
                    "unreachable statement",
                    stmt_name(stmt),
                    "control flow has already terminated",
                );
                continue;
            }
            let stmt_expected = (idx == last).then_some(expected).flatten();
            let typed = self.check_stmt(stmt, scope, stmt_expected);
            terminated = matches!(typed.ty, HirType::Never);
            ty = typed.ty;
            effects.extend(typed.effects);
        }
        Typed { ty, effects }
    }

    fn check_stmt(&mut self, stmt: &Stmt, scope: &mut Scope, expected: Option<&HirType>) -> Typed {
        match stmt {
            Stmt::Require(expr) | Stmt::Ensure(expr) => {
                let typed = self.check_expr(expr, scope, Some(&HirType::b()));
                if !is_condition(&typed.ty) {
                    self.error(
                        "E0403",
                        "condition must be bool-compatible",
                        expr_name(expr).as_deref().unwrap_or("?"),
                        "this expression is used as a condition",
                    );
                }
                Typed {
                    ty: HirType::v(),
                    effects: typed.effects,
                }
            }
            Stmt::Return(expr) => {
                let typed = self.check_expr(expr, scope, expected);
                if let Some(expected) = expected {
                    self.expect_assignable(expected, &typed.ty, "return", "E0401");
                }
                Typed {
                    ty: HirType::Never,
                    effects: typed.effects,
                }
            }
            Stmt::LoopRange {
                var,
                start,
                end,
                body,
            } => {
                let start_t = self.check_expr(start, scope, None);
                let end_t = self.check_expr(end, scope, None);
                if !is_integer(&start_t.ty) || !is_integer(&end_t.ty) {
                    self.error(
                        "E0404",
                        "range bounds must be integers",
                        &var.0,
                        "loop range",
                    );
                }
                scope
                    .values
                    .insert(var.0.clone(), HirType::Prim("iz".to_string()));
                let body_t = self.check_stmt(body, scope, None);
                let mut effects = start_t.effects;
                effects.extend(end_t.effects);
                effects.extend(body_t.effects);
                Typed {
                    ty: HirType::v(),
                    effects,
                }
            }
            Stmt::LoopWhile { cond, body } => {
                let cond_t = self.check_expr(cond, scope, Some(&HirType::b()));
                let body_t = self.check_block(body, scope, None);
                let mut effects = cond_t.effects;
                effects.extend(body_t.effects);
                Typed {
                    ty: HirType::v(),
                    effects,
                }
            }
            Stmt::Break | Stmt::Continue => Typed::pure(HirType::Never),
            Stmt::Bind { name, ty, expr } => {
                let expected_ty = ty.as_ref().map(|ty| self.lower_type(ty, scope));
                let typed = self.check_expr(expr, scope, expected_ty.as_ref().or(expected));
                let bind_ty = expected_ty.unwrap_or_else(|| typed.ty.clone());
                if let Some(existing) = scope.values.get(&name.0) {
                    let return_position_match =
                        expected.is_some_and(|expected| self.is_assignable(expected, &typed.ty));
                    if !return_position_match && !self.is_assignable(existing, &bind_ty) {
                        self.error(
                            "E0401",
                            format!("cannot bind `{}` with incompatible type", name.0),
                            &name.0,
                            "existing local has a different type",
                        );
                    }
                } else {
                    scope.values.insert(name.0.clone(), bind_ty);
                }
                typed
            }
            Stmt::Mut { place, expr } => {
                let place_ty = match place {
                    sing_ast::Place::Ident(name) => scope
                        .values
                        .get(&name.0)
                        .cloned()
                        .unwrap_or(HirType::Unknown),
                    sing_ast::Place::Field { base, name } => {
                        let base_t = self.check_expr(base, scope, None);
                        self.field_type(&base_t.ty, &name.0)
                    }
                    sing_ast::Place::Index { base, .. } => {
                        let base_t = self.check_expr(base, scope, None);
                        self.index_type(&base_t.ty)
                    }
                };
                let typed = self.check_expr(expr, scope, Some(&place_ty));
                self.expect_assignable(&place_ty, &typed.ty, "mutation", "E0401");
                Typed {
                    ty: HirType::v(),
                    effects: typed.effects,
                }
            }
            Stmt::Expr(expr) => self.check_expr(expr, scope, expected),
        }
    }

    fn check_expr(&mut self, expr: &Expr, scope: &Scope, expected: Option<&HirType>) -> Typed {
        match expr {
            Expr::Missing => Typed::pure(HirType::Any),
            Expr::Ident(name) => {
                if let Some(ty) = scope.values.get(&name.0) {
                    Typed::pure(ty.clone())
                } else if let Some(ty) = self.consts.get(&name.0) {
                    Typed::pure(ty.clone())
                } else if let Some(ty) = self.enum_variants.get(&name.0) {
                    Typed::pure(ty.clone())
                } else if self
                    .imports
                    .iter()
                    .any(|path| path.first() == Some(&name.0))
                {
                    Typed::pure(HirType::Any)
                } else {
                    self.error(
                        "E0201",
                        format!("unknown name `{}`", name.0),
                        &name.0,
                        "name is not in scope",
                    );
                    Typed::pure(HirType::Unknown)
                }
            }
            Expr::Path(path) => {
                let name = path_name(path);
                if let Some(ty) = self.enum_variants.get(&name) {
                    Typed::pure(ty.clone())
                } else if let Some(ty) = self.consts.get(&name) {
                    Typed::pure(ty.clone())
                } else {
                    self.error(
                        "E0201",
                        format!("unknown path `{name}`"),
                        &name,
                        "path not found",
                    );
                    Typed::pure(HirType::Unknown)
                }
            }
            Expr::Int(lit) => Typed::pure(int_literal_type(lit, expected)),
            Expr::Float(lit) => Typed::pure(float_literal_type(lit, expected)),
            Expr::String(_) => Typed::pure(HirType::Prim("s".to_string())),
            Expr::Char(_) => Typed::pure(HirType::Prim("c".to_string())),
            Expr::Bool(_) => Typed::pure(HirType::b()),
            Expr::None => Typed::pure(expected.cloned().unwrap_or(HirType::Any)),
            Expr::Tuple(values) => {
                let mut effects = HashSet::new();
                let expected_items = match expected {
                    Some(HirType::Tuple(types)) if types.len() == values.len() => Some(types),
                    _ => None,
                };
                let types = values
                    .iter()
                    .enumerate()
                    .map(|(idx, expr)| {
                        let expected = expected_items.and_then(|types| types.get(idx));
                        let typed = self.check_expr(expr, scope, expected);
                        effects.extend(typed.effects);
                        typed.ty
                    })
                    .collect();
                Typed {
                    ty: HirType::Tuple(types),
                    effects,
                }
            }
            Expr::Object { ty, fields } => self.check_object(ty, fields, scope),
            Expr::Field { base, name } => {
                let typed = self.check_expr(base, scope, None);
                let ty = self.field_type(&typed.ty, &name.0);
                Typed {
                    ty,
                    effects: typed.effects,
                }
            }
            Expr::Index { base, index } => {
                let base_t = self.check_expr(base, scope, None);
                let index_t = self.check_expr(index, scope, None);
                if !is_integer(&index_t.ty) {
                    self.error(
                        "E0404",
                        "index must be an integer",
                        "[]",
                        "index expression",
                    );
                }
                let mut effects = base_t.effects;
                effects.extend(index_t.effects);
                Typed {
                    ty: self.index_type(&base_t.ty),
                    effects,
                }
            }
            Expr::Call { callee, args } => self.check_call(callee, args, scope),
            Expr::Unary { op, expr } => {
                let typed = self.check_expr(expr, scope, None);
                let ty = match op {
                    UnaryOp::Neg if is_numeric(&typed.ty) => typed.ty.clone(),
                    UnaryOp::Not => HirType::b(),
                    UnaryOp::Ref => HirType::Ref {
                        mutable: false,
                        inner: Box::new(typed.ty.clone()),
                    },
                    UnaryOp::RefMut => HirType::Ref {
                        mutable: true,
                        inner: Box::new(typed.ty.clone()),
                    },
                    UnaryOp::Raw => {
                        if !scope.unsafe_allowed {
                            self.error(
                                "E0501",
                                "raw pointer creation requires `U`",
                                "^",
                                "mark the function with U to create raw pointers",
                            );
                        }
                        HirType::Raw(Box::new(typed.ty.clone()))
                    }
                    UnaryOp::Neg => {
                        self.error("E0405", "negation requires a numeric type", "-", "unary op");
                        HirType::Unknown
                    }
                };
                Typed {
                    ty,
                    effects: typed.effects,
                }
            }
            Expr::Binary { op, lhs, rhs } => self.check_binary(op, lhs, rhs, scope),
            Expr::Ternary {
                cond,
                then_expr,
                else_expr,
            } => {
                let cond_t = self.check_expr(cond, scope, Some(&HirType::b()));
                if !is_condition(&cond_t.ty) {
                    self.error(
                        "E0403",
                        "ternary condition must be bool-compatible",
                        expr_name(cond).as_deref().unwrap_or("?"),
                        "condition expression",
                    );
                }
                let then_t = self.check_expr(then_expr, scope, expected);
                let else_t = self.check_expr(else_expr, scope, expected);
                let ty = self.merge_branch_types(&then_t.ty, &else_t.ty, expected);
                let mut effects = cond_t.effects;
                effects.extend(then_t.effects);
                effects.extend(else_t.effects);
                Typed { ty, effects }
            }
            Expr::Propagate(inner) => {
                let typed = self.check_expr(inner, scope, None);
                let ty = match typed.ty {
                    HirType::Result { ok, .. } => *ok,
                    HirType::Option(inner) => *inner,
                    HirType::Unknown | HirType::Any => HirType::Unknown,
                    other => {
                        self.error(
                            "E0406",
                            format!(
                                "cannot propagate non-result type `{}`",
                                display_type(&other)
                            ),
                            "/",
                            "postfix propagation",
                        );
                        HirType::Unknown
                    }
                };
                Typed {
                    ty,
                    effects: typed.effects,
                }
            }
            Expr::Lambda { params, body } => {
                let mut local = scope.clone();
                let mut param_types = Vec::new();
                for param in params {
                    let ty = self.lower_type(&param.ty, scope);
                    for name in &param.names {
                        local.values.insert(name.0.clone(), ty.clone());
                        param_types.push(ty.clone());
                    }
                }
                let body_t = self.check_expr(body, &local, None);
                Typed {
                    ty: HirType::Fn {
                        params: param_types,
                        ret: Box::new(body_t.ty),
                    },
                    effects: body_t.effects,
                }
            }
        }
    }

    fn check_object(&mut self, path: &Path, fields: &[(Ident, Expr)], scope: &Scope) -> Typed {
        let name = path_name(path);
        let TypeDef::Struct(shape) = self
            .types
            .get(&name)
            .cloned()
            .unwrap_or(TypeDef::Alias(HirType::Unknown))
        else {
            self.error(
                "E0202",
                format!("unknown struct `{name}`"),
                &name,
                "object type",
            );
            return Typed::pure(HirType::Unknown);
        };

        let mut seen = HashSet::new();
        let mut effects = HashSet::new();
        for (field, value) in fields {
            let Some(expected) = shape.get(&field.0) else {
                self.error(
                    "E0302",
                    format!("unknown field `{}` on `{name}`", field.0),
                    &field.0,
                    "field is not declared on this struct",
                );
                continue;
            };
            seen.insert(field.0.clone());
            let value_t = self.check_expr(value, scope, Some(expected));
            if !self.is_assignable(expected, &value_t.ty) {
                self.error(
                    "E0401",
                    format!(
                        "field `{}` expected `{}` but got `{}`",
                        field.0,
                        display_type(expected),
                        display_type(&value_t.ty)
                    ),
                    &field.0,
                    "field value has the wrong type",
                );
            }
            effects.extend(value_t.effects);
        }
        for field in shape.keys() {
            if !seen.contains(field) {
                self.error(
                    "E0303",
                    format!("missing field `{field}` on `{name}`"),
                    field,
                    "object literal is incomplete",
                );
            }
        }
        Typed {
            ty: HirType::Struct(name),
            effects,
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr], scope: &Scope) -> Typed {
        let Some(name) = callee_name(callee) else {
            self.error(
                "E0201",
                "unsupported callee",
                "call",
                "callee cannot be resolved",
            );
            return Typed::pure(HirType::Unknown);
        };

        let Some(info) = self.fns.get(&name).cloned() else {
            self.error(
                "E0201",
                format!("unknown name `{name}`"),
                &name,
                "function not found",
            );
            return Typed::pure(HirType::Unknown);
        };

        let mut effects = info.effects.clone();
        let arg_types = args
            .iter()
            .enumerate()
            .map(|(idx, arg)| {
                let expected = if name == "sum" {
                    None
                } else {
                    info.params
                        .get(idx)
                        .filter(|ty| !contains_generic(ty))
                        .map(|ty| ty as &HirType)
                };
                let typed = self.check_expr(arg, scope, expected);
                effects.extend(typed.effects);
                typed.ty
            })
            .collect::<Vec<_>>();

        if name != "sum" && info.params.len() != arg_types.len() {
            self.error(
                "E0407",
                format!(
                    "function `{name}` expects {} args but got {}",
                    info.params.len(),
                    arg_types.len()
                ),
                &name,
                "wrong number of arguments",
            );
        }

        let mut substitutions = HashMap::new();
        for (idx, (expected, actual)) in info.params.iter().zip(arg_types.iter()).enumerate() {
            if name == "sum" {
                continue;
            }
            if !self.unify_arg(expected, actual, &mut substitutions) {
                let code = if !info.generics.is_empty() && contains_generic(expected) {
                    "E0410"
                } else {
                    "E0401"
                };
                let expected = substitute_type(expected, &substitutions);
                self.error(
                    code,
                    format!(
                        "argument {} of `{name}` expected `{}` but got `{}`",
                        idx + 1,
                        display_type(&expected),
                        display_type(actual)
                    ),
                    &name,
                    "argument type mismatch",
                );
            }
        }

        let ret = if name == "sum" {
            match arg_types.first() {
                Some(HirType::Slice(inner)) => *inner.clone(),
                Some(HirType::Ref { inner, .. }) => match inner.as_ref() {
                    HirType::Slice(inner) => *inner.clone(),
                    _ => HirType::Unknown,
                },
                Some(HirType::Prim(_)) => arg_types[0].clone(),
                _ => HirType::Prim("f4".to_string()),
            }
        } else {
            substitute_type(&info.ret, &substitutions)
        };

        Typed { ty: ret, effects }
    }

    fn check_binary(&mut self, op: &BinaryOp, lhs: &Expr, rhs: &Expr, scope: &Scope) -> Typed {
        let left = self.check_expr(lhs, scope, None);
        let right = self.check_expr(rhs, scope, None);
        let mut effects = left.effects;
        effects.extend(right.effects);

        let ty = match op {
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge => HirType::b(),
            BinaryOp::And | BinaryOp::Or => HirType::b(),
            _ if self.is_numeric_pair(&left.ty, &right.ty) => common_numeric(&left.ty, &right.ty),
            _ if is_ref_slice_numeric_pair(&left.ty, &right.ty) => {
                HirType::Slice(Box::new(HirType::Prim("f4".to_string())))
            }
            _ => {
                self.error(
                    "E0408",
                    format!(
                        "operator `{:?}` cannot combine `{}` and `{}`",
                        op,
                        display_type(&left.ty),
                        display_type(&right.ty)
                    ),
                    expr_name(lhs).as_deref().unwrap_or("?"),
                    "binary operand mismatch",
                );
                HirType::Unknown
            }
        };
        Typed { ty, effects }
    }

    fn field_type(&mut self, base: &HirType, field: &str) -> HirType {
        match deref_ref(base) {
            HirType::Struct(name) => match self.types.get(&name) {
                Some(TypeDef::Struct(fields)) => fields.get(field).cloned().unwrap_or_else(|| {
                    self.error(
                        "E0302",
                        format!("unknown field `{field}` on `{name}`"),
                        field,
                        "field is not declared on this struct",
                    );
                    HirType::Unknown
                }),
                _ => HirType::Unknown,
            },
            HirType::Slice(_) | HirType::Array { .. } if field == "len" => {
                HirType::Prim("uz".to_string())
            }
            HirType::Any | HirType::Unknown => HirType::Unknown,
            other => {
                self.error(
                    "E0301",
                    format!("type `{}` has no fields", display_type(&other)),
                    field,
                    "field access",
                );
                HirType::Unknown
            }
        }
    }

    fn index_type(&mut self, base: &HirType) -> HirType {
        match deref_ref(base) {
            HirType::Slice(inner) => *inner,
            HirType::Array { inner } => *inner,
            HirType::Any | HirType::Unknown => HirType::Unknown,
            other => {
                self.error(
                    "E0304",
                    format!("type `{}` cannot be indexed", display_type(&other)),
                    "[]",
                    "index base",
                );
                HirType::Unknown
            }
        }
    }

    fn merge_branch_types(
        &mut self,
        then_ty: &HirType,
        else_ty: &HirType,
        expected: Option<&HirType>,
    ) -> HirType {
        if let Some(expected) = expected {
            if self.is_assignable(expected, then_ty) && self.is_assignable(expected, else_ty) {
                return expected.clone();
            }
            if let HirType::Result { ok, err } = expected {
                let then_ok = self.is_assignable(ok, then_ty);
                let then_err = self.is_assignable(err, then_ty);
                let else_ok = self.is_assignable(ok, else_ty);
                let else_err = self.is_assignable(err, else_ty);
                if (then_ok && else_err) || (then_err && else_ok) || (then_ok && else_ok) {
                    return expected.clone();
                }
            }
        }
        if self.is_assignable(then_ty, else_ty) {
            then_ty.clone()
        } else if self.is_assignable(else_ty, then_ty) {
            else_ty.clone()
        } else {
            self.error(
                "E0402",
                format!(
                    "ternary branches have incompatible types `{}` and `{}`",
                    display_type(then_ty),
                    display_type(else_ty)
                ),
                "?",
                "branch type mismatch",
            );
            HirType::Unknown
        }
    }

    fn lower_fn_sig(&mut self, sig: &FnSig) -> HirFnSig {
        let scope = scope_for_generics(&sig.generics);
        let mut params = Vec::new();
        for param in &sig.params {
            let ty = self.lower_type(&param.ty, &scope);
            for name in &param.names {
                params.push(HirParam {
                    name: name.0.clone(),
                    ty: ty.clone(),
                });
            }
        }
        HirFnSig {
            name: sig.name.0.iter().map(id).collect(),
            params,
            ret: sig
                .ret
                .as_ref()
                .map(|ty| self.lower_type(ty, &scope))
                .unwrap_or_else(HirType::v),
            effects: sig.effects.iter().map(|effect| effect.0.clone()).collect(),
        }
    }

    fn fn_info(&mut self, sig: &FnSig) -> FnInfo {
        let scope = scope_for_generics(&sig.generics);
        self.validate_generic_bounds(&sig.generics, &scope);
        let hir = self.lower_fn_sig(sig);
        let known_effects = known_effects();
        for effect in &hir.effects {
            if !known_effects.contains(effect.as_str()) {
                self.error(
                    "E0601",
                    format!("unknown effect `{effect}`"),
                    effect,
                    "effect is not in the v1-alpha set",
                );
            }
        }
        FnInfo {
            params: hir.params.into_iter().map(|param| param.ty).collect(),
            ret: hir.ret,
            effects: hir.effects.into_iter().collect(),
            generics: sig
                .generics
                .iter()
                .map(|generic| generic.name.0.clone())
                .collect(),
        }
    }

    fn validate_generic_bounds(&mut self, generics: &[sing_ast::Generic], scope: &Scope) {
        for generic in generics {
            if let Some(bound) = &generic.bound {
                self.lower_type(bound, scope);
            }
        }
    }

    fn lower_type(&mut self, ty: &AstType, scope: &Scope) -> HirType {
        match ty {
            AstType::Prim(Prim::Any) => HirType::Any,
            AstType::Prim(prim) => HirType::Prim(prim_name(prim).to_string()),
            AstType::Path(path) => {
                let name = path_name(path);
                if scope.generics.contains(&name) {
                    HirType::Generic(name)
                } else {
                    match self.types.get(&name) {
                        Some(TypeDef::Alias(ty)) => ty.clone(),
                        Some(TypeDef::Struct(_)) => HirType::Struct(name),
                        Some(TypeDef::Enum) => HirType::Enum(name),
                        Some(TypeDef::Trait) => HirType::Trait(name),
                        None => {
                            self.error(
                                "E0202",
                                format!("unknown type `{name}`"),
                                &name,
                                "type is not declared",
                            );
                            HirType::Unknown
                        }
                    }
                }
            }
            AstType::Ref { mutable, inner } => HirType::Ref {
                mutable: *mutable,
                inner: Box::new(self.lower_type(inner, scope)),
            },
            AstType::Raw(inner) => HirType::Raw(Box::new(self.lower_type(inner, scope))),
            AstType::Slice(inner) => HirType::Slice(Box::new(self.lower_type(inner, scope))),
            AstType::Array { inner, len } => {
                let len_t = self.check_expr(len, scope, None);
                if !is_integer(&len_t.ty) {
                    self.error(
                        "E0404",
                        "array length must be an integer",
                        "[N]",
                        "array length",
                    );
                }
                HirType::Array {
                    inner: Box::new(self.lower_type(inner, scope)),
                }
            }
            AstType::Option(inner) => HirType::Option(Box::new(self.lower_type(inner, scope))),
            AstType::Result { ok, err } => HirType::Result {
                ok: Box::new(self.lower_type(ok, scope)),
                err: Box::new(self.lower_type(err, scope)),
            },
            AstType::Tuple(types) => {
                HirType::Tuple(types.iter().map(|ty| self.lower_type(ty, scope)).collect())
            }
            AstType::Fn { params, ret } => HirType::Fn {
                params: params.iter().map(|ty| self.lower_type(ty, scope)).collect(),
                ret: Box::new(self.lower_type(ret, scope)),
            },
            AstType::Dyn(path) => {
                let trait_name = path.as_ref().map(path_name);
                if let Some(name) = &trait_name {
                    if !matches!(self.types.get(name), Some(TypeDef::Trait)) {
                        self.error(
                            "E0202",
                            format!("unknown trait `{name}`"),
                            name,
                            "dynamic trait object",
                        );
                    }
                }
                HirType::Dyn(trait_name)
            }
        }
    }

    fn struct_fields(&self, name: &str) -> Vec<HirField> {
        match self.types.get(name) {
            Some(TypeDef::Struct(fields)) => fields
                .iter()
                .map(|(name, ty)| HirField {
                    name: name.clone(),
                    ty: ty.clone(),
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn insert_type(&mut self, name: String, def: TypeDef) {
        if self.types.insert(name.clone(), def).is_some() {
            self.error(
                "E0101",
                format!("duplicate type `{name}`"),
                &name,
                "type already exists",
            );
        }
    }

    fn insert_fn(&mut self, name: String, info: FnInfo) {
        if self.fns.insert(name.clone(), info).is_some() {
            self.error(
                "E0101",
                format!("duplicate function `{name}`"),
                &name,
                "function already exists",
            );
        }
    }

    fn expect_assignable(&mut self, expected: &HirType, actual: &HirType, name: &str, code: &str) {
        if !self.is_assignable(expected, actual) {
            self.error(
                code,
                format!(
                    "return type expected `{}` but got `{}`",
                    display_type(expected),
                    display_type(actual)
                ),
                name,
                "type mismatch",
            );
        }
    }

    fn is_assignable(&self, expected: &HirType, actual: &HirType) -> bool {
        if expected == actual
            || expected.is_unknown_like()
            || actual.is_unknown_like()
            || matches!(actual, HirType::Never)
            || (is_numeric(expected) && is_numeric(actual))
        {
            return true;
        }

        match (expected, actual) {
            (
                HirType::Ref { mutable, inner },
                HirType::Ref {
                    mutable: actual_mut,
                    inner: actual_inner,
                },
            ) if mutable == actual_mut => self.is_assignable(inner, actual_inner),
            (HirType::Raw(inner), HirType::Raw(actual_inner))
            | (HirType::Slice(inner), HirType::Slice(actual_inner))
            | (HirType::Option(inner), HirType::Option(actual_inner)) => {
                self.is_assignable(inner, actual_inner)
            }
            (
                HirType::Array { inner },
                HirType::Array {
                    inner: actual_inner,
                },
            ) => self.is_assignable(inner, actual_inner),
            (
                HirType::Result { ok, err },
                HirType::Result {
                    ok: actual_ok,
                    err: actual_err,
                },
            ) => self.is_assignable(ok, actual_ok) && self.is_assignable(err, actual_err),
            (HirType::Result { ok, err }, ty) => {
                self.is_assignable(ok, ty) || self.is_assignable(err, ty)
            }
            (HirType::Tuple(types), HirType::Tuple(actual_types))
                if types.len() == actual_types.len() =>
            {
                types
                    .iter()
                    .zip(actual_types)
                    .all(|(expected, actual)| self.is_assignable(expected, actual))
            }
            (
                HirType::Fn { params, ret },
                HirType::Fn {
                    params: actual_params,
                    ret: actual_ret,
                },
            ) if params.len() == actual_params.len() => {
                params
                    .iter()
                    .zip(actual_params)
                    .all(|(expected, actual)| self.is_assignable(expected, actual))
                    && self.is_assignable(ret, actual_ret)
            }
            _ => false,
        }
    }

    fn unify_arg(
        &self,
        expected: &HirType,
        actual: &HirType,
        substitutions: &mut HashMap<String, HirType>,
    ) -> bool {
        match expected {
            HirType::Generic(name) => match substitutions.get(name) {
                Some(existing) => existing == actual || self.is_assignable(existing, actual),
                None => {
                    substitutions.insert(name.clone(), actual.clone());
                    true
                }
            },
            HirType::Ref { mutable, inner } => match actual {
                HirType::Ref {
                    mutable: actual_mut,
                    inner: actual_inner,
                } if mutable == actual_mut => self.unify_arg(inner, actual_inner, substitutions),
                _ => self.is_assignable(expected, actual),
            },
            HirType::Raw(inner) => match actual {
                HirType::Raw(actual_inner) => self.unify_arg(inner, actual_inner, substitutions),
                _ => self.is_assignable(expected, actual),
            },
            HirType::Slice(inner) => match actual {
                HirType::Slice(actual_inner) => self.unify_arg(inner, actual_inner, substitutions),
                _ => self.is_assignable(expected, actual),
            },
            HirType::Array { inner } => match actual {
                HirType::Array {
                    inner: actual_inner,
                } => self.unify_arg(inner, actual_inner, substitutions),
                _ => self.is_assignable(expected, actual),
            },
            HirType::Option(inner) => match actual {
                HirType::Option(actual_inner) => self.unify_arg(inner, actual_inner, substitutions),
                _ => self.is_assignable(expected, actual),
            },
            HirType::Result { ok, err } => match actual {
                HirType::Result {
                    ok: actual_ok,
                    err: actual_err,
                } => {
                    self.unify_arg(ok, actual_ok, substitutions)
                        && self.unify_arg(err, actual_err, substitutions)
                }
                _ => self.is_assignable(expected, actual),
            },
            HirType::Tuple(types) => match actual {
                HirType::Tuple(actual_types) if types.len() == actual_types.len() => types
                    .iter()
                    .zip(actual_types)
                    .all(|(expected, actual)| self.unify_arg(expected, actual, substitutions)),
                _ => self.is_assignable(expected, actual),
            },
            HirType::Fn { params, ret } => {
                match actual {
                    HirType::Fn {
                        params: actual_params,
                        ret: actual_ret,
                    } if params.len() == actual_params.len() => {
                        params.iter().zip(actual_params).all(|(expected, actual)| {
                            self.unify_arg(expected, actual, substitutions)
                        }) && self.unify_arg(ret, actual_ret, substitutions)
                    }
                    _ => self.is_assignable(expected, actual),
                }
            }
            _ => self.is_assignable(expected, actual),
        }
    }

    fn is_numeric_pair(&self, left: &HirType, right: &HirType) -> bool {
        is_numeric(left) && is_numeric(right)
    }

    fn error(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        name: &str,
        label: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic::error(
            code,
            message,
            self.spans.name(name),
            label,
        ));
    }
}

fn scope_for_generics(generics: &[sing_ast::Generic]) -> Scope {
    Scope {
        values: HashMap::new(),
        generics: generics
            .iter()
            .map(|generic| generic.name.0.clone())
            .collect(),
        unsafe_allowed: false,
    }
}

fn has_attr(attrs: &[Attr], target: &str) -> bool {
    attrs.iter().any(|attr| attr.0 == target)
}

fn id(ident: &Ident) -> String {
    ident.0.clone()
}

fn path_name(path: &Path) -> String {
    path.0.iter().map(id).collect::<Vec<_>>().join(".")
}

fn callee_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.0.clone()),
        Expr::Path(path) => Some(path_name(path)),
        Expr::Field { base, name } => {
            let base = callee_name(base)?;
            Some(format!("{base}.{}", name.0))
        }
        _ => None,
    }
}

fn expr_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name) => Some(name.0.clone()),
        Expr::Path(path) => Some(path_name(path)),
        Expr::Field { name, .. } => Some(name.0.clone()),
        Expr::Call { callee, .. } => callee_name(callee),
        _ => None,
    }
}

fn stmt_name(stmt: &Stmt) -> &str {
    match stmt {
        Stmt::Require(_) => "?",
        Stmt::Ensure(_) => "~",
        Stmt::Return(_) => ">",
        Stmt::LoopRange { var, .. } => &var.0,
        Stmt::LoopWhile { .. } => "l",
        Stmt::Break => "b",
        Stmt::Continue => "c",
        Stmt::Bind { name, .. } => &name.0,
        Stmt::Mut { .. } => ":=",
        Stmt::Expr(_) => "expr",
    }
}

fn prim_name(prim: &Prim) -> &'static str {
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

fn known_effects() -> HashSet<&'static str> {
    [
        "h", "d", "g", "z", "fr", "fw", "ir", "iw", "nr", "nw", "dr", "dw", "tm", "rn", "th", "at",
        "ff", "bk", "sy",
    ]
    .into_iter()
    .collect()
}

fn display_type(ty: &HirType) -> String {
    match ty {
        HirType::Prim(name)
        | HirType::Struct(name)
        | HirType::Enum(name)
        | HirType::Trait(name)
        | HirType::Generic(name) => name.clone(),
        HirType::Ref { mutable, inner } => {
            let bang = if *mutable { "!" } else { "" };
            format!("&{bang}{}", display_type(inner))
        }
        HirType::Raw(inner) => format!("^{}", display_type(inner)),
        HirType::Slice(inner) => format!("{}[]", display_type(inner)),
        HirType::Array { inner } => format!("{}[N]", display_type(inner)),
        HirType::Option(inner) => format!("{}?", display_type(inner)),
        HirType::Result { ok, err } => format!("{}/{}", display_type(ok), display_type(err)),
        HirType::Tuple(types) => format!(
            "({})",
            types.iter().map(display_type).collect::<Vec<_>>().join(",")
        ),
        HirType::Fn { params, ret } => format!(
            "({})>{}",
            params
                .iter()
                .map(display_type)
                .collect::<Vec<_>>()
                .join(","),
            display_type(ret)
        ),
        HirType::Dyn(Some(name)) => format!("*{name}"),
        HirType::Dyn(None) => "*".to_string(),
        HirType::Any => "*".to_string(),
        HirType::Unknown => "?".to_string(),
        HirType::Never => "!".to_string(),
    }
}

fn deref_ref(ty: &HirType) -> HirType {
    match ty {
        HirType::Ref { inner, .. } => *inner.clone(),
        other => other.clone(),
    }
}

fn contextual_numeric_type(expected: &HirType) -> Option<&HirType> {
    match expected {
        ty if is_numeric(ty) => Some(ty),
        HirType::Option(inner) => contextual_numeric_type(inner),
        HirType::Result { ok, .. } => contextual_numeric_type(ok),
        _ => None,
    }
}

fn int_literal_type(lit: &str, expected: Option<&HirType>) -> HirType {
    for suffix in ["i1", "i2", "i4", "i8", "iz", "u1", "u2", "u4", "u8", "uz"] {
        if lit.ends_with(suffix) {
            return HirType::Prim(suffix.to_string());
        }
    }
    expected
        .and_then(contextual_numeric_type)
        .filter(|ty| is_integer(ty))
        .cloned()
        .unwrap_or_else(|| HirType::Prim("i4".to_string()))
}

fn float_literal_type(lit: &str, expected: Option<&HirType>) -> HirType {
    for suffix in ["f4", "f8"] {
        if lit.ends_with(suffix) {
            return HirType::Prim(suffix.to_string());
        }
    }
    expected
        .and_then(contextual_numeric_type)
        .filter(|ty| is_float(ty))
        .cloned()
        .unwrap_or_else(|| HirType::Prim("f4".to_string()))
}

fn is_numeric(ty: &HirType) -> bool {
    matches!(ty, HirType::Prim(name) if matches!(
        name.as_str(),
        "i1" | "i2" | "i4" | "i8" | "iz" | "u1" | "u2" | "u4" | "u8" | "uz" | "f4" | "f8"
    ))
}

fn is_float(ty: &HirType) -> bool {
    matches!(ty, HirType::Prim(name) if matches!(name.as_str(), "f4" | "f8"))
}

fn is_integer(ty: &HirType) -> bool {
    matches!(ty, HirType::Prim(name) if matches!(
        name.as_str(),
        "i1" | "i2" | "i4" | "i8" | "iz" | "u1" | "u2" | "u4" | "u8" | "uz"
    ))
}

fn is_condition(ty: &HirType) -> bool {
    *ty == HirType::b() || is_numeric(ty) || ty.is_unknown_like()
}

fn common_numeric(left: &HirType, right: &HirType) -> HirType {
    if matches!(left, HirType::Prim(name) if name.starts_with('f')) {
        left.clone()
    } else if matches!(right, HirType::Prim(name) if name.starts_with('f')) {
        right.clone()
    } else {
        left.clone()
    }
}

fn contains_generic(ty: &HirType) -> bool {
    match ty {
        HirType::Generic(_) => true,
        HirType::Ref { inner, .. }
        | HirType::Raw(inner)
        | HirType::Slice(inner)
        | HirType::Array { inner }
        | HirType::Option(inner) => contains_generic(inner),
        HirType::Result { ok, err } => contains_generic(ok) || contains_generic(err),
        HirType::Tuple(types) => types.iter().any(contains_generic),
        HirType::Fn { params, ret } => params.iter().any(contains_generic) || contains_generic(ret),
        HirType::Prim(_)
        | HirType::Struct(_)
        | HirType::Enum(_)
        | HirType::Trait(_)
        | HirType::Dyn(_)
        | HirType::Any
        | HirType::Unknown
        | HirType::Never => false,
    }
}

fn substitute_type(ty: &HirType, substitutions: &HashMap<String, HirType>) -> HirType {
    match ty {
        HirType::Generic(name) => substitutions
            .get(name)
            .cloned()
            .unwrap_or_else(|| HirType::Generic(name.clone())),
        HirType::Ref { mutable, inner } => HirType::Ref {
            mutable: *mutable,
            inner: Box::new(substitute_type(inner, substitutions)),
        },
        HirType::Raw(inner) => HirType::Raw(Box::new(substitute_type(inner, substitutions))),
        HirType::Slice(inner) => HirType::Slice(Box::new(substitute_type(inner, substitutions))),
        HirType::Array { inner } => HirType::Array {
            inner: Box::new(substitute_type(inner, substitutions)),
        },
        HirType::Option(inner) => HirType::Option(Box::new(substitute_type(inner, substitutions))),
        HirType::Result { ok, err } => HirType::Result {
            ok: Box::new(substitute_type(ok, substitutions)),
            err: Box::new(substitute_type(err, substitutions)),
        },
        HirType::Tuple(types) => HirType::Tuple(
            types
                .iter()
                .map(|ty| substitute_type(ty, substitutions))
                .collect(),
        ),
        HirType::Fn { params, ret } => HirType::Fn {
            params: params
                .iter()
                .map(|ty| substitute_type(ty, substitutions))
                .collect(),
            ret: Box::new(substitute_type(ret, substitutions)),
        },
        other => other.clone(),
    }
}

fn is_ref_slice_numeric_pair(left: &HirType, right: &HirType) -> bool {
    slice_inner(left).is_some_and(is_numeric) && slice_inner(right).is_some_and(is_numeric)
}

fn slice_inner(ty: &HirType) -> Option<&HirType> {
    match ty {
        HirType::Slice(inner) => Some(inner),
        HirType::Ref { inner, .. } => match inner.as_ref() {
            HirType::Slice(inner) => Some(inner),
            _ => None,
        },
        _ => None,
    }
}

fn block_contains_string(block: &Block) -> bool {
    block.stmts.iter().any(stmt_contains_string)
}

fn module_name(file: &File) -> Option<String> {
    file.items.iter().find_map(|item| match &item.kind {
        ItemKind::Module(path) => Some(path_name(path)),
        _ => None,
    })
}

fn raw_symbol(module: &str, namespace: Namespace, name: &str) -> Symbol {
    let qualified = if namespace == Namespace::Module {
        module.to_string()
    } else {
        format!("{module}.{name}")
    };
    Symbol {
        id: SymbolId(0),
        namespace,
        module: module.to_string(),
        name: name.to_string(),
        qualified,
    }
}

fn assign_symbol_ids(mut symbols: Vec<Symbol>) -> Vec<Symbol> {
    symbols.sort_by(|a, b| {
        a.qualified
            .cmp(&b.qualified)
            .then_with(|| format!("{:?}", a.namespace).cmp(&format!("{:?}", b.namespace)))
    });
    symbols.dedup_by(|a, b| a.qualified == b.qualified && a.namespace == b.namespace);
    for (idx, symbol) in symbols.iter_mut().enumerate() {
        symbol.id = SymbolId(idx as u64 + 1);
    }
    symbols
}

fn import_cycles(imports: &HashMap<String, Vec<String>>) -> Vec<String> {
    let mut cycles = Vec::new();
    for module in imports.keys() {
        let mut visiting = HashSet::new();
        if reaches_module(module, module, imports, &mut visiting) {
            cycles.push(module.clone());
        }
    }
    cycles.sort();
    cycles.dedup();
    cycles
}

fn reaches_module(
    start: &str,
    current: &str,
    imports: &HashMap<String, Vec<String>>,
    visiting: &mut HashSet<String>,
) -> bool {
    let Some(next) = imports.get(current) else {
        return false;
    };
    for import in next {
        if import == start {
            return true;
        }
        if visiting.insert(import.clone()) && reaches_module(start, import, imports, visiting) {
            return true;
        }
    }
    false
}

fn fn_info_from_sig(sig: &FnSig) -> FnInfo {
    let generics = sig
        .generics
        .iter()
        .map(|generic| generic.name.0.clone())
        .collect::<HashSet<_>>();
    FnInfo {
        params: sig
            .params
            .iter()
            .flat_map(|param| param_types_from_ast(param, &generics))
            .collect(),
        ret: sig
            .ret
            .as_ref()
            .map(|ty| hir_type_from_ast_shallow(ty, &generics))
            .unwrap_or_else(HirType::v),
        effects: sig.effects.iter().map(|effect| effect.0.clone()).collect(),
        generics: sig
            .generics
            .iter()
            .map(|generic| generic.name.0.clone())
            .collect(),
    }
}

fn param_types_from_ast(param: &sing_ast::Param, generics: &HashSet<String>) -> Vec<HirType> {
    let ty = hir_type_from_ast_shallow(&param.ty, generics);
    param.names.iter().map(|_| ty.clone()).collect()
}

fn hir_type_from_ast_shallow(ty: &AstType, generics: &HashSet<String>) -> HirType {
    match ty {
        AstType::Prim(Prim::Any) => HirType::Any,
        AstType::Prim(prim) => HirType::Prim(prim_name(prim).to_string()),
        AstType::Path(path) => {
            let name = path_name(path);
            if generics.contains(&name) {
                HirType::Generic(name)
            } else {
                HirType::Struct(name)
            }
        }
        AstType::Ref { mutable, inner } => HirType::Ref {
            mutable: *mutable,
            inner: Box::new(hir_type_from_ast_shallow(inner, generics)),
        },
        AstType::Raw(inner) => HirType::Raw(Box::new(hir_type_from_ast_shallow(inner, generics))),
        AstType::Slice(inner) => {
            HirType::Slice(Box::new(hir_type_from_ast_shallow(inner, generics)))
        }
        AstType::Array { inner, .. } => HirType::Array {
            inner: Box::new(hir_type_from_ast_shallow(inner, generics)),
        },
        AstType::Option(inner) => {
            HirType::Option(Box::new(hir_type_from_ast_shallow(inner, generics)))
        }
        AstType::Result { ok, err } => HirType::Result {
            ok: Box::new(hir_type_from_ast_shallow(ok, generics)),
            err: Box::new(hir_type_from_ast_shallow(err, generics)),
        },
        AstType::Tuple(types) => HirType::Tuple(
            types
                .iter()
                .map(|ty| hir_type_from_ast_shallow(ty, generics))
                .collect(),
        ),
        AstType::Fn { params, ret } => HirType::Fn {
            params: params
                .iter()
                .map(|ty| hir_type_from_ast_shallow(ty, generics))
                .collect(),
            ret: Box::new(hir_type_from_ast_shallow(ret, generics)),
        },
        AstType::Dyn(path) => HirType::Dyn(path.as_ref().map(path_name)),
    }
}

fn stmt_contains_string(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Require(expr)
        | Stmt::Ensure(expr)
        | Stmt::Return(expr)
        | Stmt::Bind { expr, .. }
        | Stmt::Mut { expr, .. }
        | Stmt::Expr(expr) => expr_contains_string(expr),
        Stmt::LoopRange {
            start, end, body, ..
        } => expr_contains_string(start) || expr_contains_string(end) || stmt_contains_string(body),
        Stmt::LoopWhile { cond, body } => expr_contains_string(cond) || block_contains_string(body),
        Stmt::Break | Stmt::Continue => false,
    }
}

fn expr_contains_string(expr: &Expr) -> bool {
    match expr {
        Expr::String(_) => true,
        Expr::Tuple(values) => values.iter().any(expr_contains_string),
        Expr::Object { fields, .. } => fields.iter().any(|(_, expr)| expr_contains_string(expr)),
        Expr::Field { base, .. } => expr_contains_string(base),
        Expr::Index { base, index } => expr_contains_string(base) || expr_contains_string(index),
        Expr::Call { callee, args } => {
            expr_contains_string(callee) || args.iter().any(expr_contains_string)
        }
        Expr::Unary { expr, .. } | Expr::Propagate(expr) => expr_contains_string(expr),
        Expr::Binary { lhs, rhs, .. } => expr_contains_string(lhs) || expr_contains_string(rhs),
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_contains_string(cond)
                || expr_contains_string(then_expr)
                || expr_contains_string(else_expr)
        }
        Expr::Lambda { body, .. } => expr_contains_string(body),
        Expr::Missing
        | Expr::Ident(_)
        | Expr::Path(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::Char(_)
        | Expr::Bool(_)
        | Expr::None => false,
    }
}
