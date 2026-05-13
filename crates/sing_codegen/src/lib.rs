use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use cranelift_codegen::{
    ir::{condcodes::IntCC, types, AbiParam, InstBuilder, Value},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{default_libcall_names, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use serde::{Deserialize, Serialize};
use sing_contract::{contract_ref, ContractRef, SchemaKind};
use sing_interp::run_source;
use sing_mir::{lower_source, MirFunction, MirOp, MirProgram, MirTerminator, Rvalue};
use thiserror::Error;

const BACKEND: &str = "cranelift-alpha";
const ABI: &str = "sing-v1-alpha";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildOptions {
    pub target: Option<String>,
    pub emit_debug: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    pub size: usize,
    pub align: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildReport {
    pub contract: ContractRef,
    pub backend: String,
    pub abi: String,
    pub target: String,
    pub codegen_strategy: String,
    pub direct_native: bool,
    pub backend_ir: String,
    pub object_path: PathBuf,
    pub direct_object_path: Option<PathBuf>,
    pub binary_path: PathBuf,
    pub debug_path: Option<PathBuf>,
    pub fallback_reason: Option<String>,
    pub layouts: Vec<Layout>,
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("failed to parse/lower source: {0:?}")]
    Lower(Vec<String>),
    #[error("interpreter oracle failed: {0}")]
    Oracle(String),
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("failed to serialize build artifact: {0}")]
    Json(#[from] serde_json::Error),
    #[error("rustc failed while {phase}: {stderr}")]
    Rustc { phase: &'static str, stderr: String },
    #[error("direct codegen failed: {0}")]
    Direct(String),
}

pub fn build_source_to_dir(
    src: &str,
    out_dir: impl AsRef<Path>,
    options: BuildOptions,
) -> Result<BuildReport, BuildError> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    let target = options.target.unwrap_or_else(host_target);
    let mir = lower_source(src).map_err(BuildError::Lower)?;
    let layouts = layouts();
    let object_path = out_dir.join(object_name("main"));
    let direct_object_candidate = out_dir.join(direct_object_name("main"));
    let binary_path = out_dir.join(binary_name("main"));
    let debug_path = options
        .emit_debug
        .then(|| out_dir.join("main.singdbg.json"));
    let source_path = out_dir.join("main.singlink.rs");
    let ir_path = out_dir.join("main.clif.txt");
    let direct_attempt = emit_direct_object(&mir, &direct_object_candidate, &target)?;
    let (direct, fallback_reason) = match direct_attempt {
        DirectAttempt::Emitted(artifact) => (Some(artifact), None),
        DirectAttempt::Fallback(reason) => (None, Some(reason)),
    };
    let backend_ir = backend_ir(&mir, &target, direct.as_ref());
    let codegen_strategy = direct
        .as_ref()
        .map(|_| "cranelift-object-alpha")
        .unwrap_or("oracle-linked-launcher")
        .to_string();
    let direct_native = direct.is_some();
    let direct_object_path = direct.as_ref().map(|artifact| artifact.path.clone());

    fs::write(&ir_path, &backend_ir)?;
    let run = run_source(src).map_err(|error| BuildError::Oracle(error.to_string()))?;
    fs::write(&source_path, launcher_source(&run.output, src, &target))?;

    invoke_rustc(
        "emitting object",
        rustc_command(
            &source_path,
            &object_path,
            &target,
            true,
            options.emit_debug,
        ),
    )?;
    invoke_rustc(
        "linking binary",
        rustc_command(
            &source_path,
            &binary_path,
            &target,
            false,
            options.emit_debug,
        ),
    )?;

    if let Some(path) = &debug_path {
        let debug = serde_json::json!({
            "contract": contract_ref(SchemaKind::Build),
            "backend": BACKEND,
            "abi": ABI,
            "target": target,
            "codegen_strategy": codegen_strategy,
            "direct_native": direct_native,
            "source_hash": stable_hash(src),
            "mir_functions": mir.functions.iter().map(|func| func.name.clone()).collect::<Vec<_>>(),
            "backend_ir": ir_path,
            "object": object_path,
            "direct_object": direct_object_path,
            "binary": binary_path,
            "fallback_reason": fallback_reason,
        });
        fs::write(path, serde_json::to_string_pretty(&debug)?)?;
    }

    Ok(BuildReport {
        contract: contract_ref(SchemaKind::Build),
        backend: BACKEND.to_string(),
        abi: ABI.to_string(),
        target,
        codegen_strategy,
        direct_native,
        backend_ir,
        object_path,
        direct_object_path,
        binary_path,
        debug_path,
        fallback_reason,
        layouts,
    })
}

fn rustc_command(
    source_path: &Path,
    output_path: &Path,
    target: &str,
    object: bool,
    debug: bool,
) -> Command {
    let mut command = Command::new("rustc");
    command
        .arg("--edition=2021")
        .arg("--crate-name")
        .arg("sing_link")
        .arg(source_path)
        .arg("-O")
        .arg("-o")
        .arg(output_path);
    if object {
        command.arg("--emit=obj");
    }
    if debug {
        command.arg("-g");
    }
    if target != host_target() {
        command.arg("--target").arg(target);
    }
    command
}

fn invoke_rustc(phase: &'static str, mut command: Command) -> Result<(), BuildError> {
    let output = command.output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(BuildError::Rustc {
            phase,
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

fn backend_ir(mir: &MirProgram, target: &str, direct: Option<&DirectArtifact>) -> String {
    let mut ir = String::new();
    ir.push_str(&format!("backend {BACKEND}\n"));
    ir.push_str(&format!("target {target}\n"));
    ir.push_str(&format!("abi {ABI}\n"));
    if let Some(direct) = direct {
        ir.push_str(&format!(
            "direct cranelift-object-alpha {}\n",
            direct.summary
        ));
    } else {
        ir.push_str("direct fallback oracle-linked-launcher\n");
    }
    for func in &mir.functions {
        ir.push_str(&format!("fn {}\n", func.name));
        for block in &func.blocks {
            ir.push_str(&format!("  b{} reachable={}\n", block.id, block.reachable));
            for op in &block.ops {
                ir.push_str(&format!("    op {op:?}\n"));
            }
            ir.push_str(&format!("    term {:?}\n", block.term));
        }
    }
    ir
}

#[derive(Debug, Clone)]
enum DirectAttempt {
    Emitted(DirectArtifact),
    Fallback(String),
}

#[derive(Debug, Clone)]
struct DirectArtifact {
    path: PathBuf,
    summary: String,
}

#[derive(Debug, Clone)]
struct DirectMain<'a> {
    func: &'a MirFunction,
    summary: String,
}

fn emit_direct_object(
    mir: &MirProgram,
    path: &Path,
    target: &str,
) -> Result<DirectAttempt, BuildError> {
    let direct = match direct_main(mir) {
        Ok(direct) => direct,
        Err(reason) => return Ok(DirectAttempt::Fallback(reason)),
    };
    if target != host_target() {
        return Ok(DirectAttempt::Fallback(
            "unsupported direct Cranelift subset: host target only".to_string(),
        ));
    }

    let mut flag_builder = settings::builder();
    flag_builder
        .set("is_pic", "true")
        .map_err(|error| BuildError::Direct(error.to_string()))?;
    let isa_builder = cranelift_native::builder()
        .map_err(|error| BuildError::Direct(format!("native target error: {error}")))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|error| BuildError::Direct(error.to_string()))?;
    let builder = ObjectBuilder::new(isa, "sing_main", default_libcall_names())
        .map_err(|error| BuildError::Direct(error.to_string()))?;
    let mut module = ObjectModule::new(builder);
    let mut ctx = module.make_context();
    ctx.func.signature.returns.push(AbiParam::new(types::I64));
    let func_id = module
        .declare_function("sing_main", Linkage::Export, &ctx.func.signature)
        .map_err(|error| BuildError::Direct(error.to_string()))?;

    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);
        emit_direct_function(&mut builder, direct.func).map_err(BuildError::Direct)?;
        builder.finalize();
    }

    module
        .define_function(func_id, &mut ctx)
        .map_err(|error| BuildError::Direct(error.to_string()))?;
    module.clear_context(&mut ctx);
    let product = module.finish();
    let bytes = product
        .emit()
        .map_err(|error| BuildError::Direct(error.to_string()))?;
    fs::write(path, bytes)?;

    Ok(DirectAttempt::Emitted(DirectArtifact {
        path: path.to_path_buf(),
        summary: direct.summary,
    }))
}

fn direct_main(mir: &MirProgram) -> Result<DirectMain<'_>, String> {
    let main = mir
        .functions
        .iter()
        .find(|func| func.name == "main")
        .ok_or_else(|| "unsupported direct Cranelift subset: missing main".to_string())?;
    validate_direct_main(main)?;
    let op_count = main
        .blocks
        .iter()
        .map(|block| block.ops.len())
        .sum::<usize>();
    let summary = if let Some(value) = direct_constant_return(main) {
        format!(
            "main={value} subset=int-main-v2 blocks={} ops={op_count}",
            main.blocks.len()
        )
    } else {
        format!(
            "subset=int-main-v2 blocks={} ops={op_count}",
            main.blocks.len()
        )
    };
    Ok(DirectMain {
        func: main,
        summary,
    })
}

