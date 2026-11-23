//! [`super::keyed_title`] and [`super::title_year`].

use super::{keyed_title, title_year};
use proptest::prelude::*;

#[test]
fn a_named_year_follows_the_title() {
    assert_eq!(
        keyed_title("JoJo's Bizarre Adventure", Some(2000)),
        "JoJo's Bizarre Adventure (2000)"
    );
    assert_eq!(
        keyed_title("JoJo's Bizarre Adventure", None),
        "JoJo's Bizarre Adventure"
    );
}

#[test]
fn a_title_that_ends_with_its_year_is_keyed_as_it_is() {
    assert_eq!(keyed_title("Show (2009)", Some(2009)), "Show (2009)");
    assert_eq!(keyed_title("Show (2009) ", Some(2009)), "Show (2009) ");
    // Another year than the one it ends with is still named.
    assert_eq!(keyed_title("Show (2009)", Some(2012)), "Show (2009) (2012)");
}

#[test]
fn a_title_names_its_trailing_year_before_or_after_a_legacy_tail() {
    assert_eq!(title_year("Show (2009)"), Some(2009));
    assert_eq!(title_year("Show (12 episodes) (2009)"), Some(2009));
    assert_eq!(title_year("Show (2009) (12 episodes)"), Some(2009));
    assert_eq!(title_year("Show ( 1 episode ) ( 2009 )"), Some(2009));
}

#[test]
fn a_title_without_a_trailing_year_names_none() {
    assert_eq!(title_year("Show"), None);
    assert_eq!(title_year("Show (12 episodes)"), None);
    assert_eq!(title_year("Show 2009"), None);
    assert_eq!(title_year("Blade Runner (2049)"), None);
    assert_eq!(title_year("Show (1080)"), None);
}

fn title() -> impl Strategy<Value = String> {
    prop::collection::vec("[A-Za-z][a-z']{0,6}", 1..4).prop_map(|w| w.join(" "))
}

proptest! {
    /// Two years key apart, the same year keys alike, and no year keys
    /// the title itself.
    #[test]
    fn years_key_apart(t in title(), a in 1960u32..2040, b in 1960u32..2040) {
        prop_assert_eq!(keyed_title(&t, Some(a)) == keyed_title(&t, Some(b)), a == b);
        prop_assert_ne!(keyed_title(&t, Some(a)), keyed_title(&t, None));
        prop_assert_eq!(keyed_title(&t, None), t.clone());
    }

    /// Keying is idempotent: a keyed title keys as itself.
    #[test]
    fn keying_twice_is_keying_once(t in title(), y in 1960u32..2040) {
        let once = keyed_title(&t, Some(y));
        prop_assert_eq!(keyed_title(&once, Some(y)), once);
    }

    /// Every legacy shape the history keeps names its year.
    #[test]
    fn every_legacy_shape_names_its_year(
        t in title(), y in 1960u32..2040, n in 1u32..2000, shape in 0usize..4, one in any::<bool>(),
    ) {
        let tail = format!("({n} episode{})", if one { "" } else { "s" });
        let titled = match shape {
            0 => format!("{t} ({y})"),
            1 => format!("{t} {tail} ({y})"),
            2 => format!("{t} ({y}) {tail}"),
            _ => format!("{t}  {tail}  ( {y} )"),
        };
        prop_assert_eq!(title_year(&titled), Some(y));
        prop_assert_eq!(title_year(&format!("{t} {tail}")), None);
    }
}
