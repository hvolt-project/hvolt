//! The state one file's lowering carries between passes.
//!
//! A `build()` body is read as a tree and rendered later, so the renderer needs facts that
//! belong to the file being lowered: the page coordinate a child's constructor names, the
//! fields the current struct exposes through a backstore, and whether that struct is
//! `@Reusable`. They are set once per struct, read while its body is rendered.

use std::cell::{Cell, RefCell};

// The state is per-thread: a lowering reads it while it renders one file's body, and tests
// lower files on their own threads, so a process-wide cell would let one test's page override
// another's.
thread_local! {
    /// The coordinate the file being lowered names itself by: `<module>/src/main/ets/<path>.ets`
    /// for a module source, the staged path for a dependency package's source.
    ///
    /// Both readers of the reference take it from the same variable it sets once per file —
    /// `process_ui_syntax.resourceFileName`, read as `path.relative(cwd, resourceFileName)` by
    /// `componentParamRowAndColumn` (`process_component_member.js`) and as `path.relative(
    /// projectRootPath, resourceFileName)` by `navigationOrNavDestinationCreateContent`
    /// (`process_component_build.js`). The staging loop sets this cell at the same boundary
    /// (`ets_assemble`), before either route is taken, so a component file that is not a page
    /// names itself and never inherits the page of the file lowered before it.
    static SOURCE_PAGE: RefCell<Option<String>> = const { RefCell::new(None) };

    /// The fields of the current V1 struct that have a `__` backstore.
    static V1_FIELDS: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };

    /// Whether the struct being expanded is `@Reusable`.
    static REUSABLE: Cell<bool> = const { Cell::new(false) };

    /// Whether the module being assembled is a HAR (`hvolt.toml` `[module] type = "har"`).
    ///
    /// The reference reads the same fact as `projectConfig.compileHar`, and one generated value
    /// depends on it: a `NavDestination`'s page path is `""` while a HAR compiles
    /// (`process_component_build.js::navigationOrNavDestinationCreateContent`).
    static COMPILE_HAR: Cell<bool> = const { Cell::new(false) };

    /// Where the child-block rewrite widened the file: `(line, column, bytes added)`, measured
    /// on the rewritten text in file order.
    ///
    /// The parser reads that text, so a node's reported column is that many columns too far
    /// right once an insertion precedes it on its line. The reference reads its position off the
    /// source node (`CallExpression.getStart()`), so the widening has to be taken back before a
    /// `{ page, line, col }` object writes the column out. Set once per file, next to the text
    /// it describes.
    static COLUMN_SHIFTS: RefCell<Vec<(usize, usize, usize)>> = const { RefCell::new(Vec::new()) };
}

/// Publish where the file's child-block rewrite widened a line.
pub(crate) fn set_column_shifts(shifts: &[(usize, usize, usize)]) {
    COLUMN_SHIFTS.with(|slot| {
        let mut slot = slot.borrow_mut();
        slot.clear();
        slot.extend_from_slice(shifts);
    });
}

/// How far right of the source the rewritten text pushed a column of this file.
pub(crate) fn column_shift(line: usize, column: usize) -> usize {
    COLUMN_SHIFTS.with(|slot| {
        slot.borrow()
            .iter()
            .filter(|(at_line, at_column, _)| *at_line == line && *at_column < column)
            .map(|(_, _, added)| *added)
            .sum()
    })
}

pub(crate) fn set_compile_har(har: bool) {
    COMPILE_HAR.with(|slot| slot.set(har));
}

// Read by a build that compiles a HAR on its own; the `NavDestination` page-path caller was
// withdrawn after measuring that clashbox's HAR reaches the reference as an entry dependency
// (`compileHar` false there), so the flag has no reader yet.
#[allow(dead_code)]
pub(crate) fn is_compile_har() -> bool {
    COMPILE_HAR.with(Cell::get)
}

/// Name the file whose lowering follows.
///
/// Called once per file by the staging loop, and by a lowering driven on its own from the route
/// it was handed: both name the same file, so the two callers agree by construction.
pub(crate) fn set_source_page(page: &str) {
    SOURCE_PAGE.with(|slot| *slot.borrow_mut() = Some(page.to_string()));
}

pub(crate) fn source_page() -> Option<String> {
    SOURCE_PAGE.with(|slot| slot.borrow().clone())
}

pub(crate) fn set_v1_fields(names: &[String]) {
    V1_FIELDS.with(|slot| *slot.borrow_mut() = Some(names.to_vec()));
}

/// Whether a field of the struct being expanded has a `__` backstore.
pub(crate) fn is_v1_field(name: &str) -> bool {
    V1_FIELDS.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|names| names.iter().any(|candidate| candidate == name))
    })
}

pub(crate) fn set_reusable_component(reusable: bool) {
    REUSABLE.with(|slot| slot.set(reusable));
}

/// A reused view updates its attributes only on the first render.
pub(crate) fn is_reusable_component() -> bool {
    REUSABLE.with(std::cell::Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The page coordinate and the field table round-trip; each set replaces the previous one.
    #[test]
    fn the_page_and_the_field_table_are_read_back() {
        set_source_page("entry/src/main/ets/pages/Index.ets");
        assert_eq!(
            source_page().as_deref(),
            Some("entry/src/main/ets/pages/Index.ets")
        );
        set_v1_fields(&["count".to_string()]);
        assert!(is_v1_field("count"));
        assert!(!is_v1_field("label"));
        set_v1_fields(&["label".to_string()]);
        assert!(!is_v1_field("count"));
    }

    /// The reusable flag defaults to false and follows the last write.
    #[test]
    fn the_reusable_flag_follows_the_last_write() {
        set_reusable_component(false);
        assert!(!is_reusable_component());
        set_reusable_component(true);
        assert!(is_reusable_component());
        set_reusable_component(false);
    }

    /// A widening is taken back only from the columns after it, on its own line.
    #[test]
    fn a_published_widening_is_taken_back_from_the_column() {
        set_column_shifts(&[(7, 25, 4)]);
        assert_eq!(column_shift(7, 28), 4, "after the insertion");
        assert_eq!(column_shift(7, 25), 0, "the insertion's own column");
        assert_eq!(column_shift(7, 20), 0, "before the insertion");
        assert_eq!(column_shift(8, 28), 0, "another line");
        set_column_shifts(&[]);
        assert_eq!(column_shift(7, 28), 0, "cleared");
    }
}