fn direct_constant_return(func: &MirFunction) -> Option<i64> {
    if func.blocks.len() != 1 || !func.blocks[0].ops.is_empty() {
        return None;
    }
    match &func.blocks[0].term {
        MirTerminator::Return(Some(Rvalue::ConstInt(value))) => Some(*value),
        _ => None,
    }
}

fn validate_direct_main(func: &MirFunction) -> Result<(), String> {
    if func.blocks.is_empty() || func.entry >= func.blocks.len() {
        return Err("unsupported direct Cranelift subset: malformed main MIR".to_string());
    }
    if func.blocks.iter().any(|block| !block.reachable) {
        return Err(
            "unsupported direct Cranelift subset: dead blocks need MIR cleanup".to_string(),
        );
    }

    let mut assigned = BTreeSet::new();
    for block in &func.blocks {
        for op in &block.ops {
            match op {
                MirOp::Assign { target, value } => {
                    validate_direct_rvalue(value, &assigned)?;
                    assigned.insert(target.clone());
                }
                MirOp::Mutate { .. } => {
                    return Err(
                        "unsupported direct Cranelift subset: mutation is not native yet"
                            .to_string(),
                    );
                }
                MirOp::Eval(_) | MirOp::Require(_) | MirOp::Ensure(_) => {
                    return Err(
                        "unsupported direct Cranelift subset: only local binds and returns are native"
                            .to_string(),
                    );
                }
            }
        }

        match &block.term {
            MirTerminator::Return(Some(value)) => validate_direct_rvalue(value, &assigned)?,
            MirTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                validate_direct_rvalue(cond, &assigned)?;
                if *then_block >= func.blocks.len() || *else_block >= func.blocks.len() {
                    return Err(
                        "unsupported direct Cranelift subset: branch target is malformed"
                            .to_string(),
                    );
                }
            }
            MirTerminator::Return(None) => {
                return Err(
                    "unsupported direct Cranelift subset: void main return is not native yet"
                        .to_string(),
                );
            }
            MirTerminator::Goto(_) | MirTerminator::Unreachable => {
                return Err(
                    "unsupported direct Cranelift subset: only return and ternary branch control flow is native"
                        .to_string(),
                );
            }
        }
    }

    Ok(())
}

