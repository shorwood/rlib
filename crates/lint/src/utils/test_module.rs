extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Pos;

/// Canonical authored in-source test-module recognition shared by topology lints.
pub trait CanonicalTestExt {
    /// Returns whether this is a direct `#[cfg(test)] mod test` or `mod tests` declaration.
    fn is_canonical_in_source_test_module(&self, cx: &LateContext<'_>) -> bool;
}

impl CanonicalTestExt for Item<'_> {
    fn is_canonical_in_source_test_module(&self, cx: &LateContext<'_>) -> bool {
        if !cx.sess().opts.test
            || self.span.from_expansion()
            || !matches!(self.kind, ItemKind::Mod(..))
            || !self
                .kind
                .ident()
                .is_some_and(|ident| matches!(ident.name.as_str(), "test" | "tests"))
        {
            return false;
        }

        // `cfg` is consumed before HIR, so recover the directly preceding authored attribute.
        let source_file = cx.sess().source_map().lookup_source_file(self.span.lo());
        let Some(source) = source_file.src.as_deref() else {
            return false;
        };
        let Ok(offset) = usize::try_from((self.span.lo() - source_file.start_pos).to_u32()) else {
            return false;
        };
        let Some(prefix) = source.get(..offset) else {
            return false;
        };
        for line in prefix.lines().rev() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line == "#[cfg(test)]" {
                return true;
            }
            if line.starts_with("#[") {
                continue;
            }
            break;
        }
        false
    }
}
