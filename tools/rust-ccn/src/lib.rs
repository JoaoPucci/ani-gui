//! Per-function cyclomatic complexity for Rust, on a real parse.

/// One measured unit: a function, or an item-position macro invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub name: String,
    pub line: usize,
    pub end_line: usize,
    pub ccn: u32,
    pub kind: UnitKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Function,
    Macro,
}

/// Measure every unit in one source file, in source order.
pub fn measure(_source: &str) -> syn::Result<Vec<Unit>> {
    Ok(Vec::new())
}
