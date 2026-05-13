use std::{fs, io::Read, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use serde_json::json;
use sing_ast::{
    Block, File, FnDecl, FnSig, Ident, Item, ItemKind, Path, Prim, Stmt, TestDecl, Type,
};
use sing_interp::Value;

#[derive(Debug, Parser)]
#[command(name = "sing")]
#[command(about = "Sing v1-alpha compiler tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the parsed JSON AST for a .sg file.
    Ast { file: PathBuf },
    /// Print compact JSON diagnostics and typed HIR for a .sg file.
    Check { file: PathBuf },
    /// Print canonical formatted Sing source.
    Fmt { file: PathBuf },
    /// Print canonical minimum-token Sing source.
    Min { file: PathBuf },
    /// Print compact JSON token-cost metrics for a source file.
    Tokens { file: PathBuf },
    /// Execute a .sg file with the interpreter oracle.
    Run { file: PathBuf },
    /// Build a native alpha executable for a .sg file.
    Build {
        file: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        target: Option<String>,
    },
    /// Run top-level # tests in a .sg file.
    Test { file: PathBuf },
    /// Print compact JSON docs for a .sg file.
    Doc { file: PathBuf },
    /// Evaluate a small expression or stdin snippet with the interpreter oracle.
    Repl {
        #[arg(long)]
        eval: Option<String>,
    },
    /// Explain a stable diagnostic code as compact JSON.
    Explain { code: String },
    /// Print package manifest, lock, and runtime contract JSON.
    Pkg {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// Print LSP-ready document facts as compact JSON.
    Lsp { file: PathBuf },
    /// Expand deterministic v1-alpha macros and print compact JSON.
    Expand { file: PathBuf },
    /// Print the production contract and schema versions as compact JSON.
    Contract,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Ast { file } => print_ast(file),
        Command::Check { file } => check(file),
        Command::Fmt { file } => fmt(file),
        Command::Min { file } => min(file),
        Command::Tokens { file } => tokens(file),
        Command::Run { file } => run(file),
        Command::Build { file, out, target } => build(file, out, target),
        Command::Test { file } => test(file),
        Command::Doc { file } => doc(file),
        Command::Repl { eval } => repl(eval),
        Command::Explain { code } => explain(code),
        Command::Pkg { root } => pkg(root),
        Command::Lsp { file } => lsp(file),
        Command::Expand { file } => expand(file),
        Command::Contract => contract(),
    }
}

fn print_ast(file: PathBuf) -> ExitCode {
    let src = match fs::read_to_string(&file) {
        Ok(src) => src,
        Err(error) => {
            eprintln!("failed to read {}: {error}", file.display());
            return ExitCode::FAILURE;
        }
    };

    match sing_parse::parse_file(&src) {
        Ok(ast) => match serde_json::to_string_pretty(&ast) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to serialize AST: {error}");
                ExitCode::FAILURE
            }
        },
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

fn check(file: PathBuf) -> ExitCode {
    let src = match fs::read_to_string(&file) {
        Ok(src) => src,
        Err(error) => {
            eprintln!("failed to read {}: {error}", file.display());
            return ExitCode::FAILURE;
        }
    };

    let checked = sing_sem::check_source(&src);
    match serde_json::to_string(&checked) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("failed to serialize check output: {error}");
            return ExitCode::FAILURE;
        }
    }

    if checked.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn tokens(file: PathBuf) -> ExitCode {
    let src = match fs::read_to_string(&file) {
        Ok(src) => src,
        Err(error) => {
            eprintln!("failed to read {}: {error}", file.display());
            return ExitCode::FAILURE;
        }
    };

    match serde_json::to_string(&sing_token::token_cost(&src)) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to serialize token cost: {error}");
            ExitCode::FAILURE
        }
    }
}

