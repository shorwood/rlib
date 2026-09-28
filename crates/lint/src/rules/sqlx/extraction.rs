extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::process::id as process_id;
use std::{env, fs};

use proc_macro2::TokenStream;
use rlib_sqlx_model::protocol::{
    ByteRange, CompilationContext, QUERY_MANIFEST_VERSION, QueryApiKind, QueryDocument,
    QueryManifest, QueryOrigin, SourceSegment,
};
use rustc_hir::{Expr, HirId, PatKind, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext, LintStore};
use rustc_session::lint::Level;
use rustc_span::def_id::LOCAL_CRATE;
use rustc_span::{Pos, Span};
use syn::parse::{Parse, ParseStream};
use syn::{Expr as SynExpr, LitStr, Token};

use super::utils::{SqlxExprExt as _, SqlxMacroSpanExt as _};

// -----------------------------------------------------------------------------
// MacroArguments: Parsed SQLx macro inputs
// -----------------------------------------------------------------------------

/// Comma-separated expressions accepted by the `SQLx` query macro family.
struct MacroArguments {
    /// Parsed arguments in authored order.
    values: Vec<SynExpr>,
}

impl Parse for MacroArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut values = Vec::new();
        while !input.is_empty() {
            values.push(input.parse()?);
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }
        Ok(Self { values })
    }
}

// -----------------------------------------------------------------------------
// SourceLocation: Authored span coordinates
// -----------------------------------------------------------------------------

/// Local Rust source path and byte range for an authored span.
struct SourceLocation {
    /// Canonical local Rust source path when canonicalization succeeds.
    rust_path: PathBuf,
    /// Byte range relative to the containing Rust source file.
    literal_span: ByteRange,
}

// -----------------------------------------------------------------------------
// MacroLiteral: Query macro source argument
// -----------------------------------------------------------------------------

/// Static query argument extracted from one `SQLx` macro invocation.
struct MacroLiteral {
    /// Authored string literal.
    value: LitStr,
    /// Whether the literal identifies an external SQL file.
    is_file: bool,
}

// -----------------------------------------------------------------------------
// BuilderDocument: Incrementally assembled query source
// -----------------------------------------------------------------------------

/// Static `QueryBuilder` content and its Rust-to-SQL source mapping.
struct BuilderDocument {
    /// SQL text assembled so far.
    text: String,
    /// Rust source file containing every retained fragment.
    rust_path: PathBuf,
    /// Span of the builder's initial static literal.
    literal_span: ByteRange,
    /// SQL-to-Rust source mappings for static fragments.
    source_segments: Vec<SourceSegment>,
}

// -----------------------------------------------------------------------------
// QueryExtractor: Compiler-backed query collection
// -----------------------------------------------------------------------------

/// Compiler-backed query collection enabled only by the companion command.
struct QueryExtractor {
    /// Directory receiving this crate's query manifest.
    output: PathBuf,
    /// Canonical workspace source boundary.
    workspace: PathBuf,
    /// Cargo manifest for the package currently being compiled.
    package_manifest: PathBuf,
    /// Deduplicated documents keyed by origin and SQL text.
    documents: BTreeMap<String, QueryDocument>,
    /// Static `QueryBuilder` state indexed by local binding.
    builders: HashMap<HirId, BuilderDocument>,
}

impl QueryExtractor {
    /// Environment variable locating the directory for extracted query manifests.
    const MANIFEST_ENV: &'static str = "RLIB_SQLX_MANIFEST_DIR";

    /// Environment variable carrying the canonical workspace source boundary.
    const WORKSPACE_ENV: &'static str = "RLIB_SQLX_WORKSPACE_ROOT";

