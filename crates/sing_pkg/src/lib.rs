use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sing_ast::Item;
use sing_contract::{contract_ref, ContractRef, SchemaKind};
use sing_parse::parse_file;
use sing_token::{token_cost, TokenCost};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    pub entry: Option<String>,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LockedSource {
    pub path: String,
    pub hash: String,
    pub cost: TokenCost,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageSource {
    pub path: String,
    pub src: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageLock {
    pub name: String,
    pub version: String,
    pub package_hash: String,
    pub sources: Vec<LockedSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeContract {
    pub heap: String,
    pub panic: String,
    pub runtime: String,
    pub dynamic_dispatch: String,
    pub gc: String,
    pub abi: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageReport {
    pub contract: ContractRef,
    pub manifest: Manifest,
    pub lock: PackageLock,
    pub runtime: RuntimeContract,
}

#[derive(Debug, Error)]
pub enum PackageError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("manifest error: {0}")]
    Manifest(String),
}

pub fn parse_manifest(src: &str) -> Result<Manifest, PackageError> {
    let mut manifest = Manifest {
        name: String::new(),
        version: String::new(),
        entry: None,
        sources: Vec::new(),
    };

    for (line_idx, raw_line) in src.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(PackageError::Manifest(format!(
                "line {} is missing `=`",
                line_idx + 1
            )));
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "n" | "name" => manifest.name = scalar(value).to_string(),
            "v" | "version" => manifest.version = scalar(value).to_string(),
            "e" | "entry" => manifest.entry = Some(scalar(value).to_string()),
            "s" | "source" => manifest.sources.push(scalar(value).to_string()),
            "sources" => manifest.sources.extend(list(value)),
            _ => {
                return Err(PackageError::Manifest(format!(
                    "unknown manifest key `{key}`"
                )))
            }
        }
    }

    if manifest.name.is_empty() {
        return Err(PackageError::Manifest("missing package name".to_string()));
    }
    if manifest.version.is_empty() {
        manifest.version = "0.1.0-alpha.0".to_string();
    }

    Ok(manifest)
}

pub fn load_package(root: &Path) -> Result<PackageReport, PackageError> {
    let manifest = load_manifest(root)?;
    let sources = load_package_sources_with_manifest(root, &manifest)?;

    let mut locked_sources = sources
        .iter()
        .map(|source| LockedSource {
            path: source.path.clone(),
            hash: stable_hash(&source.src),
            cost: token_cost(&source.src),
        })
        .collect::<Vec<_>>();
    locked_sources.sort_by(|a, b| a.path.cmp(&b.path));

    let source_texts = sources
        .into_iter()
        .map(|source| source.src)
        .collect::<Vec<_>>();
    let package_hash = package_hash(&manifest, &locked_sources);
    let runtime = runtime_contract(&source_texts);
    let lock = PackageLock {
        name: manifest.name.clone(),
        version: manifest.version.clone(),
        package_hash,
        sources: locked_sources,
    };

    Ok(PackageReport {
        contract: contract_ref(SchemaKind::Package),
        manifest,
        lock,
        runtime,
    })
}

pub fn load_package_sources(root: &Path) -> Result<Vec<PackageSource>, PackageError> {
    let manifest = load_manifest(root)?;
    load_package_sources_with_manifest(root, &manifest)
}

fn load_manifest(root: &Path) -> Result<Manifest, PackageError> {
    let manifest_path = root.join("Sing.toml");
    let manifest = if manifest_path.exists() {
        parse_manifest(&fs::read_to_string(&manifest_path)?)?
    } else {
        Manifest {
            name: root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("sing")
                .to_string(),
            version: "0.1.0-alpha.0".to_string(),
            entry: None,
            sources: Vec::new(),
        }
    };
    Ok(manifest)
}

fn load_package_sources_with_manifest(
    root: &Path,
    manifest: &Manifest,
) -> Result<Vec<PackageSource>, PackageError> {
    let mut source_paths = if manifest.sources.is_empty() {
        discover_sources(root)?
    } else {
        manifest
            .sources
            .iter()
            .map(|path| root.join(path))
            .collect::<Vec<_>>()
    };
    if let Some(entry) = &manifest.entry {
        let entry_path = root.join(entry);
        if !source_paths.iter().any(|path| path == &entry_path) {
            source_paths.push(entry_path);
        }
    }
    source_paths.sort();
    source_paths.dedup();

    let mut sources = Vec::new();
    for path in source_paths {
        let src = fs::read_to_string(&path)?;
        let rel = rel_path(root, &path);
        sources.push(PackageSource { path: rel, src });
    }
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(sources)
}

fn runtime_contract(sources: &[String]) -> RuntimeContract {
    let mut attrs = Vec::new();
    for src in sources {
        if let Ok(file) = parse_file(src) {
            for item in &file.items {
                collect_attrs(item, &mut attrs);
            }
        }
    }

    RuntimeContract {
        heap: if attrs.iter().any(|attr| attr == "H") {
            "forbidden"
        } else {
            "allowed"
        }
        .to_string(),
        panic: if attrs.iter().any(|attr| attr == "Z") {
            "forbidden"
        } else {
            "allowed"
        }
        .to_string(),
        runtime: if attrs.iter().any(|attr| attr == "R") {
            "none"
        } else {
            "minimal"
        }
        .to_string(),
        dynamic_dispatch: if attrs.iter().any(|attr| attr == "D") {
            "forbidden"
        } else {
            "allowed"
        }
        .to_string(),
        gc: if attrs.iter().any(|attr| attr == "G") {
            "forbidden"
        } else {
            "none"
        }
        .to_string(),
        abi: if attrs.iter().any(|attr| attr == "C") {
            "c"
        } else {
            "sing"
        }
        .to_string(),
    }
}

fn collect_attrs(item: &Item, attrs: &mut Vec<String>) {
    attrs.extend(item.attrs.iter().map(|attr| attr.0.clone()));
    if let sing_ast::ItemKind::Impl(decl) = &item.kind {
        for item in &decl.items {
            collect_attrs(item, attrs);
        }
    }
}

fn discover_sources(root: &Path) -> Result<Vec<PathBuf>, PackageError> {
    let mut sources = Vec::new();
    discover_sources_inner(root, &mut sources)?;
    sources.sort();
    Ok(sources)
}

fn discover_sources_inner(dir: &Path, sources: &mut Vec<PathBuf>) -> Result<(), PackageError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "target" {
            continue;
        }
        if path.is_dir() {
            discover_sources_inner(&path, sources)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("sg") {
            sources.push(path);
        }
    }
    Ok(())
}

fn scalar(value: &str) -> &str {
    value.trim().trim_matches('"').trim_matches('\'').trim()
}

fn list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(scalar)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn package_hash(manifest: &Manifest, sources: &[LockedSource]) -> String {
    let mut text = format!("{}:{}:", manifest.name, manifest.version);
    for source in sources {
        text.push_str(&source.path);
        text.push('=');
        text.push_str(&source.hash);
        text.push(';');
    }
    stable_hash(&text)
}

fn stable_hash(src: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in src.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
