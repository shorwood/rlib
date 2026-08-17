extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};

use rustc_hir::HirId;
use rustc_hir::def_id::LocalDefId;
use rustc_lint::{LateContext, LintContext};
use rustc_span::{Pos, Span};

use super::{MethodCandidate, MethodCandidateBindingUse, ReceiverKind};

// -----------------------------------------------------------------------------
// Migration: Conservative automatic migration
// -----------------------------------------------------------------------------

/// One replacement made inside a larger source range.
pub struct MigrationEdit {
    /// Source range replaced by this edit.
    span: Span,
    /// Complete replacement text for `span`.
    replacement: String,
}

impl MigrationEdit {
    /// Returns the source range replaced by this verified edit.
    pub(crate) const fn span(&self) -> Span {
        self.span
    }

    /// Consumes this verified edit and returns its complete replacement text.
    pub(crate) fn into_replacement(self) -> String {
        self.replacement
    }
}

/// A group of replacements made inside one larger source range.
#[derive(Default)]
struct MigrationEdits(
    /// Source replacements accumulated before offset-stable application.
    Vec<MigrationEdit>,
);

impl MigrationEdits {
    /// Adds one source replacement to the group.
    fn push(&mut self, span: Span, replacement: String) {
        self.0.push(MigrationEdit { span, replacement });
    }

    /// Applies every replacement to a source string while preserving the original offsets.
    ///
    /// Editing from right to left ensures that an earlier replacement cannot move the text used by
    /// a later one.
    fn apply_to(&mut self, source: &mut String, outer: Span) -> Option<()> {
        self.0.sort_unstable_by_key(|edit| Reverse(edit.span.lo()));
        for edit in &self.0 {
            // An edit beginning outside the containing item cannot be applied safely.
            let Ok(start) = usize::try_from((edit.span.lo() - outer.lo()).to_u32()) else {
                return None;
            };

            // An edit ending outside the containing item cannot be applied safely.
            let Ok(end) = usize::try_from((edit.span.hi() - outer.lo()).to_u32()) else {
                return None;
            };
            source.replace_range(start..end, &edit.replacement);
        }
        Some(())
    }
}

/// Cross-reference indexes consulted while constructing one `candidate` migration.
struct MigrationReferences<'rule> {
    /// All candidates, used to prevent overlapping simultaneous migrations.
    candidates: &'rule [MethodCandidate],
    /// Body references keyed by first-parameter binding identity.
    binding_uses: &'rule HashMap<HirId, Vec<MethodCandidateBindingUse>>,
    /// Resolved references keyed by free-function definition.
    function_uses: &'rule HashMap<LocalDefId, Vec<Span>>,
    /// Functions whose imported aliases make automatic migration incomplete.
    imported_functions: &'rule HashSet<LocalDefId>,
}

/// Builds all edits needed to move one free function without leaving broken references behind.
pub struct MigrationBuilder<'rule, 'cx, 'tcx> {
    /// Compiler context used for snippets, paths, and source ownership.
    cx: &'cx LateContext<'tcx>,
    /// Candidate currently being migrated.
    candidate: &'rule MethodCandidate,
    /// Cross-reference indexes needed to validate and rewrite the migration.
    references: MigrationReferences<'rule>,
    /// Replacements applied inside the `candidate`'s whole-item edit.
    internal_edits: MigrationEdits,
    /// Call-site replacements outside the `candidate` function.
    external_edits: Vec<MigrationEdit>,
}

impl<'rule, 'cx, 'tcx> MigrationBuilder<'rule, 'cx, 'tcx> {
    /// Starts an empty migration for one `candidate`.
    pub(crate) fn new(
        cx: &'cx LateContext<'tcx>,
        candidate: &'rule MethodCandidate,
        candidates: &'rule [MethodCandidate],
        binding_uses: &'rule HashMap<HirId, Vec<MethodCandidateBindingUse>>,
        function_uses: &'rule HashMap<LocalDefId, Vec<Span>>,
        imported_functions: &'rule HashSet<LocalDefId>,
    ) -> Self {
        // Group the shared reference indexes used by safety checks and rewrites.
        let references = MigrationReferences {
            candidates,
            binding_uses,
            function_uses,
            imported_functions,
        };

        // Initialize candidate-local edits independently from external call-site edits.
        Self {
            cx,
            candidate,
            references,
            internal_edits: MigrationEdits::default(),
            external_edits: Vec::new(),
        }
    }

