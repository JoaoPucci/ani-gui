use std::time::Duration;

use proptest::prelude::*;

use super::{retry_delay, RETRY_DELAYS, RETRY_EVERY};

proptest! {
    #[test]
    fn retries_never_speed_up_and_never_wait_past_an_hour(a in any::<u32>(), b in any::<u32>()) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(retry_delay(lo) <= retry_delay(hi));
        prop_assert!(retry_delay(a) <= RETRY_EVERY);
        prop_assert!(retry_delay(a) >= RETRY_DELAYS[0]);
        prop_assert!(retry_delay(a) >= Duration::from_secs(60));
    }
}
