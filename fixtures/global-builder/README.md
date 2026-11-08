# global-builder

A global `@Builder function` is emitted by `emit_builder` (`process_ui_syntax.rs`), not copied
through as a statement, so a comment written ahead of its `@Builder` decorator is not carried
into the emitted function: the comment sits in the source trivia before the decorator, while
`emit_builder` rebuilds the declaration from the recorded name, parameters and body. The
fixture therefore keeps its explanation here rather than in `input.ets`.

The two builders cover `parseGlobalBuilderParams`'s one-parameter split: `badge` declares one
parameter and gets `const __text__ = text;` plus the third arrow parameter, `pair` declares two
and gets neither. Both take the trailing `parent = null` parameter.