fn validate_direct_rvalue(value: &Rvalue, assigned: &BTreeSet<String>) -> Result<(), String> {
    match value {
        Rvalue::ConstInt(_) | Rvalue::ConstBool(_) => Ok(()),
        Rvalue::Use(name) if assigned.contains(name) => Ok(()),
        Rvalue::Use(_) => {
            Err("unsupported direct Cranelift subset: external values are not native yet".to_string())
        }
        Rvalue::Unary { op, value } if matches!(op.as_str(), "Neg" | "Not") => {
            validate_direct_rvalue(value, assigned)
        }
        Rvalue::Binary { op, lhs, rhs } if is_direct_binary_op(op) => {
            validate_direct_rvalue(lhs, assigned)?;
            validate_direct_rvalue(rhs, assigned)
        }
        _ => Err(
            "unsupported direct Cranelift subset: integer main only supports binds, arithmetic, comparisons, and ternary"
                .to_string(),
        ),
    }
}

fn is_direct_binary_op(op: &str) -> bool {
    matches!(
        op,
        "Add" | "Sub" | "Mul" | "Div" | "Mod" | "Lt" | "Le" | "Gt" | "Ge" | "Eq" | "Ne"
    )
}

fn emit_direct_function(
    builder: &mut FunctionBuilder<'_>,
    func: &MirFunction,
) -> Result<(), String> {
    let blocks = (0..func.blocks.len())
        .map(|_| builder.create_block())
        .collect::<Vec<_>>();
    let mut vars = BTreeMap::new();

    for block in &func.blocks {
        builder.switch_to_block(blocks[block.id]);
        for op in &block.ops {
            match op {
                MirOp::Assign { target, value } => {
                    let value = emit_direct_rvalue(builder, &mut vars, value)?;
                    let var = direct_var(builder, &mut vars, target);
                    builder.def_var(var, value);
                }
                _ => return Err("validated direct MIR contained unsupported op".to_string()),
            }
        }

        match &block.term {
            MirTerminator::Return(Some(value)) => {
                let value = emit_direct_rvalue(builder, &mut vars, value)?;
                builder.ins().return_(&[value]);
            }
            MirTerminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cond = emit_direct_rvalue(builder, &mut vars, cond)?;
                let truthy = builder.ins().icmp_imm(IntCC::NotEqual, cond, 0);
                builder
                    .ins()
                    .brif(truthy, blocks[*then_block], &[], blocks[*else_block], &[]);
            }
            _ => return Err("validated direct MIR contained unsupported terminator".to_string()),
        }
    }

    builder.seal_all_blocks();
    Ok(())
}

