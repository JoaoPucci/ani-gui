//! Strongly connected components of a run's links (Tarjan): members of
//! a component larger than one lead round to one another, a cycle.

use std::collections::{BTreeMap, BTreeSet};

/// Tarjan's working state.
struct Tarjan<'a> {
    out: &'a BTreeMap<String, Vec<String>>,
    index: BTreeMap<String, usize>,
    low: BTreeMap<String, usize>,
    stack: Vec<String>,
    on_stack: BTreeSet<String>,
    comp: BTreeMap<String, usize>,
    counter: usize,
    count: usize,
}

impl Tarjan<'_> {
    fn visit(&mut self, v: &str) {
        self.index.insert(v.to_string(), self.counter);
        self.low.insert(v.to_string(), self.counter);
        self.counter += 1;
        self.stack.push(v.to_string());
        self.on_stack.insert(v.to_string());
        let out = self.out;
        for w in out.get(v).into_iter().flatten() {
            if !self.index.contains_key(w) {
                self.visit(w);
                let low = self.low[v].min(self.low[w]);
                self.low.insert(v.to_string(), low);
            } else if self.on_stack.contains(w) {
                let low = self.low[v].min(self.index[w]);
                self.low.insert(v.to_string(), low);
            }
        }
        if self.low[v] == self.index[v] {
            while let Some(w) = self.stack.pop() {
                self.on_stack.remove(&w);
                let done = w == v;
                self.comp.insert(w, self.count);
                if done {
                    break;
                }
            }
            self.count += 1;
        }
    }
}

/// The component of every id `ids` and `out` name, numbered in the
/// order Tarjan closes them.
pub(super) fn components(
    ids: &[String],
    out: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, usize> {
    let mut t = Tarjan {
        out,
        index: BTreeMap::new(),
        low: BTreeMap::new(),
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        comp: BTreeMap::new(),
        counter: 0,
        count: 0,
    };
    for v in ids {
        if !t.index.contains_key(v) {
            t.visit(v);
        }
    }
    t.comp
}
