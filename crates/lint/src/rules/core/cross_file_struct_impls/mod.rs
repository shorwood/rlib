extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::impl_target::ImplTargetExt;

/// Distinct path suffixes used to label an implementation and its struct definition.
struct DistinctiveFileLabels {
    /// Compact implementation-file suffix.
    implementation: String,
    /// Compact struct-definition-file suffix.
    definition: String,
}

impl DistinctiveFileLabels {
    /// Returns the shortest stable path suffixes that distinguish two physical files.
    fn between(first: &str, second: &str) -> Self {
        let first = first.split(['/', '\\']).collect::<Vec<_>>();
        let second = second.split(['/', '\\']).collect::<Vec<_>>();
        let maximum = first.len().min(second.len());
        for depth in 1..=maximum {
            let first_suffix = first[first.len() - depth..].join("/");
            let second_suffix = second[second.len() - depth..].join("/");
            if first_suffix != second_suffix {
                return Self {
                    implementation: first_suffix,
                    definition: second_suffix,
                };
            }
        }
        Self {
            implementation: first.join("/"),
            definition: second.join("/"),
        }
    }
}

// -----------------------------------------------------------------------------
// Violation: Cross file implementation diagnostic
// -----------------------------------------------------------------------------

/// One authored source location with its compact physical filename.
struct ViolationLocation {
    /// Source span shown in the diagnostic.
    span: Span,
    /// Compact physical filename used in labels and remediation.
    file: String,
}

/// Direct struct implementation separated from the definition it belongs to.
struct Violation {
    /// Implementation node used to anchor the lint level.
    hir_id: HirId,
    /// Authored implementation span shown as the primary location.
    implementation: ViolationLocation,
    /// Implemented struct name shown in diagnostics.
    struct_name: Symbol,
    /// Struct definition span shown as the related location.
    definition: ViolationLocation,
}

impl Violation {
    /// Classifies an authored direct implementation whose local struct is in another file.
    fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Require an authored direct implementation item.
        if !matches!(item.kind, ItemKind::Impl(_))
            || item.span.in_external_macro(cx.sess().source_map())
        {
            return None;
        }

        // Resolve an authored local struct definition from the implementation.
        let struct_def_id = item.direct_struct(cx)?;
        let struct_span = cx.tcx.def_span(struct_def_id);
        if struct_span.in_external_macro(cx.sess().source_map()) {
            return None;
        }

        // Resolve both physical locations before crossing the diagnostic boundary.
        let source_map = cx.sess().source_map();
        let implementation_file = source_map.span_to_filename(item.span);
        let struct_file = source_map.span_to_filename(struct_span);
        if implementation_file == struct_file {
            return None;
        }
        let implementation_path = implementation_file
            .prefer_remapped_unconditionally()
            .to_string_lossy();
        let struct_path = struct_file
            .prefer_remapped_unconditionally()
            .to_string_lossy();
        let labels = DistinctiveFileLabels::between(&implementation_path, &struct_path);

        // Own both physical locations before crossing the diagnostic boundary.
        let implementation = ViolationLocation {
            span: item.span,
            file: labels.implementation,
        };

        // Capture the definition independently because it is also the move target.
        let definition = ViolationLocation {
            span: struct_span,
            file: labels.definition,
        };

        // Preserve the precise source facts needed to explain and repair the violation.
        Some(Self {
            hir_id: item.hir_id(),
            struct_name: cx.tcx.item_name(struct_def_id.to_def_id()),
            implementation,
            definition,
        })
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "impl block for `{}` is not colocated with its definition",
            self.struct_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "understanding `{}` currently requires searching both `{}` and `{}` for its behavior",
            self.struct_name, self.definition.file, self.implementation.file
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "move this complete impl block into `{}` beside `{}`",
            self.definition.file, self.struct_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            CROSS_FILE_STRUCT_IMPLS,
            self.hir_id,
            self.implementation.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.implementation.span,
                    format!("this impl is defined in `{}`", self.implementation.file),
                );
                diag.span_label(
                    self.definition.span,
                    format!(
                        "`{}` is defined in `{}`",
                        self.struct_name, self.definition.file
                    ),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// CrossFileStructImpls: Type colocation policy
// -----------------------------------------------------------------------------

/// Late lint pass that keeps direct impls in their struct's physical file.
struct CrossFileStructImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub CROSS_FILE_STRUCT_IMPLS,
    Warn,
    "enforces struct definitions and their impl blocks are in the same file",
    CrossFileStructImpls
}

impl<'tcx> LateLintPass<'tcx> for CrossFileStructImpls {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let Some(violation) = Violation::from_item(cx, item) else {
            return;
        };
        violation.emit(cx);
    }
}
