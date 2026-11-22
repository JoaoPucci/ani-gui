//! Property coverage for the working folder's naming: which download,
//! if any, owns a name the sweep finds there.

use super::{owner_of, Entry};
use proptest::prelude::*;

fn key(pid: u32, id: u64) -> String {
    format!(".ani-gui-{pid}-{id:016x}")
}

proptest! {
    /// Every name a hold gives out — its own, and a part under its key
    /// with whatever a tool appends — is owned by exactly that key.
    #[test]
    fn every_name_a_hold_gives_out_is_its_keys(
        pid in any::<u32>(),
        id in any::<u64>(),
        suffix in "[^/\\\\\x00]{1,40}",
    ) {
        let key = key(pid, id);
        let hold = format!("{key}.hold");
        prop_assert_eq!(owner_of(&hold), Some((key.as_str(), Entry::Hold)));
        let part = format!("{key}-{suffix}");
        prop_assert_eq!(owner_of(&part), Some((key.as_str(), Entry::Part)));
    }

    /// A key is never a prefix of another key's names: two downloads'
    /// files are never one owner's to the sweep.
    #[test]
    fn two_keys_never_own_each_others_names(
        a in (any::<u32>(), any::<u64>()),
        b in (any::<u32>(), any::<u64>()),
        suffix in "[a-zA-Z0-9.-]{1,20}",
    ) {
        prop_assume!(a != b);
        let (ka, kb) = (key(a.0, a.1), key(b.0, b.1));
        let name = format!("{kb}-{suffix}");
        prop_assert_ne!(owner_of(&name).map(|(k, _)| k), Some(ka.as_str()));
    }

    /// A name the scheme did not give out is nobody's, so the sweep
    /// leaves it: anything without the prefix, a key whose id is not
    /// 16 lowercase hex, and the bare key with nothing after it.
    #[test]
    fn names_outside_the_scheme_are_nobodys(
        other in "[^.][^\x00]{0,40}",
        pid in any::<u32>(),
        id in any::<u64>(),
        at in 0usize..16,
        bad in "[g-zA-Z_ ]",
    ) {
        prop_assert_eq!(owner_of(&other), None);
        let key = key(pid, id);
        prop_assert_eq!(owner_of(&key), None);
        let bare = format!("{key}-");
        prop_assert_eq!(owner_of(&bare), None);
        let dotted = format!("{key}.part");
        prop_assert_eq!(owner_of(&dotted), None);
        let mut broken = key.clone();
        let hex_start = broken.len() - 16;
        broken.replace_range(hex_start + at..=hex_start + at, &bad);
        let broken_hold = format!("{broken}.hold");
        prop_assert_eq!(owner_of(&broken_hold), None);
    }
}