    /// Constructs an extractor only for invocations configured by the companion command.
    fn from_environment() -> Option<Self> {
        Some(Self {
            output: PathBuf::from(env::var_os(Self::MANIFEST_ENV)?),
            workspace: PathBuf::from(env::var_os(Self::WORKSPACE_ENV)?),
            package_manifest: PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?).join("Cargo.toml"),
            documents: BTreeMap::new(),
            builders: HashMap::new(),
        })
    }

    /// Resolves an authored span to a local path and file-relative byte range.
    fn source_location(cx: &LateContext<'_>, span: Span) -> Option<SourceLocation> {
        let source_map = cx.sess().source_map();
        let path = source_map.span_to_filename(span).into_local_path()?;
        let file = source_map.lookup_source_file(span.lo());
        let start = (span.lo() - file.start_pos).to_u32();
        let end = (span.hi() - file.start_pos).to_u32();
        Some(SourceLocation {
            rust_path: path.canonicalize().unwrap_or(path),
            literal_span: ByteRange { start, end },
        })
    }

    /// Creates an inline query origin with a source mapping when the SQL is directly authored.
    fn inline_origin(cx: &LateContext<'_>, span: Span, sql: &str) -> Option<QueryOrigin> {
        let SourceLocation {
            rust_path,
            literal_span,
        } = Self::source_location(cx, span)?;

        // Missing source text preserves origin coordinates without segment precision.
        let Ok(snippet) = cx.sess().source_map().span_to_snippet(span) else {
            return Some(QueryOrigin::Inline {
                rust_path,
                literal_span,
                source_segments: Vec::new(),
            });
        };
        let source_segments = snippet
            .find(sql)
            .and_then(|relative| {
                // Source coordinates outside the manifest range cannot be represented.
                let Ok(relative) = u32::try_from(relative) else {
                    return None;
                };
                // Query lengths outside the manifest range cannot be represented.
                let Ok(length) = u32::try_from(sql.len()) else {
                    return None;
                };
                Some(SourceSegment {
                    sql: ByteRange {
                        start: 0,
                        end: length,
                    },
                    rust: ByteRange {
                        start: literal_span.start.checked_add(relative)?,
                        end: literal_span
                            .start
                            .checked_add(relative)?
                            .checked_add(length)?,
                    },
                })
            })
            .into_iter()
            .collect();
        Some(QueryOrigin::Inline {
            rust_path,
            literal_span,
            source_segments,
        })
    }

    /// Extracts the query literal and file mode from parsed `SQLx` macro tokens.
    fn macro_literal(tokens: TokenStream, name: &str) -> Option<MacroLiteral> {
        // Invalid macro syntax is owned by rustc and SQLx.
        let Ok(parsed) = syn::parse2::<MacroArguments>(tokens) else {
            return None;
        };
        let arguments = parsed.values;
        let query_index = usize::from(name.contains("_as"));

        // SQLx macro families place the query after the optional output type.
        let SynExpr::Lit(literal) = arguments.get(query_index)? else {
            return None;
        };

        // Dynamic macro arguments cannot produce a static query document.
        let syn::Lit::Str(value) = &literal.lit else {
            return None;
        };
        Some(MacroLiteral {
            value: value.clone(),
            is_file: name.contains("_file"),
        })
    }

    /// Converts a static Rust expression into an initial or appended builder fragment.
    fn static_fragment(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<BuilderDocument> {
        let text = expression.static_string()?;

        // Builder fragments are accepted only from directly authored inline strings.
        let QueryOrigin::Inline {
            rust_path,
            literal_span,
            source_segments,
        } = Self::inline_origin(cx, expression.span, &text)?
        else {
            return None;
        };
        Some(BuilderDocument {
            text,
            rust_path,
            literal_span,
            source_segments,
        })
    }

    /// Inserts one query document while deduplicating repeated expansion visits.
    fn insert(&mut self, document: QueryDocument) {
        let path = match &document.origin {
            QueryOrigin::Inline {
                rust_path,
                literal_span,
                ..
            } => {
                format!("{}:{}", rust_path.display(), literal_span.start)
            }
            QueryOrigin::File { sql_path } => sql_path.display().to_string(),
        };
        self.documents
            .entry(format!("{path}\0{}", document.text))
            .or_insert(document);
    }

    /// Observes one checked or unchecked `SQLx` query macro expansion.
    fn observe_macro(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Expressions outside SQLx macro expansions carry no macro query document.
        let Some(macro_call) = expression.span.sqlx_macro(cx) else {
            return;
        };
        let name = macro_call.name;
        let call_site = macro_call.call_site;

        // Non-query SQLx macros are outside SQL extraction.
        if !name.starts_with("query") {
            return;
        }

        // Unavailable authored source cannot be parsed as a macro invocation.
        let Ok(snippet) = cx.sess().source_map().span_to_snippet(call_site) else {
            return;
        };

        // Invalid macro syntax is reported by rustc or SQLx itself.
        let Ok(parsed) = syn::parse_str::<syn::ExprMacro>(&snippet) else {
            return;
        };

        // Dynamic query arguments cannot produce a static SQL document.
        let Some(literal) = Self::macro_literal(parsed.mac.tokens, &name) else {
            return;
        };
        let api_kind = if name.contains("unchecked") {
            QueryApiKind::UncheckedMacro
        } else {
            QueryApiKind::CheckedMacro
        };
        if literal.is_file {
            let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
            let path = manifest.join(literal.value.value());

            // Missing query files are reported by SQLx and cannot be linted as documents.
            let Ok(path) = path.canonicalize() else {
                return;
            };

            // Unreadable query files provide no SQL text for downstream linting.
            let Ok(text) = fs::read_to_string(&path) else {
                return;
            };
            self.insert(QueryDocument {
                api_kind,
                text,
                origin: QueryOrigin::File { sql_path: path },
            });
        } else {
            let text = literal.value.value();

            // Unmappable expansion spans cannot support source-aware diagnostics.
            let Some(origin) = Self::inline_origin(cx, call_site, &text) else {
                return;
            };
            self.insert(QueryDocument {
                api_kind,
                text,
                origin,
            });
        }
    }

    /// Observes one runtime query API supplied with statically recoverable SQL.
    fn observe_runtime_query(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Macro expansions are collected through their public macro call site.
        if expression.span.from_expansion() {
            return;
        }

        // Non-SQLx expressions carry no SQLx query document.
        let Some(call) = expression.sqlx_operation(cx) else {
            return;
        };
        let api_kind = match call.name.as_str() {
            "query" | "query_as" | "query_as_with" | "query_scalar" | "query_scalar_with"
            | "query_with" => QueryApiKind::RuntimeQuery,
            "raw_sql" => QueryApiKind::RawSql,
            // Other SQLx APIs do not introduce standalone query documents.
            _ => return,
        };

        // Runtime query APIs require their SQL argument in the first position.
        let Some(argument) = call.arguments.first() else {
            return;
        };

        // Dynamic SQL cannot be represented as one complete document.
        let Some(text) = argument.static_string() else {
            return;
        };

        // Unmappable source spans cannot support source-aware diagnostics.
        let Some(origin) = Self::inline_origin(cx, argument.span, &text) else {
            return;
        };
        self.insert(QueryDocument {
            api_kind,
            text,
            origin,
        });
    }

    /// Starts tracking a `QueryBuilder` initialized from a static string.
    fn observe_builder_binding(&mut self, cx: &LateContext<'_>, statement: &Stmt<'_>) {
        // Only local declarations can introduce a trackable builder binding.
        let StmtKind::Let(local) = statement.kind else {
            return;
        };

        // Destructuring patterns do not provide one stable builder identity.
        let PatKind::Binding(_, binding, _, None) = local.pat.kind else {
            return;
        };

        // Declarations without an initializer cannot construct a builder.
        let Some(initializer) = local.init else {
            return;
        };

        // Non-SQLx initializers cannot construct SQLx QueryBuilder state.
        let Some(call) = initializer.sqlx_operation(cx) else {
            return;
        };

        // Only QueryBuilder construction starts a new static query document.
        if call.name != "new" {
            return;
        }

        // Dynamic or missing initial SQL cannot be represented as a static document.
        let Some(document) = call
            .arguments
            .first()
            .and_then(|argument| Self::static_fragment(cx, argument))
        else {
            return;
        };
        self.builders.insert(binding, document);
    }

    /// Updates or finalizes tracked `QueryBuilder` state for one `SQLx` operation.
    fn observe_builder_operation(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Non-SQLx expressions cannot mutate QueryBuilder state.
        let Some(call) = expression.sqlx_operation(cx) else {
            return;
        };

        // Temporary receivers have no stable builder identity across expressions.
        let Some(binding) = call
            .receiver
            .and_then(super::utils::SqlxExprExt::root_local)
        else {
            return;
        };

        // Build operations publish the complete tracked document.
        if matches!(
            call.name.as_str(),
            "build" | "build_query_as" | "build_query_scalar"
        ) {
            // Builds on untracked or invalidated builders carry no complete static document.
            let Some(builder) = self.builders.get(&binding) else {
                return;
            };
            self.insert(QueryDocument {
                api_kind: QueryApiKind::QueryBuilder,
                text: builder.text.clone(),
                origin: QueryOrigin::Inline {
                    rust_path: builder.rust_path.clone(),
                    literal_span: builder.literal_span.clone(),
                    source_segments: builder.source_segments.clone(),
                },
            });
            return;
        }

        // Shape-changing operations invalidate the accumulated static document.
        if matches!(
            call.name.as_str(),
            "push_bind" | "push_values" | "push_tuples" | "separated" | "reset"
        ) {
            self.builders.remove(&binding);
            return;
        }

        // APIs other than static fragment pushes do not alter tracked text.
        if !matches!(call.name.as_str(), "push" | "push_unseparated") {
            return;
        }

        // A dynamic fragment invalidates the builder's static document model.
        let Some(fragment) = call
            .arguments
            .first()
            .and_then(|argument| Self::static_fragment(cx, argument))
        else {
            self.builders.remove(&binding);
            return;
        };

        // Operations on builders that were never tracked require no state update.
        let Some(builder) = self.builders.get_mut(&binding) else {
            return;
        };

        // Fragments from another source file cannot share one inline origin.
        if fragment.rust_path != builder.rust_path {
            self.builders.remove(&binding);
            return;
        }

        // SQL segment offsets use the manifest protocol's 32-bit coordinate space.
        let Ok(sql_offset) = u32::try_from(builder.text.len()) else {
            self.builders.remove(&binding);
            return;
        };
        let mut source_segments = fragment.source_segments;
        for segment in &mut source_segments {
            // Overflow makes this segment impossible to represent in the manifest protocol.
            let Some(start) = segment.sql.start.checked_add(sql_offset) else {
                self.builders.remove(&binding);
                return;
            };

            // Both ends must remain representable before retaining builder state.
            let Some(end) = segment.sql.end.checked_add(sql_offset) else {
                self.builders.remove(&binding);
                return;
            };
            segment.sql = ByteRange { start, end };
        }
        builder.text.push_str(&fragment.text);
        builder.source_segments.extend(source_segments);
    }

    /// Builds the protocol manifest for this crate's accumulated documents.
    fn query_manifest(&self, cx: &LateContext<'_>, crate_id: &str) -> QueryManifest {
        QueryManifest {
            version: QUERY_MANIFEST_VERSION,
            crate_id: crate_id.to_owned(),
            compilation: CompilationContext {
                package_manifest: self
                    .package_manifest
                    .canonicalize()
                    .unwrap_or_else(|_| self.package_manifest.clone()),
                crate_name: crate_id.to_owned(),
                has_denied_warnings: cx
                    .tcx
                    .sess
                    .opts
                    .lint_opts
                    .iter()
                    .rev()
                    .find(|(name, _)| name == "warnings")
                    .is_some_and(|(_, level)| matches!(level, Level::Deny | Level::Forbid)),
            },
            workspace_root: self.workspace.clone(),
            documents: self.documents.values().cloned().collect(),
        }
    }

    /// Writes this crate's collected queries through an atomic temporary file.
    fn write_manifest(&self, cx: &LateContext<'_>) {
        // Crates without query documents need no manifest artifact.
        if self.documents.is_empty() {
            return;
        }
        let crate_name = cx.tcx.crate_name(LOCAL_CRATE);
        let crate_id = crate_name.to_string();
        let manifest = self.query_manifest(cx, &crate_id);

        // Serialization failures leave no valid manifest to publish.
        let Ok(contents) = serde_json::to_vec_pretty(&manifest) else {
            return;
        };
        let process = process_id();
        let final_path = self.output.join(format!("{crate_id}-{process}.json"));
        let temporary_path = self.output.join(format!(".{crate_id}-{process}.tmp"));

        // Failed temporary writes must not replace an earlier complete manifest.
        if fs::write(&temporary_path, contents).is_err() {
            return;
        }
        match fs::rename(&temporary_path, final_path) {
            Ok(()) => {}
            Err(_rename_error) => {
                // The companion command treats a missing final manifest as extraction failure.
            }
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for QueryExtractor {
    fn check_stmt(&mut self, cx: &LateContext<'tcx>, statement: &'tcx Stmt<'tcx>) {
        self.observe_builder_binding(cx, statement);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.observe_macro(cx, expression);
        self.observe_runtime_query(cx, expression);
        self.observe_builder_operation(cx, expression);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        self.write_manifest(cx);
    }
}

rustc_session::impl_lint_pass!(QueryExtractor => []);

// -----------------------------------------------------------------------------
// SqlxExtractionLintStoreExt: Conditional extractor registration
// -----------------------------------------------------------------------------

/// Query-extraction registration behavior for the compiler lint store.
pub trait SqlxExtractionLintStoreExt {
    /// Registers extraction only when the private runner supplies its environment.
    fn register_sqlx_extraction(&mut self);
}

impl SqlxExtractionLintStoreExt for LintStore {
    fn register_sqlx_extraction(&mut self) {
        // Ordinary compiler invocations do not opt into extraction.
        let Some(_) = QueryExtractor::from_environment() else {
            return;
        };
        self.register_late_pass(dylint_linting::__make_late_closure!(
            QueryExtractor::from_environment().expect("extractor environment remains set")
        ));
    }
}
