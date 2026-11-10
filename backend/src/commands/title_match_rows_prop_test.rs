//! Properties of the cour a history row's title match is keyed by.

use super::*;
use proptest::prelude::*;

fn base() -> impl Strategy<Value = String> {
    "[A-Z][a-z]{2,8}( [A-Z][a-z]{2,8}){0,2}"
}

proptest! {
    /// The cour a row's title names, in each form the page reads,
    /// with or without a legacy episode tail; a bare title is the first.
    #[test]
    fn a_row_s_cour_is_the_one_its_title_names(
        title in base(),
        cour in 1u32..30,
        keyword in prop::sample::select(vec!["Part", "Cour", "Season"]),
        tail in any::<bool>(),
    ) {
        let tailed = |t: String| if tail { format!("{t} (12 episodes)") } else { t };
        prop_assert_eq!(row_cour(&tailed(title.clone())), 1);
        prop_assert_eq!(row_cour(&tailed(format!("{title} {keyword} {cour}"))), cour);
        let ordinal = match cour % 10 { 1 if cour != 11 => "st", 2 if cour != 12 => "nd", 3 if cour != 13 => "rd", _ => "th" };
        prop_assert_eq!(row_cour(&tailed(format!("{title} {cour}{ordinal} {keyword}"))), cour);
    }
}
