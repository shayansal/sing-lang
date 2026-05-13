use serde::{Deserialize, Serialize};

pub const LANGUAGE_VERSION: &str = "sing-v1-native-draft.1";
pub const CONTRACT_VERSION: &str = "contract.v1";
pub const EDITION: &str = "2026";

pub const DIAGNOSTIC_SCHEMA_VERSION: &str = "diagnostic.v1";
pub const CHECK_SCHEMA_VERSION: &str = "check.v1";
pub const HIR_SCHEMA_VERSION: &str = "hir.v1";
pub const MIR_SCHEMA_VERSION: &str = "mir.v1";
pub const BUILD_SCHEMA_VERSION: &str = "build.v1";
pub const PACKAGE_SCHEMA_VERSION: &str = "package.v1";
pub const MACRO_SCHEMA_VERSION: &str = "macro.v1";
pub const TOKEN_SCHEMA_VERSION: &str = "token.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractRef {
    pub language_version: String,
    pub contract_version: String,
    pub schema_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractReport {
    pub language_version: String,
    pub contract_version: String,
    pub edition: String,
    pub schemas: SchemaVersions,
    pub token_economy: TokenEconomyContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaVersions {
    pub diagnostic: String,
    pub check: String,
    pub hir: String,
    pub mir: String,
    pub build: String,
    pub package: String,
    pub macro_expansion: String,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenEconomyContract {
    pub north_star: String,
    pub metric: String,
    pub default_machine_output: String,
    pub feature_gate: bool,
    pub feature_gate_rule: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    Diagnostic,
    Check,
    Hir,
    Mir,
    Build,
    Package,
    MacroExpansion,
    Token,
}

pub fn contract_ref(kind: SchemaKind) -> ContractRef {
    ContractRef {
        language_version: LANGUAGE_VERSION.to_string(),
        contract_version: CONTRACT_VERSION.to_string(),
        schema_version: schema_version(kind).to_string(),
    }
}

pub fn contract_report() -> ContractReport {
    ContractReport {
        language_version: LANGUAGE_VERSION.to_string(),
        contract_version: CONTRACT_VERSION.to_string(),
        edition: EDITION.to_string(),
        schemas: SchemaVersions {
            diagnostic: DIAGNOSTIC_SCHEMA_VERSION.to_string(),
            check: CHECK_SCHEMA_VERSION.to_string(),
            hir: HIR_SCHEMA_VERSION.to_string(),
            mir: MIR_SCHEMA_VERSION.to_string(),
            build: BUILD_SCHEMA_VERSION.to_string(),
            package: PACKAGE_SCHEMA_VERSION.to_string(),
            macro_expansion: MACRO_SCHEMA_VERSION.to_string(),
            token: TOKEN_SCHEMA_VERSION.to_string(),
        },
        token_economy: TokenEconomyContract {
            north_star: "lowest-total-ai-token-spend".to_string(),
            metric: "bytes + chars + lexemes*4 + llm_tokens*8".to_string(),
            default_machine_output: "compact-json".to_string(),
            feature_gate: true,
            feature_gate_rule:
                "new features must reduce source, diagnostic, IR, or repair-loop token cost"
                    .to_string(),
        },
    }
}

fn schema_version(kind: SchemaKind) -> &'static str {
    match kind {
        SchemaKind::Diagnostic => DIAGNOSTIC_SCHEMA_VERSION,
        SchemaKind::Check => CHECK_SCHEMA_VERSION,
        SchemaKind::Hir => HIR_SCHEMA_VERSION,
        SchemaKind::Mir => MIR_SCHEMA_VERSION,
        SchemaKind::Build => BUILD_SCHEMA_VERSION,
        SchemaKind::Package => PACKAGE_SCHEMA_VERSION,
        SchemaKind::MacroExpansion => MACRO_SCHEMA_VERSION,
        SchemaKind::Token => TOKEN_SCHEMA_VERSION,
    }
}