fn fmt(file: PathBuf) -> ExitCode {
    let src = match read_source(&file) {
        Ok(src) => src,
        Err(code) => return code,
    };
    match sing_fmt::format_source(&src) {
        Ok(formatted) => {
            print!("{formatted}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("format failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn min(file: PathBuf) -> ExitCode {
    let src = match read_source(&file) {
        Ok(src) => src,
        Err(code) => return code,
    };
    match sing_fmt::minify_source(&src) {
        Ok(minimized) => {
            print!("{minimized}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("min failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(file: PathBuf) -> ExitCode {
    let src = match fs::read_to_string(&file) {
        Ok(src) => src,
        Err(error) => {
            eprintln!("failed to read {}: {error}", file.display());
            return ExitCode::FAILURE;
        }
    };

    match sing_interp::run_source(&src) {
        Ok(run) => {
            print!("{}", run.output);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn build(file: PathBuf, out: Option<PathBuf>, target: Option<String>) -> ExitCode {
    let src = match fs::read_to_string(&file) {
        Ok(src) => src,
        Err(error) => {
            eprintln!("failed to read {}: {error}", file.display());
            return ExitCode::FAILURE;
        }
    };

    let stem = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("main");
    let out_dir = out.unwrap_or_else(|| PathBuf::from("target").join("sing-build").join(stem));
    let report = match sing_codegen::build_source_to_dir(
        &src,
        &out_dir,
        sing_codegen::BuildOptions {
            target,
            emit_debug: true,
        },
    ) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("build failed: {error}");
            return ExitCode::FAILURE;
        }
    };

    match serde_json::to_string(&report) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to serialize build output: {error}");
            ExitCode::FAILURE
        }
    }
}

fn test(file: PathBuf) -> ExitCode {
    let src = match read_source(&file) {
        Ok(src) => src,
        Err(code) => return code,
    };
    let parsed = match sing_parse::parse_file(&src) {
        Ok(file) => file,
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            return ExitCode::FAILURE;
        }
    };

    let tests = parsed
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Test(test) => Some(test.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let base = File {
        items: parsed
            .items
            .iter()
            .filter(|item| !matches!(item.kind, ItemKind::Test(_)) && !is_main_fn(item))
            .cloned()
            .collect(),
    };

    let mut cases = Vec::new();
    for (idx, test) in tests.iter().enumerate() {
        let name = test
            .name
            .as_ref()
            .map(|name| name.0.clone())
            .unwrap_or_else(|| format!("test{idx}"));
        let source = sing_fmt::minify_file(&test_file(&base, test));
        let outcome = match sing_interp::run_source(&source) {
            Ok(run) if value_truthy(&run.value) => json!({
                "name": name,
                "ok": true
            }),
            Ok(run) => json!({
                "name": name,
                "ok": false,
                "value": value_text(&run.value)
            }),
            Err(error) => json!({
                "name": name,
                "ok": false,
                "error": error.to_string()
            }),
        };
        cases.push(outcome);
    }

    let failed = cases
        .iter()
        .filter(|case| case["ok"].as_bool() != Some(true))
        .count();
    let passed = cases.len() - failed;
    let report = json!({
        "ok": failed == 0,
        "passed": passed,
        "failed": failed,
        "tests": cases
    });
    println!("{report}");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn doc(file: PathBuf) -> ExitCode {
    let src = match read_source(&file) {
        Ok(src) => src,
        Err(code) => return code,
    };
    let parsed = match sing_parse::parse_file(&src) {
        Ok(file) => file,
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            return ExitCode::FAILURE;
        }
    };
    let items = parsed.items.iter().filter_map(doc_item).collect::<Vec<_>>();
    let module = parsed
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Module(path) => Some(path_s(path)),
            _ => None,
        })
        .unwrap_or_default();
    println!(
        "{}",
        json!({
            "module": module,
            "items": items,
            "token_cost": sing_token::token_cost(&src)
        })
    );
    ExitCode::SUCCESS
}

fn repl(eval: Option<String>) -> ExitCode {
    let expr = match eval {
        Some(expr) => expr,
        None => {
            let mut src = String::new();
            if let Err(error) = std::io::stdin().read_to_string(&mut src) {
                eprintln!("failed to read stdin: {error}");
                return ExitCode::FAILURE;
            }
            src
        }
    };
    let src = format!("f main()>*:{}", expr.trim());
    match sing_interp::run_source(&src) {
        Ok(run) => {
            if run.output.is_empty() {
                println!("{}", value_text(&run.value));
            } else {
                print!("{}", run.output);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn explain(code: String) -> ExitCode {
    match serde_json::to_string(&sing_diag::explain(&code)) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to serialize diagnostic explanation: {error}");
            ExitCode::FAILURE
        }
    }
}

fn pkg(root: PathBuf) -> ExitCode {
    match sing_pkg::load_package(&root) {
        Ok(report) => match serde_json::to_string(&report) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to serialize package report: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("package failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn lsp(file: PathBuf) -> ExitCode {
    let src = match read_source(&file) {
        Ok(src) => src,
        Err(code) => return code,
    };
    match serde_json::to_string(&sing_lsp::document_facts(file.display().to_string(), &src)) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to serialize LSP facts: {error}");
            ExitCode::FAILURE
        }
    }
}

fn expand(file: PathBuf) -> ExitCode {
    let src = match read_source(&file) {
        Ok(src) => src,
        Err(code) => return code,
    };
    match sing_macro::expand_source(&src) {
        Ok(expanded) => match serde_json::to_string(&expanded) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to serialize expansion report: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("expand failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn contract() -> ExitCode {
    match serde_json::to_string(&sing_contract::contract_report()) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("failed to serialize contract report: {error}");
            ExitCode::FAILURE
        }
    }
}

fn read_source(file: &PathBuf) -> Result<String, ExitCode> {
    fs::read_to_string(file).map_err(|error| {
        eprintln!("failed to read {}: {error}", file.display());
        ExitCode::FAILURE
    })
}

fn test_file(base: &File, test: &TestDecl) -> File {
    let mut file = base.clone();
    file.items.push(Item {
        attrs: Vec::new(),
        kind: ItemKind::Fn(FnDecl {
            sig: FnSig {
                name: Path(vec![Ident("main".to_string())]),
                generics: Vec::new(),
                params: Vec::new(),
                ret: Some(Type::Prim(Prim::B)),
                effects: Vec::new(),
            },
            body: Some(Block {
                stmts: vec![Stmt::Expr(test.expr.clone())],
            }),
        }),
    });
    file
}

fn is_main_fn(item: &Item) -> bool {
    matches!(&item.kind, ItemKind::Fn(decl) if path_s(&decl.sig.name) == "main")
}

fn doc_item(item: &Item) -> Option<serde_json::Value> {
    match &item.kind {
        ItemKind::Module(_) | ItemKind::Use(_) => None,
        ItemKind::Alias { name, .. } => Some(json!({"k":"alias","n":name.0})),
        ItemKind::Const { name, .. } => Some(json!({"k":"const","n":name.0})),
        ItemKind::Type(decl) => Some(json!({"k":"type","n":decl.name.0})),
        ItemKind::Enum(decl) => Some(json!({"k":"enum","n":decl.name.0})),
        ItemKind::Trait(decl) => Some(json!({"k":"trait","n":decl.name.0})),
        ItemKind::Impl(decl) => Some(json!({"k":"impl","n":decl.trait_name.0})),
        ItemKind::Fn(decl) => Some(json!({"k":"fn","n":path_s(&decl.sig.name)})),
        ItemKind::Extern(sig) => Some(json!({"k":"extern","n":path_s(&sig.name)})),
        ItemKind::Macro(call) => Some(json!({"k":"macro","n":call.name.0})),
        ItemKind::Test(test) => Some(json!({
            "k":"test",
            "n": test.name.as_ref().map(|name| name.0.as_str()).unwrap_or("_")
        })),
    }
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Void => false,
        Value::Int(value) => *value != 0,
        Value::Float(value) => *value != 0.0,
        Value::Bool(value) => *value,
        Value::String(value) => !value.is_empty(),
        Value::Tuple(values) => !values.is_empty(),
        Value::Struct { .. } | Value::Enum { .. } => true,
        Value::Option(value) => value.is_some(),
        Value::ResultOk(_) => true,
        Value::ResultErr(_) => false,
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Void => String::new(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Tuple(values) => values.iter().map(value_text).collect::<Vec<_>>().join(","),
        Value::Struct { ty, .. } => ty.clone(),
        Value::Enum { name, .. } => name.clone(),
        Value::Option(None) => "none".to_string(),
        Value::Option(Some(value)) => value_text(value),
        Value::ResultOk(value) | Value::ResultErr(value) => value_text(value),
    }
}

fn path_s(path: &Path) -> String {
    path.0
        .iter()
        .map(|part| part.0.as_str())
        .collect::<Vec<_>>()
        .join(".")
}