fn direct_var(
    builder: &mut FunctionBuilder<'_>,
    vars: &mut BTreeMap<String, Variable>,
    name: &str,
) -> Variable {
    if let Some(var) = vars.get(name) {
        *var
    } else {
        let var = builder.declare_var(types::I64);
        vars.insert(name.to_string(), var);
        var
    }
}

fn emit_direct_rvalue(
    builder: &mut FunctionBuilder<'_>,
    vars: &mut BTreeMap<String, Variable>,
    value: &Rvalue,
) -> Result<Value, String> {
    match value {
        Rvalue::ConstInt(value) => Ok(builder.ins().iconst(types::I64, *value)),
        Rvalue::ConstBool(value) => Ok(builder.ins().iconst(types::I64, i64::from(*value))),
        Rvalue::Use(name) => {
            let var = vars
                .get(name)
                .ok_or_else(|| format!("direct Cranelift used undefined local {name}"))?;
            Ok(builder.use_var(*var))
        }
        Rvalue::Unary { op, value } if op == "Neg" => {
            let value = emit_direct_rvalue(builder, vars, value)?;
            Ok(builder.ins().ineg(value))
        }
        Rvalue::Unary { op, value } if op == "Not" => {
            let value = emit_direct_rvalue(builder, vars, value)?;
            let cmp = builder.ins().icmp_imm(IntCC::Equal, value, 0);
            Ok(bool_to_i64(builder, cmp))
        }
        Rvalue::Binary { op, lhs, rhs } => {
            let lhs = emit_direct_rvalue(builder, vars, lhs)?;
            let rhs = emit_direct_rvalue(builder, vars, rhs)?;
            match op.as_str() {
                "Add" => Ok(builder.ins().iadd(lhs, rhs)),
                "Sub" => Ok(builder.ins().isub(lhs, rhs)),
                "Mul" => Ok(builder.ins().imul(lhs, rhs)),
                "Div" => Ok(builder.ins().sdiv(lhs, rhs)),
                "Mod" => Ok(builder.ins().srem(lhs, rhs)),
                "Lt" => Ok(emit_compare(builder, IntCC::SignedLessThan, lhs, rhs)),
                "Le" => Ok(emit_compare(
                    builder,
                    IntCC::SignedLessThanOrEqual,
                    lhs,
                    rhs,
                )),
                "Gt" => Ok(emit_compare(builder, IntCC::SignedGreaterThan, lhs, rhs)),
                "Ge" => Ok(emit_compare(
                    builder,
                    IntCC::SignedGreaterThanOrEqual,
                    lhs,
                    rhs,
                )),
                "Eq" => Ok(emit_compare(builder, IntCC::Equal, lhs, rhs)),
                "Ne" => Ok(emit_compare(builder, IntCC::NotEqual, lhs, rhs)),
                _ => Err(format!("unsupported direct binary op {op}")),
            }
        }
        _ => Err("unsupported direct rvalue escaped validation".to_string()),
    }
}

