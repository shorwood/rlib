extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, ImplItemId, Item, ItemKind, TraitItemId};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// AssociatedItemSource: Authored associated item boundary
// -----------------------------------------------------------------------------

/// Source identity needed to compare one associated item with its successor.
struct AssociatedItemSource {
    /// HIR node used to respect lint levels on the following item.
    hir_id: HirId,
    /// Declaration span used to label the item in the diagnostic.
    span: Span,
    /// Human-readable associated-item name used in remediation guidance.
    name: String,
}

impl AssociatedItemSource {
    /// Captures the source identity shared by impl and trait associated items.
    const fn new(hir_id: HirId, span: Span, name: String) -> Self {
        Self { hir_id, span, name }
    }
}

// -----------------------------------------------------------------------------
// Violation: Unseparated associated item diagnostic
// -----------------------------------------------------------------------------

/// Adjacent associated items lacking a visually empty line between them.
struct Violation {
    /// Following item used to respect its local lint level.
    hir_id: HirId,
    /// Following declaration highlighted as the unreadable boundary.
    span: Span,
    /// Name of the item immediately before the missing boundary.
    previous_name: String,
    /// Name of the item immediately after the missing boundary.
    following_name: String,
    /// Zero-width insertion point when leading context can retain its ownership.
    insertion: Option<Span>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "associated items `{}` and `{}` are not separated by a blank line",
            self.previous_name, self.following_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "visually separating associated items makes implementation and trait boundaries easier for readers and tools to scan",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "insert one blank line before `{}`",
            self.following_name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render diagnostic text before moving the optional insertion span.
        let primary = self.primary_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Suggest a source edit only when no comment or other syntax occupies the boundary.
        cx.tcx.emit_node_span_lint(
            UNSEPARATED_ASSOCIATED_ITEMS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.span_label(self.span, "this associated item needs visual separation");
                diag.note(self.rationale_message().into_owned());
                if let Some(insertion) = self.insertion {
                    diag.span_suggestion(
                        insertion,
                        remediation,
                        "\n",
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(remediation);
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// UnseparatedAssociatedItems: Associated item boundary policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires one visually empty line between associated items.
struct UnseparatedAssociatedItems;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub UNSEPARATED_ASSOCIATED_ITEMS,
    Warn,
    "rejects associated items without a separating blank line",
    UnseparatedAssociatedItems
}

impl UnseparatedAssociatedItems {
    /// Returns whether the source gap contains a complete visually empty line.
    fn has_blank_line(source: &str) -> bool {
        let mut saw_line_break = false;
        let mut line_is_empty = true;
        for byte in source.bytes() {
            if byte != b'\n' {
                line_is_empty &= !saw_line_break || matches!(byte, b' ' | b'\t' | b'\r');
                continue;
            }
            if saw_line_break && line_is_empty {
                return true;
            }
            saw_line_break = true;
            line_is_empty = true;
        }
        false
    }

    /// Returns whether inserting after the previous item preserves comment ownership.
    fn has_safe_insertion(source: &str) -> bool {
        let Some((same_line, following_lines)) = source.split_once('\n') else {
            return false;
        };
        if !same_line.trim().is_empty() {
            return false;
        }
        !following_lines.lines().any(|line| {
            let line = line.trim_start();
            (line.starts_with("//") && !line.starts_with("///"))
                || (line.starts_with("/*") && !line.starts_with("/**"))
        })
    }

    /// Diagnoses every adjacent authored pair without an intervening empty line.
    fn check_items(cx: &LateContext<'_>, items: &[AssociatedItemSource]) {
        let source_map = cx.sess().source_map();
        for pair in items.windows(2) {
            // Resolve the exact authored gap between neighboring declarations.
            let [previous, following] = pair else {
                continue;
            };
            if previous.span.from_expansion() || following.span.from_expansion() {
                continue;
            }
            let gap = Span::with_root_ctxt(previous.span.hi(), following.span.lo());
            let Ok(source) = source_map.span_to_snippet(gap) else {
                continue;
            };

            // Retain only boundaries that lack a complete visually empty line.
            if Self::has_blank_line(&source) {
                continue;
            }

            // Capture precise remediation context without reassigning ordinary comments.
            let insertion = Self::has_safe_insertion(&source).then(|| previous.span.shrink_to_hi());

            // Preserve both neighboring names so the diagnostic explains the exact boundary.
            let violation = Violation {
                hir_id: following.hir_id,
                span: following.span,
                previous_name: previous.name.clone(),
                following_name: following.name.clone(),
                insertion,
            };

            // Render the classified boundary through the shared diagnostic contract.
            violation.emit(cx);
        }
    }

    /// Resolves directly authored implementation items into comparable source identities.
    fn impl_items(cx: &LateContext<'_>, item_ids: &[ImplItemId]) -> Vec<AssociatedItemSource> {
        let mut sources = Vec::new();
        for id in item_ids {
            let item = cx.tcx.hir_impl_item(*id);
            sources.push(AssociatedItemSource::new(
                item.hir_id(),
                item.span,
                item.ident.name.to_string(),
            ));
        }
        sources
    }

    /// Resolves directly authored trait items into comparable source identities.
    fn trait_items(cx: &LateContext<'_>, item_ids: &[TraitItemId]) -> Vec<AssociatedItemSource> {
        let mut sources = Vec::new();
        for id in item_ids {
            let item = cx.tcx.hir_trait_item(*id);
            sources.push(AssociatedItemSource::new(
                item.hir_id(),
                item.span,
                item.ident.name.to_string(),
            ));
        }
        sources
    }
}

impl<'tcx> LateLintPass<'tcx> for UnseparatedAssociatedItems {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Exclude outer declarations produced beyond the current source file.
        if item.span.from_expansion() {
            return;
        }

        // Resolve the associated-item family owned by the declaration.
        let items = match item.kind {
            ItemKind::Impl(implementation) => Self::impl_items(cx, implementation.items),
            ItemKind::Trait(.., item_ids) => Self::trait_items(cx, item_ids),
            _ => return,
        };

        // Diagnose every missing visual boundary in source order.
        Self::check_items(cx, &items);
    }
}
