//! What the CRAP gate's Rust complexity counts. The cases that matter
//! most are the ones lizard's tokenizer lost its place on: each of them
//! used to drop functions, or decisions, from the totals without a word.

use proptest::prelude::*;
use rust_ccn::{measure, measure_file, Unit, UnitKind};

fn units(source: &str) -> Vec<(String, u32)> {
    measure(source)
        .expect("parses")
        .into_iter()
        .map(|u| (u.name, u.ccn))
        .collect()
}

fn named(list: &[(&str, u32)]) -> Vec<(String, u32)> {
    list.iter().map(|(n, c)| ((*n).to_owned(), *c)).collect()
}

#[test]
fn a_word_character_literal_keeps_the_decisions_after_it() {
    // lizard read `'_` as a lifetime and the next `'` as the start of a
    // quoted run, which swallowed the `||` and, further on, functions.
    let src = r#"
        fn is_word(c: char) -> bool { c == '_' || c == 'a' }
        fn after(x: bool) -> u32 { if x { 1 } else { 2 } }
    "#;
    assert_eq!(units(src), named(&[("is_word", 2), ("after", 2)]));
}

#[test]
fn a_quote_character_literal_is_one_token() {
    let src = r#"
        fn split(s: &str) -> Option<&str> { s.split('"').next() }
        fn after(x: bool) -> u32 { if x { 1 } else { 2 } }
    "#;
    assert_eq!(units(src), named(&[("split", 1), ("after", 2)]));
}

#[test]
fn a_raw_string_is_text_however_it_reads() {
    // On this shape lizard dropped the function holding the raw
    // string outright; elsewhere it kept the function and lost the
    // decisions on the raw string's line.
    let src = r###"
        fn page() -> &'static str { r#"<a href="x"> fn if && || match "#; "" }
        fn after(x: bool) -> u32 { if x { 1 } else { 2 } }
    "###;
    assert_eq!(units(src), named(&[("page", 1), ("after", 2)]));
}

#[test]
fn a_declaration_without_a_body_is_not_a_function() {
    // lizard read each bodiless declaration as a function running on
    // into the next body: the default method under the trait
    // declaration's name, the function after the extern block under
    // the extern declaration's.
    let src = r#"
        trait Probe {
            fn required(&self) -> u32;
            fn provided(&self) -> u32 { if self.required() > 1 { 1 } else { 0 } }
        }
        extern "C" { fn abs(x: i32) -> i32; }
        fn after(x: bool) -> u32 { if x { 1 } else { 2 } }
    "#;
    assert_eq!(units(src), named(&[("provided", 2), ("after", 2)]));
}

#[test]
fn methods_and_nested_functions_are_units_of_their_own() {
    let src = r#"
        struct S;
        impl S {
            fn method(&self, x: Option<u32>) -> Option<u32> {
                fn inner(v: u32) -> u32 { if v > 1 { v } else { 0 } }
                Some(inner(x?))
            }
        }
    "#;
    // The nested function's `if` is its own, not the method's too.
    assert_eq!(units(src), named(&[("method", 2), ("inner", 2)]));
}

#[test]
fn the_count_is_lizards() {
    // 1 + if + else-if's if + && + || + match (once, not per arm)
    //   + while + for + ? + where. The empty closure is a unit of its
    //   own, and its `||` is its parameter list, not a decision.
    let src = r#"
        fn all<T>(t: T, xs: &[u32], o: Option<u32>) -> Option<u32>
        where
            T: Clone,
        {
            if xs.is_empty() && o.is_none() || xs.len() > 3 {
                return None;
            } else if xs.len() == 1 {
                return Some(1);
            }
            match o { Some(1) => {}, Some(_) => {}, None => {} }
            while false {}
            for _ in xs {}
            let f = || 1;
            let _ = t.clone();
            Some(o? + f())
        }
    "#;
    assert_eq!(units(src), named(&[("all", 10), ("(closure)", 1)]));
}

#[test]
fn a_closure_is_a_unit_of_its_own() {
    // As an arrow function is in TypeScript: its own base path, and its
    // decisions are its own, not the function's around it.
    let src = r#"
        fn outer(xs: &[u32]) -> Vec<u32> {
            let keep = |x: &u32| *x > 1 && *x < 9;
            xs.iter().copied().filter(|x| keep(x)).map(|x| if x > 3 { x } else { 0 }).collect()
        }
    "#;
    assert_eq!(
        units(src),
        named(&[
            ("outer", 1),
            ("(closure)", 2),
            ("(closure)", 1),
            ("(closure)", 2)
        ])
    );
}

