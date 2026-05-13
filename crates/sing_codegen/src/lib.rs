use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use cranelift_codegen::{
    ir::{types, AbiParam, InstBuilder},
    settings::{self, Configurable},
};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{default_libcall_names, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use serde::{Deserialize, Serialize};
use sing_interp::run_source;
use sing_mir::{lower_source, MirProgram, MirTerminator, Rvalue};
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
    let direct = emit_direct_object(&mir, &direct_object_candidate, &target)?;
    let backend_ir = backend_ir(&mir, &target, direct.as_ref());
    let codegen_strategy = direct
        .as_ref()
        .map(|_| "cranelift-object-alpha")
        .unwrap_or("oracle-linked-launcher")
        .to_string();
    let direct_native = direct.is_some();
    let direct_object_path = direct.as_ref().map(|artifact| artifact.path.clone());
    let fallback_reason = direct
        .as_ref()
        .and_then(|artifact| artifact.fallback_reason.clone())
        .or_else(|| {
            if direct_native {
                None
            } else {
                Some(
                    direct_constant_main(&mir)
                        .err()
                        .unwrap_or_else(|| "unsupported direct Cranelift subset".to_string()),
                )
            }
        });

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
            "direct cranelift-object-alpha main={}\n",
            direct.main_return
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
struct DirectArtifact {
    path: PathBuf,
    main_return: i64,
    fallback_reason: Option<String>,
}

fn emit_direct_object(
    mir: &MirProgram,
    path: &Path,
    target: &str,
) -> Result<Option<DirectArtifact>, BuildError> {
    let main_return = match direct_constant_main(mir) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    if target != host_target() {
        return Ok(Some(DirectArtifact {
            path: path.to_path_buf(),
            main_return,
            fallback_reason: Some(
                "direct Cranelift currently supports host target only".to_string(),
            ),
        }));
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
        let block = builder.create_block();
        builder.switch_to_block(block);
        builder.seal_block(block);
        let value = builder.ins().iconst(types::I64, main_return);
        builder.ins().return_(&[value]);
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

    Ok(Some(DirectArtifact {
        path: path.to_path_buf(),
        main_return,
        fallback_reason: None,
    }))
}

fn direct_constant_main(mir: &MirProgram) -> Result<i64, String> {
    let main = mir
        .functions
        .iter()
        .find(|func| func.name == "main")
        .ok_or_else(|| "unsupported direct Cranelift subset: missing main".to_string())?;
    if main.blocks.len() != 1 || !main.blocks[0].ops.is_empty() {
        return Err(
            "unsupported direct Cranelift subset: main must be one return block".to_string(),
        );
    }
    match &main.blocks[0].term {
        MirTerminator::Return(Some(Rvalue::ConstInt(value))) => Ok(*value),
        _ => Err(
            "unsupported direct Cranelift subset: main must return a constant integer".to_string(),
        ),
    }
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
