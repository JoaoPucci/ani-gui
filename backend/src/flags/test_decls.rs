//! Declarations the tests resolve against, one of each kind, so the
//! rules are tested apart from whatever the production list holds.

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