#[test]
fn a_closure_in_a_constant_is_a_unit_not_an_outside_decision() {
    let src = "static PICK: fn(u32) -> u32 = |x| if x > 1 { x } else { 0 };\nconst ID: fn(u32) -> u32 = |x| x;\n";
    let measured = measure_file(src).expect("parses");
    let got: Vec<(String, u32)> = measured
        .units
        .into_iter()
        .map(|u| (u.name, u.ccn))
        .collect();
    assert_eq!(got, named(&[("(closure)", 2), ("(closure)", 1)]));
    assert_eq!(measured.outside, 0);
}

#[test]
fn an_async_block_is_charged_to_the_function_around_it() {
    // An async block is an expression the function evaluates, not
    // something called: no unit of its own.
    let src = r#"
        fn spawn(x: bool) { let _f = async move { if x { 1 } else { 0 } }; }
    "#;
    assert_eq!(units(src), named(&[("spawn", 2)]));
}

#[test]
fn a_closure_inside_macro_input_stays_with_the_function_around_it() {
    // Macro input is not parsed, so a closure written there is not seen
    // as one: its decisions count for the enclosing function, without a
    // base path of its own.
    let src = r#"
        fn checks(a: bool, b: bool) { assert!((|x: bool| x && b)(a)); }
    "#;
    assert_eq!(units(src), named(&[("checks", 2)]));
}

#[test]
fn tokens_inside_a_macro_call_in_a_body_count_for_the_body() {
    let src = r#"
        fn checks(a: bool, b: bool) { assert!(a && b, "{}", if a { 1 } else { 2 }); }
    "#;
    assert_eq!(units(src), named(&[("checks", 3)]));
}

#[test]
fn a_macro_in_item_position_is_measured_as_one_block() {
    // A parser cannot see functions inside macro input; what it can do
    // is keep their decisions in the file's total, under a name that
    // says what they are.
    let src = r#"
        proptest! {
            #[test]
            fn holds(x in 0u32..9) { if x > 3 { prop_assert!(x > 3 && x < 9); } }
        }
    "#;
    let measured = measure(src).expect("parses");
    assert_eq!(measured.len(), 1);
    assert_eq!(measured[0].name, "proptest!");
    assert_eq!(measured[0].kind, UnitKind::Macro);
    assert_eq!(measured[0].ccn, 3);
}

#[test]
fn a_unit_spans_from_its_fn_keyword_to_its_closing_brace() {
    let src = "fn a() {}\n\npub async fn b(\n    x: u32,\n) -> u32 {\n    x\n}\n";
    let measured = measure(src).expect("parses");
    assert_eq!(
        measured,
        vec![
            Unit {
                name: "a".into(),
                line: 1,
                end_line: 1,
                ccn: 1,
                kind: UnitKind::Function
            },
            Unit {
                name: "b".into(),
                line: 3,
                end_line: 7,
                ccn: 1,
                kind: UnitKind::Function
            },
        ]
    );
}

#[test]
fn decisions_outside_any_function_are_reported_not_counted() {
    // The `for` of an impl header and a constant's initializer belong
    // to no function; they are not charged to one, and the count of
    // them is kept so the report can say what it left out.
    let src = "struct S;\ntrait T {}\nimpl T for S {}\nconst N: u32 = if true { 1 } else { 2 };\nfn f() {}\n";
    let measured = measure_file(src).expect("parses");
    assert_eq!(measured.units.len(), 1);
    assert_eq!(measured.units[0].ccn, 1);
    assert_eq!(measured.outside, 2);
}

#[test]
fn a_file_that_does_not_parse_is_an_error_not_an_empty_count() {
    assert!(measure("fn broken( {").is_err());
}

/// Literals lizard misread, placed between functions.
fn decoy() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "let _ = '_';",
        "let _ = 'a';",
        "let _ = '\"';",
        "let _ = '\\'';",
        "let _ = b'x';",
        "let _ = r#\"a \"quoted\" fn if\"#;",
        "let _ = \"fn x() { if }\";",
        "let _ = 'label: loop { break 'label; };",
    ])
}

proptest! {
    /// However the literals fall, every function is a unit carrying
    /// exactly its own decisions.
    #[test]
    fn every_function_is_counted(
        bodies in prop::collection::vec((0u32..4, decoy()), 1..12)
    ) {
        let mut src = String::new();
        for (i, (ifs, decoy)) in bodies.iter().enumerate() {
            src.push_str(&format!("fn f{i}(x: u32) -> u32 {{ {decoy} "));
            for _ in 0..*ifs {
                src.push_str("if x > 1 { return 1; } ");
            }
            src.push_str("x }\n");
        }
        let measured = measure(&src).expect("parses");
        let expected: Vec<(String, u32)> = bodies
            .iter()
            .enumerate()
            .map(|(i, (ifs, _))| (format!("f{i}"), ifs + 1))
            .collect();
        let got: Vec<(String, u32)> = measured.into_iter().map(|u| (u.name, u.ccn)).collect();
        prop_assert_eq!(got, expected);
    }
}
