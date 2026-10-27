//! What a look at a tree reads as.

use super::still_running;

/// A look that could not be made — kill(1) not spawned under fork
/// pressure, a job whose members could not be listed — says nothing
/// about the tree. Read as gone, it ends a teardown with the tree
/// perhaps still running: a tool's helper left beside the respawn or
/// the sweep that follows. Read as running, the teardown looks again,
/// and asks again, until the tree is seen gone or its ceiling passes.
#[test]
fn a_look_that_could_not_be_made_does_not_read_as_gone() {
    let failed: Result<bool, std::io::Error> = Err(std::io::Error::other("fork failed"));
    assert!(still_running(failed));
}

proptest::proptest! {
    /// A look that was made is read as it answered; one that could not
    /// be made is read as running, whatever kept it from being made.
    #[test]
    fn a_look_is_read_as_it_answered_or_as_running(
        answered in proptest::bool::ANY,
        why in "[a-z ]{0,20}",
    ) {
        proptest::prop_assert_eq!(still_running::<std::io::Error>(Ok(answered)), answered);
        let failed: Result<bool, std::io::Error> = Err(std::io::Error::other(why));
        proptest::prop_assert!(still_running(failed));
    }
}
