//! Declarations the tests resolve against. The production list is
//! empty until a flag's consumer lands, so these stand in for one of
//! each kind.

use super::decl::{FlagDecl, FlagKind};

pub(crate) const KILL: &str = "test_kill";
pub(crate) const FALLBACK: &str = "test_fallback";

pub(crate) const TEST_DECLS: &[FlagDecl] = &[
    FlagDecl {
        name: KILL,
        kind: FlagKind::KillSwitch,
        safe_because: "test-only: switching the feature off leaves the rest of the app working",
    },
    FlagDecl {
        name: FALLBACK,
        kind: FlagKind::Fallback {
            current: "new",
            earlier: &["old", "older"],
        },
        safe_because: "test-only: the earlier behaviours shipped before and are known good",
    },
];
