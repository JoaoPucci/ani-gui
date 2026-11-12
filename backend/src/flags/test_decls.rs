//! Declarations the tests resolve against. This build declares no
//! flag, and no kind a build could declare one with, so these use the
//! test-only probe kind: two on/off flags that ship off and that the
//! file may switch on, so a test can tell the file's values from the
//! defaults.

use super::decl::{FlagDecl, FlagKind};

pub(crate) const A: &str = "test_a";
pub(crate) const B: &str = "test_b";

pub(crate) const TEST_DECLS: &[FlagDecl] = &[
    FlagDecl {
        name: A,
        kind: FlagKind::Probe {
            may_switch_on: true,
        },
        safe_because: "test-only: the flag gates nothing",
    },
    FlagDecl {
        name: B,
        kind: FlagKind::Probe {
            may_switch_on: true,
        },
        safe_because: "test-only: the flag gates nothing",
    },
];
