use std::collections::HashMap;

use sing_ast::{Block, Expr, Stmt, UnaryOp};
use sing_hir::Type as HirType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MemoryError {
    UseAfterMove,
    BorrowConflict,
    MutateBorrowed,
}

impl MemoryError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::UseAfterMove => "E0503",
            Self::BorrowConflict => "E0504",
            Self::MutateBorrowed => "E0505",
        }
    }

    pub(crate) fn message(&self, name: &str) -> String {
        match self {
            Self::UseAfterMove => format!("use of moved value `{name}`"),
            Self::BorrowConflict => format!("borrow conflict on `{name}`"),
            Self::MutateBorrowed => format!("cannot mutate `{name}` while it is borrowed"),
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::UseAfterMove => "value was already moved",
            Self::BorrowConflict => "active borrow is incompatible",
            Self::MutateBorrowed => "active borrow prevents mutation",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MemoryState {
    locals: HashMap<String, LocalState>,
}

#[derive(Debug, Clone, Default)]
struct LocalState {
    moved: bool,
    borrow: BorrowState,
}

#[derive(Debug, Clone, Default)]
struct BorrowState {
    shared: usize,
    mutable: bool,
}

impl MemoryState {
    pub(crate) fn define(&mut self, name: impl Into<String>) {
        self.locals.insert(name.into(), LocalState::default());
    }

    pub(crate) fn assign(&mut self, name: &str) {
        self.locals.insert(name.to_string(), LocalState::default());
    }

    pub(crate) fn use_value(&mut self, name: &str, ty: &HirType) -> Option<MemoryError> {
        let state = self.locals.entry(name.to_string()).or_default();
        if state.moved {
            return Some(MemoryError::UseAfterMove);
        }
        if !is_copy_type(ty) {
            if state.borrow.shared > 0 || state.borrow.mutable {
                return Some(MemoryError::BorrowConflict);
            }
            state.moved = true;
        }
        None
    }

    pub(crate) fn borrow(&mut self, name: &str, mutable: bool) -> Option<MemoryError> {
        let state = self.locals.entry(name.to_string()).or_default();
        if state.moved {
            return Some(MemoryError::UseAfterMove);
        }
        if mutable {
            if state.borrow.shared > 0 || state.borrow.mutable {
                return Some(MemoryError::BorrowConflict);
            }
            state.borrow.mutable = true;
        } else if state.borrow.mutable {
            return Some(MemoryError::BorrowConflict);
        } else {
            state.borrow.shared += 1;
        }
        None
    }

    pub(crate) fn mutate(&mut self, name: &str) -> Option<MemoryError> {
        let state = self.locals.entry(name.to_string()).or_default();
        if state.borrow.shared > 0 || state.borrow.mutable {
            return Some(MemoryError::MutateBorrowed);
        }
        state.moved = false;
        None
    }
}

pub(crate) fn is_copy_type(ty: &HirType) -> bool {
    match ty {
        HirType::Prim(_) | HirType::Ref { .. } | HirType::Raw(_) | HirType::Slice(_) => true,
        HirType::Array { inner } | HirType::Option(inner) => is_copy_type(inner),
        HirType::Result { ok, err } => is_copy_type(ok) && is_copy_type(err),
        HirType::Tuple(types) => types.iter().all(is_copy_type),
        HirType::Fn { .. } => true,
        HirType::Struct(_)
        | HirType::Enum(_)
        | HirType::Trait(_)
        | HirType::Generic(_)
        | HirType::Dyn(_)
        | HirType::Any
        | HirType::Unknown
        | HirType::Never => false,
    }
}

pub(crate) fn contains_dyn(ty: &HirType) -> bool {
    match ty {
        HirType::Dyn(_) => true,
        HirType::Ref { inner, .. }
        | HirType::Raw(inner)
        | HirType::Slice(inner)
        | HirType::Array { inner }
        | HirType::Option(inner) => contains_dyn(inner),
        HirType::Result { ok, err } => contains_dyn(ok) || contains_dyn(err),
        HirType::Tuple(types) => types.iter().any(contains_dyn),
        HirType::Fn { params, ret } => params.iter().any(contains_dyn) || contains_dyn(ret),
        _ => false,
    }
}

pub(crate) fn is_ffi_safe_type(ty: &HirType) -> bool {
    match ty {
        HirType::Prim(name) => matches!(
            name.as_str(),
            "b" | "i1"
                | "i2"
                | "i4"
                | "i8"
                | "iz"
                | "u1"
                | "u2"
                | "u4"
                | "u8"
                | "uz"
                | "f4"
                | "f8"
                | "c"
                | "y"
                | "v"
        ),
        HirType::Raw(inner) => is_ffi_safe_type(inner),
        HirType::Array { inner } => is_ffi_safe_type(inner),
        HirType::Struct(_) | HirType::Enum(_) => true,
        _ => false,
    }
}

pub(crate) fn borrowed_local(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Unary {
            op: UnaryOp::Ref | UnaryOp::RefMut,
            expr,
        } => match expr.as_ref() {
            Expr::Ident(name) => Some(name.0.as_str()),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn body_has_contract(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_contract)
}

pub(crate) fn body_has_mutation_or_mut_borrow(block: &Block) -> bool {
    block.stmts.iter().any(stmt_has_mutation_or_mut_borrow)
}

fn stmt_has_contract(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Require(_) | Stmt::Ensure(_) => true,
        Stmt::LoopRange { body, .. } => stmt_has_contract(body),
        Stmt::LoopWhile { body, .. } => body_has_contract(body),
        _ => false,
    }
}

fn stmt_has_mutation_or_mut_borrow(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Mut { .. } => true,
        Stmt::Require(expr)
        | Stmt::Ensure(expr)
        | Stmt::Return(expr)
        | Stmt::Bind { expr, .. }
        | Stmt::Expr(expr) => expr_has_mut_borrow(expr),
        Stmt::LoopRange {
            start, end, body, ..
        } => {
            expr_has_mut_borrow(start)
                || expr_has_mut_borrow(end)
                || stmt_has_mutation_or_mut_borrow(body)
        }
        Stmt::LoopWhile { cond, body } => {
            expr_has_mut_borrow(cond) || body_has_mutation_or_mut_borrow(body)
        }
        Stmt::Break | Stmt::Continue => false,
    }
}

fn expr_has_mut_borrow(expr: &Expr) -> bool {
    match expr {
        Expr::Unary {
            op: UnaryOp::RefMut,
            ..
        } => true,
        Expr::Tuple(values) => values.iter().any(expr_has_mut_borrow),
        Expr::Object { fields, .. } => fields.iter().any(|(_, expr)| expr_has_mut_borrow(expr)),
        Expr::Field { base, .. } | Expr::Propagate(base) => expr_has_mut_borrow(base),
        Expr::Index { base, index } => expr_has_mut_borrow(base) || expr_has_mut_borrow(index),
        Expr::Call { callee, args } => {
            expr_has_mut_borrow(callee) || args.iter().any(expr_has_mut_borrow)
        }
        Expr::Unary { expr, .. } => expr_has_mut_borrow(expr),
        Expr::Binary { lhs, rhs, .. } => expr_has_mut_borrow(lhs) || expr_has_mut_borrow(rhs),
        Expr::Ternary {
            cond,
            then_expr,
            else_expr,
        } => {
            expr_has_mut_borrow(cond)
                || expr_has_mut_borrow(then_expr)
                || expr_has_mut_borrow(else_expr)
        }
        Expr::Lambda { body, .. } => expr_has_mut_borrow(body),
        _ => false,
    }
}