    /// Returns whether migrating `other` would overlap this `candidate`'s whole-item edit.
    fn overlaps_candidate_migration(&self, other: &MethodCandidate) -> bool {
        other.function.def_id != self.candidate.function.def_id
            && self
                .references
                .function_uses
                .get(&other.function.def_id)
                .is_some_and(|uses| uses.iter().any(|span| self.candidate.contains(*span)))
    }

    /// Rejects moves that cannot be applied as one complete, non-overlapping change.
    ///
    /// This covers imports, generic call syntax, and interactions with other candidates that
    /// Rustfix would otherwise try to edit at the same time.
    fn check_whole_migration_is_safe(&self) -> Option<()> {
        // Require editable local syntax with no imported alias contract.
        if !self.candidate.migration.is_suggestible
            || self
                .references
                .imported_functions
                .contains(&self.candidate.function.def_id)
        {
            return None;
        }

        // Generic call paths may carry turbofish arguments. Rewriting those correctly needs more
        // than replacing the resolved function path, so generic migrations currently stay local.
        if self.candidate.migration.impl_generics_span.is_some()
            && self
                .references
                .function_uses
                .contains_key(&self.candidate.function.def_id)
        {
            return None;
        }

        // Rustfix applies all machine suggestions together. If this function refers to another
        // candidate, moving both would produce overlapping whole-item edits. Keep the caller as a
        // warning-only case and let the callee safely rewrite the reference inside it.
        let overlaps_another_migration = self
            .references
            .candidates
            .iter()
            .any(|other| self.overlaps_candidate_migration(other));
        (!overlaps_another_migration).then_some(())
    }

    /// Reads the original source text covered by a compiler source range.
    fn snippet(&self, span: Span) -> Option<String> {
        let source_map = self.cx.sess().source_map();

        // Unavailable authored text prevents a source-preserving migration edit.
        let Ok(snippet) = source_map.span_to_snippet(span) else {
            return None;
        };
        Some(snippet)
    }

    /// Preserves explicit reference syntax while replacing its binding with `self`.
    fn rewrite_reference_receiver(&self, parameter: &str) -> Option<String> {
        let inner = self.snippet(self.candidate.receiver.receiver_type_span)?;
        let offset = parameter.rfind(&inner)?;
        format!(
            "{}self{}",
            &parameter[..offset],
            &parameter[offset + inner.len()..]
        )
        .split_once(':')
        .map(|(_, receiver)| receiver.trim().to_owned())
    }

    /// Replaces the first parameter with the appropriate `self` spelling.
    ///
    /// It also moves a simple generic parameter to the `impl` and preserves explicit lifetimes and
    /// mutability where their meaning is clear.
    fn rewrite_receiver(&mut self) -> Option<String> {
        // Resolve a plain binding and its complete first-parameter source.
        let binding_name = self.candidate.migration.binding.name?;
        let parameter = self.snippet(self.candidate.receiver.parameter_span)?;
        let pattern = parameter.split_once(':')?.0.trim();

        // Destructured or otherwise transformed parameters cannot become an equivalent receiver.
        if pattern != binding_name.as_str() && pattern != format!("mut {binding_name}") {
            return None;
        }

        // `mut binding: &T` permits reassigning the reference itself. `&mut self` only permits
        // mutating the referent, so that spelling cannot be migrated without semantic analysis.
        if matches!(self.candidate.receiver.semantics.kind, ReceiverKind::Ref(_))
            && pattern.starts_with("mut ")
        {
            return None;
        }

        // Preserve ownership and mutability in the replacement receiver spelling.
        let receiver = match self.candidate.receiver.semantics.kind {
            ReceiverKind::Value if pattern.starts_with("mut ") => "mut self".to_owned(),
            ReceiverKind::Value => "self".to_owned(),
            ReceiverKind::Ref(_) => self.rewrite_reference_receiver(&parameter)?,
        };
        self.internal_edits
            .push(self.candidate.receiver.parameter_span, receiver);

        // Move safe generic syntax from the function to its new impl header.
        self.candidate.migration.impl_generics_span.map_or_else(
            || Some(String::new()),
            |span| {
                let generics = self.snippet(span)?;
                self.internal_edits.push(span, String::new());
                Some(generics)
            },
        )
    }

