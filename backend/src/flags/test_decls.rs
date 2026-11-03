//! Declarations the tests resolve against. The production list is
//! empty until a flag's consumer lands, so these stand in for one of
//! each kind.

use super::decl::{FlagDecl, FlagKind};

pub(crate) const KILL: &str = "test_kill";
pub(crate) const FALLBACK: &str = "test_fallback";
pub(crate) const ROLLOUT: &str = "test_rollout";

/// A feature that ships off, which only a verified file may turn on.
pub(crate) const TURN_ON: FlagDecl = FlagDecl {
    name: ROLLOUT,
    kind: FlagKind::Rollout,
    safe_because: "test-only: off is the build's own behaviour; on comes only from a signed file",
};

/// [`TEST_DECLS`] with a rollout beside them.
pub(crate) const WITH_TURN_ON: &[FlagDecl] = &[TEST_DECLS[0], TEST_DECLS[1], TURN_ON];

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
