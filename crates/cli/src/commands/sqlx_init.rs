//! `cargo rlib sqlx init` command.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use cargo_metadata::MetadataCommand;
use color_eyre::Section as _;
use color_eyre::eyre::WrapErr as _;

use super::error::{CommandResult, Failure};
use super::sqlfluff::{PolicyScope, SqlDialect};

/// Resolves the Cargo workspace containing the invocation.
fn workspace_root() -> CommandResult<PathBuf> {
    MetadataCommand::new()
        .no_deps()
        .exec()
        .map(|metadata| metadata.workspace_root.into_std_path_buf())
        .map_err(|error| Failure::tool(format!("could not read Cargo metadata: {error}")))
}

// -----------------------------------------------------------------------------
// Request: SQL policy initialization
// -----------------------------------------------------------------------------

/// One request to create project-owned `SQLFluff` policy.
pub struct Request {
    /// Database SQL dialect.
    dialect: SqlDialect,
    /// Optional directory receiving `.sqlfluff`.
    root: Option<PathBuf>,
}

impl Request {
    /// Creates an initialization request from validated CLI values.
    pub const fn new(dialect: SqlDialect, root: Option<PathBuf>) -> Self {
        Self { dialect, root }
    }

    /// Creates policy without overwriting an existing configuration.
    pub fn run(self) -> CommandResult {
        let root = self.root.map_or_else(workspace_root, Ok)?;
        fs::create_dir_all(&root).wrap_err_with(|| {
            format!("could not create SQL policy directory `{}`", root.display())
        })?;

        // Existing project policy always owns initialization.
        if PolicyScope::new(&root, &root).is_configured() {
            return Err(Failure::tool(format!(
                "SQLFluff configuration already exists at `{}`",
                root.display()
            ))
            .suggestion("Edit the existing project policy instead of initializing it again."));
        }
        let path = root.join(".sqlfluff");
        let contents = format!(
            "[sqlfluff]\ndialect = {}\ntemplater = placeholder\n\n[sqlfluff:templater:placeholder]\nparam_style = {}\n",
            self.dialect.as_str(),
            self.dialect.placeholder_style()
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .wrap_err_with(|| format!("could not create `{}`", path.display()))?;
        file.write_all(contents.as_bytes())
            .wrap_err_with(|| format!("could not write `{}`", path.display()))?;
        println!("created {}", path.display());
        Ok(())
    }
}
