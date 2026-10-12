//! The origin rule over arbitrary paths, hosts and ports: a request
//! on the provider's origin is held to it, a request elsewhere is
//! not, and a landing URL that cannot be read is never the origin.

use super::answered_elsewhere;

const BASE: &str = "https://hianime.at";

proptest::proptest! {
    #[test]
    fn a_landing_on_the_origin_is_the_origin_answering(
        asked in "/[a-z0-9/?=&+-]{0,24}",
        landed in "/[a-z0-9/?=&+-]{0,24}",
    ) {
        let asked = format!("{BASE}{asked}");
        let landed = format!("{BASE}{landed}");
        proptest::prop_assert!(!answered_elsewhere(BASE, &asked, &landed));
    }

    #[test]
    fn a_landing_on_another_host_or_port_is_elsewhere(
        asked in "/[a-z0-9/?=&+-]{0,24}",
        host in "[a-z]{1,12}\\.(so|to|net)",
        port in proptest::option::of(1u16..),
    ) {
        let landed = match port {
            Some(p) if p != 443 => format!("https://hianime.at:{p}/"),
            _ => format!("https://{host}/"),
        };
        let asked = format!("{BASE}{asked}");
        proptest::prop_assert!(answered_elsewhere(BASE, &asked, &landed));
    }

    #[test]
    fn a_request_off_the_origin_is_not_held_to_it(
        host in "[a-z]{1,12}\\.example",
        landed in ".{0,40}",
    ) {
        let asked = format!("https://{host}/e/1");
        proptest::prop_assert!(!answered_elsewhere(BASE, &asked, &landed));
    }
}

#[test]
fn an_unreadable_landing_is_elsewhere() {
    assert!(answered_elsewhere(
        BASE,
        "https://hianime.at/search?keyword=x",
        ""
    ));
    assert!(answered_elsewhere(
        BASE,
        "https://hianime.at/search?keyword=x",
        "not a url"
    ));
}

#[test]
fn an_unreadable_origin_holds_nothing() {
    assert!(!answered_elsewhere(
        "not a url",
        "https://hianime.at/search?keyword=x",
        "https://unrelated.example/"
    ));
}

#[test]
fn a_scheme_change_leaves_the_origin() {
    assert!(answered_elsewhere(
        BASE,
        "https://hianime.at/search?keyword=x",
        "http://hianime.at/search?keyword=x"
    ));
}
