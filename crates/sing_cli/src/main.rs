use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

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
    /// Stub for the future formatter.
    Fmt { file: PathBuf },
    /// Stub for the future minifier.
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
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Ast { file } => print_ast(file),
        Command::Check { file } => check(file),
        Command::Fmt { file } => stub("fmt", file),
        Command::Min { file } => stub("min", file),
        Command::Tokens { file } => tokens(file),
        Command::Run { file } => run(file),
        Command::Build { file, out, target } => build(file, out, target),
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

fn stub(command: &str, file: PathBuf) -> ExitCode {
    let _ = file;
    eprintln!("sing {command} is reserved for a later milestone");
    ExitCode::SUCCESS
}
