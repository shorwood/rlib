//! Cargo frontend for the complete rlib analysis suite.

mod sqlx;

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

// -----------------------------------------------------------------------------
// SqlDialect: Initialization grammar
// -----------------------------------------------------------------------------

/// SQL dialects supported by the initializer.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum SqlDialect {
    /// `PostgreSQL` grammar and placeholders.
    Postgres,
    /// `MySQL` grammar and placeholders.
    Mysql,
    /// `SQLite` grammar and placeholders.
    Sqlite,
}

impl SqlDialect {
    /// Returns the `SQLFluff` dialect name.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
            Self::Sqlite => "sqlite",
        }
    }
}

// -----------------------------------------------------------------------------
// SqlxCommand: Integration grammar
// -----------------------------------------------------------------------------

/// `SQLx` integration commands.
#[derive(Debug, Subcommand)]
enum SqlxCommand {
    /// Create initial `SQLFluff` policy for a project.
    Init {
        /// SQL dialect used by the project.
        #[arg(long, value_enum)]
        dialect: SqlDialect,

        /// Directory that receives `.sqlfluff`.
        #[arg(long)]
        path: Option<PathBuf>,
    },
}

// -----------------------------------------------------------------------------
// Command: Support grammar
// -----------------------------------------------------------------------------

/// Public and private support commands.
#[derive(Debug, Subcommand)]
enum Command {
    /// Manage `SQLx` lint integration.
    Sqlx {
        /// `SQLx` integration command.
        #[command(subcommand)]
        command: SqlxCommand,
    },

    /// Run the bundled compiler driver in a child process.
    #[command(name = "__run-dylint", hide = true)]
    RunDylint {
        /// Apply compiler suggestions during this private pass.
        #[arg(long = "fix")]
        should_fix: bool,
    },
}

// -----------------------------------------------------------------------------
// CommandArguments: Cargo protocol adapter
// -----------------------------------------------------------------------------

/// Arguments separated across Clap and rustc protocol boundaries.
struct CommandArguments {
    /// Program name and arguments interpreted by Clap.
    clap: Vec<String>,
    /// Arguments following Cargo's explicit rustc delimiter.
    rustc: Vec<String>,
}

impl CommandArguments {
    /// Reads and normalizes Cargo's external-subcommand invocation.
    fn from_environment() -> Self {
        env::args().skip(1).collect::<Vec<_>>().into()
    }

    /// Removes and returns arguments following Cargo's explicit rustc delimiter.
    fn split_rustc(arguments: &mut Vec<String>) -> Vec<String> {
        // Without the delimiter, every forwarded argument belongs to Cargo.
        let Some(delimiter) = arguments.iter().position(|argument| argument == "--") else {
            return Vec::new();
        };
        let rustc = arguments.split_off(delimiter + 1);
        arguments.pop();
        rustc
    }
}

impl From<Vec<String>> for CommandArguments {
    /// Normalizes Cargo's injected subcommand name and preserves the rustc tail.
    fn from(mut arguments: Vec<String>) -> Self {
        if arguments.first().is_some_and(|argument| argument == "rlib") {
            arguments.remove(0);
        }
        let rustc = Self::split_rustc(&mut arguments);
        arguments.insert(0, "cargo rlib".to_owned());
        Self {
            clap: arguments,
            rustc,
        }
    }
}

// -----------------------------------------------------------------------------
// Cli: Public command grammar
// -----------------------------------------------------------------------------

/// Compiler-backed architectural lints for Rust projects.
#[derive(Debug, Parser)]
#[command(
    name = "cargo-rlib",
    bin_name = "cargo rlib",
    version,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    after_help = "Arguments after `--` are forwarded unchanged to rustc."
)]
struct Cli {
    /// Apply machine-applicable native suggestions before checking.
    #[arg(long = "fix")]
    should_fix: bool,

    /// An rlib support command.
    #[command(subcommand)]
    command: Option<Command>,

    /// Arguments forwarded unchanged to Cargo.
    #[arg(allow_hyphen_values = true)]
    cargo_arguments: Vec<String>,
}

impl Cli {
    /// Dispatches one parsed rlib operation.
    fn run(self, rustc_arguments: Vec<String>) -> Result<(), sqlx::CommandError> {
        match self.command {
            Some(Command::Sqlx {
                command: SqlxCommand::Init { dialect, path },
            }) => sqlx::SqlxInit::new(dialect.as_str(), path).run(),
            Some(Command::RunDylint { should_fix }) => {
                sqlx::run_internal_dylint(sqlx::FixMode::for_request(should_fix.into()))
            }
            None => sqlx::RlibCommand::new(
                self.cargo_arguments,
                rustc_arguments,
                sqlx::FixMode::for_request(self.should_fix.into()),
            )
            .check(),
        }
    }
}

// -----------------------------------------------------------------------------
// Main: Process boundary
// -----------------------------------------------------------------------------

/// Parses the command, renders failures, and returns a stable process status.
fn main() -> ExitCode {
    let arguments = CommandArguments::from_environment();
    match Cli::try_parse_from(arguments.clap) {
        Ok(cli) => match cli.run(arguments.rustc) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("rlib: {error}");
                ExitCode::from(error.exit_code())
            }
        },
        Err(error) => {
            let status = if error.use_stderr() { 2 } else { 0 };
            if let Err(print_error) = error.print() {
                eprintln!("cargo rlib: could not render command output: {print_error}");
            }
            ExitCode::from(status)
        }
    }
}

// -----------------------------------------------------------------------------
// Tests: Command grammar contracts
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, CommandArguments, SqlDialect, SqlxCommand};

    #[test]
    fn separates_rlib_cargo_and_rustc_arguments() {
        let arguments = CommandArguments::from(
            [
                "--fix",
                "--workspace",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ]
            .map(str::to_owned)
            .to_vec(),
        );
        let cli =
            Cli::try_parse_from(arguments.clap).expect("public check invocation should parse");
        assert!(cli.should_fix);
        assert_eq!(cli.cargo_arguments, ["--workspace", "--all-features"]);
        assert_eq!(arguments.rustc, ["-D", "warnings"]);
    }

    #[test]
    fn preserves_rlib_like_options_after_the_rustc_delimiter() {
        let arguments = CommandArguments::from(vec!["--".to_owned(), "--fix".to_owned()]);
        let cli = Cli::try_parse_from(arguments.clap).expect("empty check invocation should parse");
        assert!(!cli.should_fix);
        assert!(cli.cargo_arguments.is_empty());
        assert_eq!(arguments.rustc, ["--fix"]);
    }

    #[test]
    fn parses_typed_sqlx_initialization() {
        let cli = Cli::try_parse_from([
            "cargo rlib",
            "sqlx",
            "init",
            "--dialect",
            "postgres",
            "--path",
            "database",
        ])
        .expect("SQLx initialization should parse");
        assert!(matches!(
            cli.command,
            Some(Command::Sqlx {
                command: SqlxCommand::Init {
                    dialect: SqlDialect::Postgres,
                    path: Some(_),
                }
            })
        ));
    }

    #[test]
    fn rejects_removed_sqlx_check_command() {
        assert!(Cli::try_parse_from(["cargo rlib", "sqlx", "check"]).is_err());
    }

    #[test]
    fn does_not_treat_a_cargo_value_as_a_subcommand() {
        let cli = Cli::try_parse_from(["cargo rlib", "--package", "sqlx"])
            .expect("Cargo package selection should remain opaque");
        assert!(cli.command.is_none());
        assert_eq!(cli.cargo_arguments, ["--package", "sqlx"]);
    }
}
