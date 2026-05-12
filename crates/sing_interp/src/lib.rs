use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sing_ast::{BinaryOp, Block, Expr, FnDecl, ItemKind, Stmt, UnaryOp};
use sing_parse::parse_file;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Void,
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunOutput {
    pub value: Value,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum RunError {
    #[error("parse failed: {0}")]
    Parse(String),
    #[error("missing main function")]
    MissingMain,
    #[error("unknown function `{0}`")]
    UnknownFunction(String),
    #[error("unknown name `{0}`")]
    UnknownName(String),
    #[error("unsupported expression")]
    UnsupportedExpr,
    #[error("unsupported statement")]
    UnsupportedStmt,
    #[error("type error: {0}")]
    Type(String),
}

pub fn run_source(src: &str) -> Result<RunOutput, RunError> {
    let file = parse_file(src).map_err(|errors| RunError::Parse(format!("{errors:#?}")))?;
    let mut functions = HashMap::new();
    for item in &file.items {
        if let ItemKind::Fn(decl) = &item.kind {
            let name = decl
                .sig
                .name
                .0
                .iter()
                .map(|part| part.0.clone())
                .collect::<Vec<_>>()
                .join(".");
            functions.insert(name, decl.clone());
        }
    }

    let mut interp = Interpreter {
        functions,
        output: String::new(),
    };
    let value = interp.call("main", Vec::new())?;
    Ok(RunOutput {
        value,
        output: interp.output,
    })
}

struct Interpreter {
    functions: HashMap<String, FnDecl>,
    output: String,
}

impl Interpreter {
    fn call(&mut self, name: &str, args: Vec<Value>) -> Result<Value, RunError> {
        match name {
            "out" => {
                if let Some(value) = args.first() {
                    self.output.push_str(&value.as_output_string());
                }
                return Ok(Value::Void);
            }
            "sqrt" => {
                let value = args
                    .first()
                    .ok_or_else(|| RunError::Type("sqrt expects one arg".to_string()))?;
                return Ok(Value::Float(value.as_f64()?.sqrt()));
            }
            _ => {}
        }

        let function = self.functions.get(name).cloned().ok_or_else(|| {
            if name == "main" {
                RunError::MissingMain
            } else {
                RunError::UnknownFunction(name.to_string())
            }
        })?;

        let mut env = HashMap::new();
        let mut arg_iter = args.into_iter();
        for param in &function.sig.params {
            for name in &param.names {
                env.insert(name.0.clone(), arg_iter.next().unwrap_or(Value::Void));
            }
        }

        if let Some(body) = &function.body {
            match self.eval_block(body, &mut env)? {
                Control::Value(value) | Control::Return(value) => Ok(value),
            }
        } else {
            Ok(Value::Void)
        }
    }

    fn eval_block(
        &mut self,
        block: &Block,
        env: &mut HashMap<String, Value>,
    ) -> Result<Control, RunError> {
        let mut last = Value::Void;
        for stmt in &block.stmts {
            match self.eval_stmt(stmt, env)? {
                Control::Value(value) => last = value,
                control @ Control::Return(_) => return Ok(control),
            }
        }
        Ok(Control::Value(last))
    }

    fn eval_stmt(
        &mut self,
        stmt: &Stmt,
        env: &mut HashMap<String, Value>,
    ) -> Result<Control, RunError> {
        match stmt {
            Stmt::Return(expr) => Ok(Control::Return(self.eval_expr(expr, env)?)),
            Stmt::Bind { name, expr, .. } => {
                let value = self.eval_expr(expr, env)?;
                env.insert(name.0.clone(), value.clone());
                Ok(Control::Value(value))
            }
            Stmt::Expr(expr) => Ok(Control::Value(self.eval_expr(expr, env)?)),
            Stmt::Require(expr) | Stmt::Ensure(expr) => {
                let value = self.eval_expr(expr, env)?;
                if value.truthy() {
                    Ok(Control::Value(Value::Void))
                } else {
                    Err(RunError::Type("contract failed".to_string()))
                }
            }
            _ => Err(RunError::UnsupportedStmt),
        }
    }

    fn eval_expr(
        &mut self,
        expr: &Expr,
        env: &mut HashMap<String, Value>,
    ) -> Result<Value, RunError> {
        match expr {
            Expr::Int(value) => Ok(Value::Int(parse_int(value))),
            Expr::Float(value) => Ok(Value::Float(parse_float(value))),
            Expr::String(value) => Ok(Value::String(value.clone())),
            Expr::Char(value) => Ok(Value::String(value.to_string())),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::None | Expr::Missing => Ok(Value::Void),
            Expr::Ident(name) => env
                .get(&name.0)
                .cloned()
                .ok_or_else(|| RunError::UnknownName(name.0.clone())),
            Expr::Unary { op, expr } => {
                let value = self.eval_expr(expr, env)?;
                match op {
                    UnaryOp::Neg => Ok(Value::Int(-value.as_i64()?)),
                    UnaryOp::Not => Ok(Value::Bool(!value.truthy())),
                    _ => Err(RunError::UnsupportedExpr),
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let left = self.eval_expr(lhs, env)?;
                let right = self.eval_expr(rhs, env)?;
                eval_binary(op, left, right)
            }
            Expr::Ternary {
                cond,
                then_expr,
                else_expr,
            } => {
                if self.eval_expr(cond, env)?.truthy() {
                    self.eval_expr(then_expr, env)
                } else {
                    self.eval_expr(else_expr, env)
                }
            }
            Expr::Call { callee, args } => {
                let name = callee_name(callee)?;
                let args = args
                    .iter()
                    .map(|arg| self.eval_expr(arg, env))
                    .collect::<Result<Vec<_>, _>>()?;
                self.call(&name, args)
            }
            _ => Err(RunError::UnsupportedExpr),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Control {
    Value(Value),
    Return(Value),
}

impl Value {
    fn as_i64(&self) -> Result<i64, RunError> {
        match self {
            Value::Int(value) => Ok(*value),
            other => Err(RunError::Type(format!("expected int, got {other:?}"))),
        }
    }

    fn as_f64(&self) -> Result<f64, RunError> {
        match self {
            Value::Int(value) => Ok(*value as f64),
            Value::Float(value) => Ok(*value),
            other => Err(RunError::Type(format!("expected number, got {other:?}"))),
        }
    }

    fn truthy(&self) -> bool {
        match self {
            Value::Void => false,
            Value::Int(value) => *value != 0,
            Value::Float(value) => *value != 0.0,
            Value::Bool(value) => *value,
            Value::String(value) => !value.is_empty(),
        }
    }

    fn as_output_string(&self) -> String {
        match self {
            Value::Void => String::new(),
            Value::Int(value) => value.to_string(),
            Value::Float(value) => value.to_string(),
            Value::Bool(value) => value.to_string(),
            Value::String(value) => value.clone(),
        }
    }
}

fn eval_binary(op: &BinaryOp, left: Value, right: Value) -> Result<Value, RunError> {
    match op {
        BinaryOp::Add => Ok(Value::Int(left.as_i64()? + right.as_i64()?)),
        BinaryOp::Sub => Ok(Value::Int(left.as_i64()? - right.as_i64()?)),
        BinaryOp::Mul => Ok(Value::Int(left.as_i64()? * right.as_i64()?)),
        BinaryOp::Div => Ok(Value::Int(left.as_i64()? / right.as_i64()?)),
        BinaryOp::Mod => Ok(Value::Int(left.as_i64()? % right.as_i64()?)),
        BinaryOp::Eq => Ok(Value::Bool(left == right)),
        BinaryOp::Ne => Ok(Value::Bool(left != right)),
        BinaryOp::Lt => Ok(Value::Bool(left.as_i64()? < right.as_i64()?)),
        BinaryOp::Le => Ok(Value::Bool(left.as_i64()? <= right.as_i64()?)),
        BinaryOp::Gt => Ok(Value::Bool(left.as_i64()? > right.as_i64()?)),
        BinaryOp::Ge => Ok(Value::Bool(left.as_i64()? >= right.as_i64()?)),
        BinaryOp::And => Ok(Value::Bool(left.truthy() && right.truthy())),
        BinaryOp::Or => Ok(Value::Bool(left.truthy() || right.truthy())),
        _ => Err(RunError::UnsupportedExpr),
    }
}

fn callee_name(expr: &Expr) -> Result<String, RunError> {
    match expr {
        Expr::Ident(name) => Ok(name.0.clone()),
        Expr::Path(path) => Ok(path
            .0
            .iter()
            .map(|part| part.0.clone())
            .collect::<Vec<_>>()
            .join(".")),
        Expr::Field { base, name } => Ok(format!("{}.{}", callee_name(base)?, name.0)),
        _ => Err(RunError::UnsupportedExpr),
    }
}

fn parse_int(value: &str) -> i64 {
    let digits = value.trim_end_matches(|ch: char| ch.is_ascii_alphabetic());
    if let Some(hex) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16).unwrap_or(0)
    } else if let Some(bin) = digits
        .strip_prefix("0b")
        .or_else(|| digits.strip_prefix("0B"))
    {
        i64::from_str_radix(bin, 2).unwrap_or(0)
    } else {
        digits.parse().unwrap_or(0)
    }
}

fn parse_float(value: &str) -> f64 {
    value
        .trim_end_matches(|ch: char| ch.is_ascii_alphabetic())
        .parse()
        .unwrap_or(0.0)
}