fn emit_compare(builder: &mut FunctionBuilder<'_>, cc: IntCC, lhs: Value, rhs: Value) -> Value {
    let cond = builder.ins().icmp(cc, lhs, rhs);
    bool_to_i64(builder, cond)
}

fn bool_to_i64(builder: &mut FunctionBuilder<'_>, cond: Value) -> Value {
    let one = builder.ins().iconst(types::I64, 1);
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().select(cond, one, zero)
}

fn launcher_source(output: &str, src: &str, target: &str) -> String {
    let output = format!("{output:?}");
    let hash = stable_hash(src);
    format!(
        r#"// Sing native alpha launcher
// backend: {BACKEND}
// target: {target}
// source_hash: {hash}
fn main() {{
    print!("{{}}", {output});
}}
"#
    )
}

fn layouts() -> Vec<Layout> {
    [
        ("b", 1, 1),
        ("i1", 1, 1),
        ("i2", 2, 2),
        ("i4", 4, 4),
        ("i8", 8, 8),
        (
            "iz",
            std::mem::size_of::<isize>(),
            std::mem::align_of::<isize>(),
        ),
        ("u1", 1, 1),
        ("u2", 2, 2),
        ("u4", 4, 4),
        ("u8", 8, 8),
        (
            "uz",
            std::mem::size_of::<usize>(),
            std::mem::align_of::<usize>(),
        ),
        ("f4", 4, 4),
        ("f8", 8, 8),
        ("c", 4, 4),
        ("y", 1, 1),
        ("v", 0, 1),
    ]
    .into_iter()
    .map(|(name, size, align)| Layout {
        name: name.to_string(),
        size,
        align,
    })
    .collect()
}

fn object_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.obj")
    } else {
        format!("{stem}.o")
    }
}

fn direct_object_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.direct.obj")
    } else {
        format!("{stem}.direct.o")
    }
}

fn binary_name(stem: &str) -> String {
    if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    }
}

fn host_target() -> String {
    let arch = std::env::consts::ARCH;
    let os = match std::env::consts::OS {
        "windows" => "pc-windows-msvc",
        "macos" => "apple-darwin",
        "linux" => "unknown-linux-gnu",
        other => other,
    };
    format!("{arch}-{os}")
}

fn stable_hash(src: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in src.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
