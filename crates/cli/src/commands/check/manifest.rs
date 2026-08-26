//! Compiler query-manifest loading and trust validation.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use color_eyre::eyre::WrapErr as _;
use rlib_sqlx_model::protocol::{
    CompilationContext, QUERY_MANIFEST_VERSION, QueryDocument, QueryManifest,
};

use crate::commands::error::{CommandResult, Failure};
use crate::commands::sqlfluff::QueryOriginExt as _;

// -----------------------------------------------------------------------------
// ExtractedQuery: Trusted compiler output
// -----------------------------------------------------------------------------

/// One recovered query paired with the compilation that produced it.
pub(super) struct ExtractedQuery {
    /// Recovered SQL document and authored provenance.
    pub(super) document: QueryDocument,
    /// Cargo compilation identity used for machine diagnostics.
    pub(super) compilation: CompilationContext,
}

// -----------------------------------------------------------------------------
// Loader: Manifest trust boundary
// -----------------------------------------------------------------------------

/// Loads compiler query manifests within one canonical workspace.
pub(super) struct Loader<'a> {
    /// Directory populated by the compiler extraction pass.
    directory: &'a Path,
    /// Canonical workspace allowed to own recovered source paths.
    workspace: &'a Path,
}

impl<'a> Loader<'a> {
    /// Binds an extraction directory to its workspace trust boundary.
    pub(super) const fn new(directory: &'a Path, workspace: &'a Path) -> Self {
        Self {
            directory,
            workspace,
        }
    }

    /// Loads, validates, and deterministically merges compiler query manifests.
    pub(super) fn load(&self) -> CommandResult<Vec<ExtractedQuery>> {
        let mut queries = BTreeMap::new();
        let entries = fs::read_dir(self.directory).wrap_err_with(|| {
            format!(
                "could not read compiler output directory `{}`",
                self.directory.display()
            )
        })?;
        for entry in entries {
            let path = entry
                .wrap_err("could not inspect a compiler output entry")?
                .path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let contents =
                fs::read(&path).wrap_err_with(|| format!("could not read `{}`", path.display()))?;
            let manifest: QueryManifest = serde_json::from_slice(&contents).map_err(|error| {
                Failure::tool(format!(
                    "invalid query manifest `{}`: {error}",
                    path.display()
                ))
            })?;

            // Protocol mismatches cannot be interpreted safely across tool versions.
            if manifest.version != QUERY_MANIFEST_VERSION {
                return Err(Failure::tool(format!(
                    "unsupported query manifest version {}",
                    manifest.version
                )));
            }
            let manifest_workspace = manifest
                .workspace_root
                .canonicalize()
                .unwrap_or(manifest.workspace_root);

            // A manifest may describe only the workspace that requested extraction.
            if manifest_workspace != self.workspace {
                return Err(Failure::tool(format!(
                    "query manifest workspace `{}` does not match `{}`",
                    manifest_workspace.display(),
                    self.workspace.display()
                )));
            }
            for document in manifest.documents {
                let source = document.origin.authored_path();
                let canonical = source
                    .canonicalize()
                    .unwrap_or_else(|_| source.to_path_buf());

                // Source paths outside the workspace are not trusted diagnostic targets.
                if !canonical.starts_with(self.workspace) {
                    return Err(Failure::tool(format!(
                        "query source `{}` is outside workspace `{}`",
                        canonical.display(),
                        self.workspace.display()
                    )));
                }
                let key = format!("{}\0{}", canonical.display(), document.text);
                queries.entry(key).or_insert_with(|| ExtractedQuery {
                    document,
                    compilation: manifest.compilation.clone(),
                });
            }
        }
        Ok(queries.into_values().collect())
    }
}
