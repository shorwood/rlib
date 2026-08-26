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

use super::utils::{operation, root_local, sqlx_macro, static_string};

const MANIFEST_ENV: &str = "RLIB_SQLX_MANIFEST_DIR";
const WORKSPACE_ENV: &str = "RLIB_SQLX_WORKSPACE_ROOT";

struct MacroArguments {
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

/// Compiler-backed query collection enabled only by the companion command.
struct QueryExtractor {
    output: PathBuf,
    workspace: PathBuf,
    package_manifest: PathBuf,
    documents: BTreeMap<String, QueryDocument>,
    builders: HashMap<HirId, BuilderDocument>,
}

struct BuilderDocument {
    text: String,
    rust_path: PathBuf,
    literal_span: ByteRange,
    source_segments: Vec<SourceSegment>,
}

impl QueryExtractor {
    fn from_environment() -> Option<Self> {
        Some(Self {
            output: PathBuf::from(env::var_os(MANIFEST_ENV)?),
            workspace: PathBuf::from(env::var_os(WORKSPACE_ENV)?),
            package_manifest: PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?).join("Cargo.toml"),
            documents: BTreeMap::new(),
            builders: HashMap::new(),
        })
    }

    fn source_location(cx: &LateContext<'_>, span: Span) -> Option<(PathBuf, ByteRange)> {
        let source_map = cx.sess().source_map();
        let path = source_map.span_to_filename(span).into_local_path()?;
        let file = source_map.lookup_source_file(span.lo());
        let start = (span.lo() - file.start_pos).to_u32();
        let end = (span.hi() - file.start_pos).to_u32();
        Some((
            path.canonicalize().unwrap_or(path),
            ByteRange { start, end },
        ))
    }

