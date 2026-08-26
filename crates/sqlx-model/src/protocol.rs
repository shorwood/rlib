//! Versioned query-manifest protocol.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// -----------------------------------------------------------------------------
// QueryManifestVersion: Manifest compatibility
// -----------------------------------------------------------------------------

/// Current query-manifest protocol version.
pub const QUERY_MANIFEST_VERSION: u32 = 2;

// -----------------------------------------------------------------------------
// CompilationContext: Cargo compilation identity
// -----------------------------------------------------------------------------

/// Cargo identity for the compiler invocation that recovered a query.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CompilationContext {
    /// Canonical package manifest owning the compiler invocation.
    pub package_manifest: PathBuf,
    /// rustc crate name used to resolve the Cargo target.
    pub crate_name: String,
    /// Whether compiler lint policy elevates warnings for this invocation.
    pub has_denied_warnings: bool,
}

// -----------------------------------------------------------------------------
// ByteRange: Source coordinates
// -----------------------------------------------------------------------------

/// A half-open byte range.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ByteRange {
    /// Inclusive byte offset.
    pub start: u32,
    /// Exclusive byte offset.
    pub end: u32,
}

// -----------------------------------------------------------------------------
// SourceSegment: Decoded source mapping
// -----------------------------------------------------------------------------

/// Maps decoded SQL bytes to bytes in an authored Rust source file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceSegment {
    /// Byte range in decoded SQL.
    pub sql: ByteRange,
    /// Corresponding byte range in the Rust file.
    pub rust: ByteRange,
}

// -----------------------------------------------------------------------------
// QueryApiKind: SQL ownership
// -----------------------------------------------------------------------------

/// `SQLx` interface from which a query was recovered.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryApiKind {
    /// A compile-time checked macro.
    CheckedMacro,
    /// A compile-time macro that skips Rust type checking.
    UncheckedMacro,
    /// A runtime prepared-query function.
    RuntimeQuery,
    /// The unprepared raw SQL function.
    RawSql,
    /// A fully reconstructable query builder.
    QueryBuilder,
}

// -----------------------------------------------------------------------------
// QueryOrigin: Authored query provenance
// -----------------------------------------------------------------------------

/// Authored location and representation of a complete SQL document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum QueryOrigin {
    /// SQL decoded from one or more Rust string literals.
    #[serde(rename = "inline")]
    Inline {
        /// Canonical Rust source path.
        #[serde(rename = "rust_path")]
        rust_path: PathBuf,
        /// Complete source range used as a fallback diagnostic location.
        #[serde(rename = "literal_span")]
        literal_span: ByteRange,
        /// Decoded-to-authored byte mappings.
        #[serde(rename = "source_segments")]
        source_segments: Vec<SourceSegment>,
    },
    /// SQL loaded from an authored file.
    #[serde(rename = "file")]
    File {
        /// Canonical SQL source path.
        #[serde(rename = "sql_path")]
        sql_path: PathBuf,
    },
}

// -----------------------------------------------------------------------------
// QueryDocument: Extracted SQL unit
// -----------------------------------------------------------------------------

/// One complete query suitable for `SQLFluff`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct QueryDocument {
    /// `SQLx` interface which owns the query.
    pub api_kind: QueryApiKind,
    /// Complete decoded SQL text.
    pub text: String,
    /// Authored query location.
    pub origin: QueryOrigin,
}

// -----------------------------------------------------------------------------
// QueryManifest: Compiler invocation payload
// -----------------------------------------------------------------------------

/// Queries extracted from one compiler invocation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct QueryManifest {
    /// Wire-format version.
    pub version: u32,
    /// Stable compiler crate identity used for deterministic merging.
    pub crate_id: String,
    /// Cargo package and target identity for diagnostics.
    pub compilation: CompilationContext,
    /// Canonical workspace root.
    pub workspace_root: PathBuf,
    /// Complete recovered query documents.
    pub documents: Vec<QueryDocument>,
}
