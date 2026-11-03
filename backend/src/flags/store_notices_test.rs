//! The store keeps the deciding document's notices block beside the
//! flags, decided once with them.

use serde_json::json;

use super::{FlagStore, Origin};
use crate::flags::resolve::parse_document;
use crate::flags::test_decls::TEST_DECLS;

fn doc(block: &str) -> crate::flags::resolve::Document {
    parse_document(format!(r#"{{"flags":{{"test_kill":false,"notices":{block}}}}}"#).as_bytes())
        .expect("doc")
}

#[test]
fn a_store_keeps_the_notices_block_of_the_document_that_decided_it() {
    let store = FlagStore::pending(TEST_DECLS);
    assert_eq!(store.notices(), None, "nothing before it settles");
    store.settle(Some(&doc(r#"{"format":1}"#)), Origin::File);
    assert_eq!(store.notices(), Some(json!({ "format": 1 })));
}

#[test]
fn a_later_document_changes_nothing() {
    let store = FlagStore::pending(TEST_DECLS);
    store.settle(Some(&doc(r#"{"format":1}"#)), Origin::Cache);
    store.settle(Some(&doc(r#"{"format":2}"#)), Origin::File);
    assert_eq!(store.notices(), Some(json!({ "format": 1 })));
}

#[test]
fn no_document_or_no_block_is_no_notices() {
    let store = FlagStore::pending(TEST_DECLS);
    store.settle(None, Origin::Defaults);
    assert_eq!(store.notices(), None);
    let store = FlagStore::pending(TEST_DECLS);
    let bare = parse_document(br#"{"flags":{"test_kill":false}}"#).expect("doc");
    store.settle(Some(&bare), Origin::File);
    assert_eq!(store.notices(), None);
}
