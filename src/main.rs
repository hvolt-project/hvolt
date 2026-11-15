// main: the binary entry point. The module tree and the CLI live in the
// library crate (`src/lib.rs`), so the same code can be loaded as a cdylib.
fn main() {
    hvolt::main_entry();
}
