//! Declarations the tests resolve against, apart from whatever the
//! production list holds: two rollouts, which ship off and which a
//! document may turn on, so a test can tell a document's values from
//! the defaults. Resolution reads a document as it is handed one; that
//! only a verified file's document can turn them on is the signature's
//! part ([`super::signature::admit`]).

use super::decl::{FlagDecl, FlagKind};

pub(crate) const A: &str = "test_a";
pub(crate) const B: &str = "test_b";

pub(crate) const TEST_DECLS: &[FlagDecl] = &[
    FlagDecl {
        name: A,
        kind: FlagKind::Rollout,
        safe_because: "test-only: the flag gates nothing, and only a signed file turns it on",
    },
    FlagDecl {
        name: B,
        kind: FlagKind::Rollout,
        safe_because: "test-only: the flag gates nothing, and only a signed file turns it on",
    },
];