    fn inline_origin(cx: &LateContext<'_>, span: Span, sql: &str) -> Option<QueryOrigin> {
        let (rust_path, literal_span) = Self::source_location(cx, span)?;
        let snippet = cx.sess().source_map().span_to_snippet(span).ok();
        let source_segments = snippet
            .as_deref()
            .and_then(|snippet| snippet.find(sql))
            .and_then(|relative| {
                let relative = u32::try_from(relative).ok()?;
                let length = u32::try_from(sql.len()).ok()?;
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

    fn macro_literal(tokens: TokenStream, name: &str) -> Option<(LitStr, bool)> {
        let arguments = syn::parse2::<MacroArguments>(tokens).ok()?.values;
        let query_index = usize::from(name.contains("_as"));
        let SynExpr::Lit(literal) = arguments.get(query_index)? else {
            return None;
        };
        let syn::Lit::Str(value) = &literal.lit else {
            return None;
        };
        Some((value.clone(), name.contains("_file")))
    }

    fn observe_macro(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let Some((name, call_site)) = sqlx_macro(cx, expression.span) else {
            return;
        };
        if !name.starts_with("query") {
            return;
        }
        let Ok(snippet) = cx.sess().source_map().span_to_snippet(call_site) else {
            return;
        };
        let Ok(parsed) = syn::parse_str::<syn::ExprMacro>(&snippet) else {
            return;
        };
        let Some((literal, is_file)) = Self::macro_literal(parsed.mac.tokens, &name) else {
            return;
        };
        let api_kind = if name.contains("unchecked") {
            QueryApiKind::UncheckedMacro
        } else {
            QueryApiKind::CheckedMacro
        };
        if is_file {
            let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
            let path = manifest.join(literal.value());
            let Ok(path) = path.canonicalize() else {
                return;
            };
            let Ok(text) = fs::read_to_string(&path) else {
                return;
            };
            self.insert(QueryDocument {
                api_kind,
                text,
                origin: QueryOrigin::File { sql_path: path },
            });
        } else {
            let text = literal.value();
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

    fn observe_runtime_query(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        if expression.span.from_expansion() {
            return;
        }
        let Some(call) = operation(cx, expression) else {
            return;
        };
        let api_kind = match call.name.as_str() {
            "query" | "query_as" | "query_as_with" | "query_scalar" | "query_scalar_with"
            | "query_with" => QueryApiKind::RuntimeQuery,
            "raw_sql" => QueryApiKind::RawSql,
            _ => return,
        };
        let Some(argument) = call.arguments.first() else {
            return;
        };
        let Some(text) = static_string(argument) else {
            return;
        };
        let Some(origin) = Self::inline_origin(cx, argument.span, &text) else {
            return;
        };
        self.insert(QueryDocument {
            api_kind,
            text,
            origin,
        });
    }

    fn static_fragment(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
    ) -> Option<(String, PathBuf, ByteRange, Vec<SourceSegment>)> {
        let text = static_string(expression)?;
        let QueryOrigin::Inline {
            rust_path,
            literal_span,
            source_segments,
        } = Self::inline_origin(cx, expression.span, &text)?
        else {
            return None;
        };
        Some((text, rust_path, literal_span, source_segments))
    }

    fn observe_builder_binding(&mut self, cx: &LateContext<'_>, statement: &Stmt<'_>) {
        let StmtKind::Let(local) = statement.kind else {
            return;
        };
        let PatKind::Binding(_, binding, _, None) = local.pat.kind else {
            return;
        };
        let Some(initializer) = local.init else {
            return;
        };
        let Some(call) = operation(cx, initializer) else {
            return;
        };
        if call.name != "new" {
            return;
        }
        let Some((text, rust_path, literal_span, source_segments)) = call
            .arguments
            .first()
            .and_then(|argument| Self::static_fragment(cx, argument))
        else {
            return;
        };
        self.builders.insert(
            binding,
            BuilderDocument {
                text,
                rust_path,
                literal_span,
                source_segments,
            },
        );
    }

    fn observe_builder_operation(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let Some(call) = operation(cx, expression) else {
            return;
        };
        let Some(binding) = call.receiver.and_then(root_local) else {
            return;
        };
        if matches!(
            call.name.as_str(),
            "build" | "build_query_as" | "build_query_scalar"
        ) {
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
        if matches!(
            call.name.as_str(),
            "push_bind" | "push_values" | "push_tuples" | "separated" | "reset"
        ) {
            self.builders.remove(&binding);
            return;
        }
        if !matches!(call.name.as_str(), "push" | "push_unseparated") {
            return;
        }
        let Some(fragment) = call
            .arguments
            .first()
            .and_then(|argument| Self::static_fragment(cx, argument))
        else {
            self.builders.remove(&binding);
            return;
        };
        let Some(builder) = self.builders.get_mut(&binding) else {
            return;
        };
        let (text, rust_path, _, mut source_segments) = fragment;
        if rust_path != builder.rust_path {
            self.builders.remove(&binding);
            return;
        }
        let Ok(sql_offset) = u32::try_from(builder.text.len()) else {
            self.builders.remove(&binding);
            return;
        };
        for segment in &mut source_segments {
            let Some(start) = segment.sql.start.checked_add(sql_offset) else {
                self.builders.remove(&binding);
                return;
            };
            let Some(end) = segment.sql.end.checked_add(sql_offset) else {
                self.builders.remove(&binding);
                return;
            };
            segment.sql = ByteRange { start, end };
        }
        builder.text.push_str(&text);
        builder.source_segments.extend(source_segments);
    }

    fn write_manifest(&self, cx: &LateContext<'_>) {
        if self.documents.is_empty() {
            return;
        }
        let crate_name = cx.tcx.crate_name(LOCAL_CRATE);
        let crate_id = crate_name.to_string();
        let manifest = QueryManifest {
            version: QUERY_MANIFEST_VERSION,
            crate_id: crate_id.clone(),
            compilation: CompilationContext {
                package_manifest: self
                    .package_manifest
                    .canonicalize()
                    .unwrap_or_else(|_| self.package_manifest.clone()),
                crate_name: crate_id.clone(),
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
        };
        let Ok(contents) = serde_json::to_vec_pretty(&manifest) else {
            return;
        };
        let process = process_id();
        let final_path = self.output.join(format!("{crate_id}-{process}.json"));
        let temporary_path = self.output.join(format!(".{crate_id}-{process}.tmp"));
        if fs::write(&temporary_path, contents).is_ok() {
            let _result = fs::rename(temporary_path, final_path);
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

/// Registers the query extractor only when the private runner supplies its environment.
pub fn register(lint_store: &mut LintStore) {
    if QueryExtractor::from_environment().is_some() {
        lint_store.register_late_pass(dylint_linting::__make_late_closure!(
            QueryExtractor::from_environment().expect("extractor environment remains set")
        ));
    }
}
