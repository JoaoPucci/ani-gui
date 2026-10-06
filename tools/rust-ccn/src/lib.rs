//! Per-function cyclomatic complexity for Rust, on a real parse.
//!
//! The CRAP gate used to take its Rust complexity from lizard, whose
//! Rust support is a regex tokenizer driving a small state machine.
//! That tokenizer misreads ordinary constructs and loses its place: a
//! char literal of a word character (`'_'`, `'a'`, `b'x'`) is read as a
//! lifetime plus a stray quote that opens a run to the next one; a raw
//! string (`r#"..."#`) can drop the function holding it, or the
//! decisions on its line; a bodiless trait or extern declaration is
//! read as a function running on into the next body, which loses that
//! function's own entry. Each drops decisions or whole functions from
//! the counts, and nothing reports the loss, so the gate under-measured
//! while reading green.
//!
//! Here `syn` parses each file. A file it cannot parse is an error, not
//! a smaller number. Function boundaries come from the syntax tree:
//! every `fn` item, every `impl` method and every trait method with a
//! default body is a unit. Within a unit the count is lizard's, kept so
//! the numbers stay continuous with the ceilings recorded against it:
//! 1, plus one for each `if`, `for`, `while`, `match` and `where`
//! keyword, each `?`, and each `&&` and `||` token. Counting is over the
//! token stream, so the tokens inside macro invocations count the way
//! lizard counted them. A function nested inside another is its own
//! unit, and its tokens are not counted again in the enclosing one.
//!
//! What a parser cannot see it says so about. A macro invocation in
//! item position — `proptest! { ... }`, `macro_rules!` — holds tokens
//! that are not Rust syntax until expansion, so the functions written
//! inside one are not visible as functions. Such an invocation becomes a
//! unit of its own, named `name!` and counted like a function (1 plus
//! the decision tokens it contains), and is listed as such: the
//! complexity is measured, the function boundaries inside it are not.
//! Decisions outside every unit — the `for` of `impl Trait for Type`, a
//! constant's initializer — are charged to none, as lizard charged
//! them to none; [`Measurement::outside`] counts them so the report can
//! say so.

use proc_macro2::{Spacing, TokenStream, TokenTree};
use syn::visit::{self, Visit};

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
    /// An item-position macro invocation, measured as one block because
    /// its contents are not parseable as Rust.
    Macro,
}

/// Every unit in a file, and how many decision tokens sat outside all
/// of them — the `for` of `impl Trait for Type`, a constant's
/// initializer — which no unit is charged with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement {
    pub units: Vec<Unit>,
    pub outside: u32,
}

/// Measure every unit in one source file, in source order.
pub fn measure(source: &str) -> syn::Result<Vec<Unit>> {
    Ok(measure_file(source)?.units)
}

/// A source position, as (line, column).
type Pos = (usize, usize);

/// Measure one source file: its units in source order, and the
/// decisions outside them.
///
/// The syntax tree decides where units begin and end; the counting is
/// over the file's own lexed tokens rather than over the tree printed
/// back out, because printing loses the spacing that tells `||` (one
/// token) from the two pipes of an empty closure parameter list, and
/// lizard read the source, not a reprint of it. Each decision token is
/// charged to the innermost unit whose span holds it — spans nest, so
/// that is the holder that starts last.
pub fn measure_file(source: &str) -> syn::Result<Measurement> {
    let file = syn::parse_file(source)?;
    let mut census = Census::default();
    census.visit_file(&file);
    let tokens: TokenStream = source
        .parse()
        .map_err(|e: proc_macro2::LexError| syn::Error::new(e.span(), e.to_string()))?;
    let mut decision_positions = Vec::new();
    decisions(tokens, &mut decision_positions);

    let mut own = vec![0u32; census.units.len()];
    let mut outside = 0;
    for pos in decision_positions {
        let innermost = census
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.start <= pos && pos <= u.end)
            .max_by_key(|(_, u)| u.start)
            .map(|(i, _)| i);
        match innermost {
            Some(i) => own[i] += 1,
            None => outside += 1,
        }
    }
    let mut units: Vec<Unit> = census
        .units
        .into_iter()
        .zip(own)
        .map(|(u, own)| Unit {
            name: u.name,
            line: u.start.0,
            end_line: u.end.0,
            ccn: 1 + own,
            kind: u.kind,
        })
        .collect();
    units.sort_by_key(|u| (u.line, u.end_line));
    Ok(Measurement { units, outside })
}

