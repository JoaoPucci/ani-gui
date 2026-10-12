//! The availability arguments carry the caller's subtype.

use super::*;

#[test]
fn availability_args_carry_the_callers_subtype() {
    // The synthesized PlayArgs dropped subtype, so availability
    // accepted the first format where Movie, Special and OVA
    // candidates tie on title, year and count — caching the
    // wrong candidate's availability and cap while play and
    // download select or reject a different entry. The wire
    // field is optional: probes without a subtype stay valid.
    let args: AvailabilityArgs =
        serde_json::from_str(r#"{"title":"x","mode":"sub","subtype":"special"}"#)
            .expect("optional subtype deserializes");
    assert_eq!(args.subtype.as_deref(), Some("special"));
}
