//! Property coverage for the ignored-signal mask's pure helpers.

use super::{ignores, sig_ign_mask};

proptest::proptest! {
    /// Whatever mask the kernel prints, in the shape it prints it —
    /// sixteen hexadecimal digits after a tab, among other lines — is
    /// the mask read back.
    #[test]
    fn a_printed_mask_is_read_back(mask in proptest::num::u64::ANY) {
        let status = format!(
            "Name:\tani-gui-backend\nSigBlk:\t0000000000000000\nSigIgn:\t{mask:016x}\nSigCgt:\t0000000000004000\n"
        );
        proptest::prop_assert_eq!(sig_ign_mask(&status), Some(mask));
    }

    /// Signal `n` is ignored exactly when bit `n - 1` is set, for
    /// every signal the mask can name.
    #[test]
    fn a_signal_is_ignored_exactly_when_its_bit_is_set(
        mask in proptest::num::u64::ANY,
        signal in 1_i32..=64,
    ) {
        let bit_set = (mask >> (signal - 1)) & 1 == 1;
        proptest::prop_assert_eq!(ignores(mask, signal), bit_set);
    }
}
