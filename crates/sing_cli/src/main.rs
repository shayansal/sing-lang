use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "sing")]
#[command(about = "Sing v1-alpha parser tools")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print the parsed JSON AST for a .sg file.
    Ast { file: PathBuf },
    /// Stub for the future semantic checker.
    Check { file: PathBuf },
    /// Stub for the future formatter.
    Fmt { file: PathBuf },
    /// Stub for the future minifier.
    Min { file: PathBuf },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Ast { file } => print_ast(file),
        Command::Check { file } => stub("check", file),
        Command::Fmt { file } => stub("fmt", file),
        Command::Min { file } => stub("min", file),
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

fn stub(command: &str, file: PathBuf) -> ExitCode {
    let _ = file;
    eprintln!("sing {command} is reserved for a later milestone");
    ExitCode::SUCCESS
}