/// Collect the position of every decision token, recursing into groups.
fn decisions(tokens: TokenStream, out: &mut Vec<Pos>) {
    let mut joint_prev: Option<char> = None;
    for tree in tokens {
        let mut this_joint = None;
        match tree {
            TokenTree::Group(group) => decisions(group.stream(), out),
            TokenTree::Ident(ident) => {
                if matches!(
                    ident.to_string().as_str(),
                    "if" | "for" | "while" | "match" | "where"
                ) {
                    out.push(pos(ident.span().start()));
                }
            }
            TokenTree::Punct(punct) => {
                let ch = punct.as_char();
                if ch == '?' {
                    out.push(pos(punct.span().start()));
                }
                // `&&` and `||` lex as two puncts, the first Joint.
                if (ch == '&' || ch == '|') && joint_prev == Some(ch) {
                    out.push(pos(punct.span().start()));
                } else if punct.spacing() == Spacing::Joint {
                    this_joint = Some(ch);
                }
            }
            TokenTree::Literal(_) => {}
        }
        joint_prev = this_joint;
    }
}

fn pos(at: proc_macro2::LineColumn) -> Pos {
    (at.line, at.column)
}

struct Open {
    name: String,
    start: Pos,
    end: Pos,
    kind: UnitKind,
}

#[derive(Default)]
struct Census {
    units: Vec<Open>,
    /// Depth of function bodies currently being walked.
    depth: usize,
}

impl Census {
    fn function<F: FnOnce(&mut Self)>(
        &mut self,
        sig: &syn::Signature,
        body: &syn::Block,
        recurse: F,
    ) {
        self.units.push(Open {
            name: sig.ident.to_string(),
            start: pos(sig.fn_token.span.start()),
            end: pos(body.brace_token.span.close().end()),
            kind: UnitKind::Function,
        });
        self.depth += 1;
        recurse(self);
        self.depth -= 1;
    }

    fn item_macro(&mut self, mac: &syn::Macro) {
        // Inside a function the tokens already belong to it.
        if self.depth > 0 {
            return;
        }
        let Some(first) = mac.path.segments.first() else {
            return;
        };
        let last = mac.path.segments.last().unwrap_or(first);
        self.units.push(Open {
            name: format!("{}!", last.ident),
            start: pos(first.ident.span().start()),
            end: pos(delimiter_close(&mac.delimiter).end()),
            kind: UnitKind::Macro,
        });
    }
}

fn delimiter_close(delimiter: &syn::MacroDelimiter) -> proc_macro2::Span {
    match delimiter {
        syn::MacroDelimiter::Paren(d) => d.span.close(),
        syn::MacroDelimiter::Brace(d) => d.span.close(),
        syn::MacroDelimiter::Bracket(d) => d.span.close(),
    }
}
impl<'ast> Visit<'ast> for Census {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.function(&node.sig, &node.block, |v| visit::visit_item_fn(v, node));
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.function(&node.sig, &node.block, |v| {
            visit::visit_impl_item_fn(v, node)
        });
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        match &node.default {
            Some(body) => self.function(&node.sig, body, |v| visit::visit_trait_item_fn(v, node)),
            None => visit::visit_trait_item_fn(self, node),
        }
    }

    fn visit_item_macro(&mut self, node: &'ast syn::ItemMacro) {
        self.item_macro(&node.mac);
    }

    fn visit_impl_item_macro(&mut self, node: &'ast syn::ImplItemMacro) {
        self.item_macro(&node.mac);
    }

    fn visit_trait_item_macro(&mut self, node: &'ast syn::TraitItemMacro) {
        self.item_macro(&node.mac);
    }
}