    /// Applies the edits inside the function, wraps it in an `impl`, and adds call-site edits.
    fn finish(mut self, impl_generics: &str) -> Option<Vec<MigrationEdit>> {
        // Apply candidate-local rewrites before wrapping the function in its impl.
        let mut function = self.snippet(self.candidate.function.item_span)?;
        self.internal_edits
            .apply_to(&mut function, self.candidate.function.item_span)?;
        let self_type = self.snippet(self.candidate.receiver.receiver_type_span)?;
        let moved = format!("impl{impl_generics} {self_type} {{\n{function}\n}}");

        // Combine the whole-item replacement with every external reference rewrite.
        let mut edits = vec![MigrationEdit {
            span: self.candidate.function.item_span,
            replacement: moved,
        }];
        edits.append(&mut self.external_edits);
        Some(edits)
    }

    /// Returns whether a source range is ordinary editable text in the `candidate`'s file.
    ///
    /// Macro expansions and other files are rejected because the displayed edit would not own the
    /// text it claims to change.
    fn is_editable_in_candidate_file(&self, span: Span) -> bool {
        let source_map = self.cx.sess().source_map();
        !span.from_expansion()
            && source_map.span_to_filename(span)
                == source_map.span_to_filename(self.candidate.function.item_span)
    }

    /// Replaces uses of the old parameter name with `self` inside the function body.
    ///
    /// Struct shorthand such as `Snapshot { item }` becomes `Snapshot { item: self }` so the field
    /// name does not accidentally change.
    ///
    /// ```rust
    /// struct Item;
    /// struct Snapshot {
    ///     item: Item,
    /// }
    ///
    /// fn snapshot(item: Item) -> Snapshot {
    ///     Snapshot { item }
    /// }
    /// ```
    fn rewrite_binding_uses(&mut self) -> Option<()> {
        let binding_id = self.candidate.migration.binding.id?;
        for use_ in self
            .references
            .binding_uses
            .get(&binding_id)
            .into_iter()
            .flatten()
        {
            // Every binding use must belong to editable source inside the function being moved.
            if !self.candidate.contains(use_.span) || !self.is_editable_in_candidate_file(use_.span)
            {
                return None;
            }
            let replacement = use_
                .shorthand_field
                .map_or_else(|| "self".to_owned(), |field| format!("{field}: self"));
            self.internal_edits.push(use_.span, replacement);
        }
        Some(())
    }

    /// Rewrites calls and function values to use the method's fully qualified path.
    ///
    /// A reference outside the `candidate`'s source file makes the whole migration warning-only
    /// because one-file suggestions must not leave another file broken.
    ///
    /// A call such as `inspect(&item)` becomes `crate::Item::inspect(&item)`. Using the complete
    /// method name also preserves places where the old function was stored as a function value.
    fn rewrite_function_uses(&mut self) -> Option<()> {
        let qualified_method = self.candidate.qualified_method_path(self.cx);
        for span in self
            .references
            .function_uses
            .get(&self.candidate.function.def_id)
            .into_iter()
            .flatten()
        {
            // A cross-file or expanded reference makes the one-file migration incomplete.
            if !self.is_editable_in_candidate_file(*span) {
                return None;
            }
            if self.candidate.contains(*span) {
                self.internal_edits.push(*span, qualified_method.clone());
            } else {
                self.external_edits.push(MigrationEdit {
                    span: *span,
                    replacement: qualified_method.clone(),
                });
            }
        }
        Some(())
    }

    /// Builds the complete edit set, or declines when any part of the move is uncertain.
    ///
    /// Each stage must succeed before the suggestion is marked as safe for automatic application.
    pub(crate) fn build(mut self) -> Option<Vec<MigrationEdit>> {
        self.check_whole_migration_is_safe()?;
        let impl_generics = self.rewrite_receiver()?;
        self.rewrite_binding_uses()?;
        self.rewrite_function_uses()?;
        self.finish(&impl_generics)
    }
}
