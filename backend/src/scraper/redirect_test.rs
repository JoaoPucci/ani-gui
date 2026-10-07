//! The hop rule over arbitrary paths, hosts, ports and statuses.

use super::same_origin_hop;

proptest::proptest! {
    #[test]
    fn a_redirect_on_the_origin_is_followed(
        status in 300u16..400,
        path in "(/[a-z0-9]{1,8}){1,3}",
    ) {
        proptest::prop_assert_eq!(
            same_origin_hop("https://anidb.app/browse?q=x", status, Some(&path)),
            Some(format!("https://anidb.app{path}"))
        );
    }

    #[test]
    fn a_redirect_off_the_origin_is_not(
        status in 300u16..400,
        host in "[a-z]{1,10}\\.(so|net)",
        port in 1u16..,
    ) {
        let from = "https://anidb.app/browse?q=x";
        let other = format!("https://{host}/");
        proptest::prop_assert_eq!(same_origin_hop(from, status, Some(&other)), None);
        let on_port = format!("https://anidb.app:{port}/");
        if port != 443 {
            proptest::prop_assert_eq!(same_origin_hop(from, status, Some(&on_port)), None);
        }
    }

    #[test]
    fn only_a_redirect_status_moves(status in 0u16..1000, path in "/[a-z]{0,8}") {
        proptest::prop_assume!(!(300..400).contains(&status));
        proptest::prop_assert_eq!(
            same_origin_hop("https://anidb.app/x", status, Some(&path)),
            None
        );
    }
}

#[test]
fn a_redirect_without_a_target_or_from_nowhere_is_the_answer() {
    assert_eq!(same_origin_hop("https://anidb.app/x", 302, None), None);
    assert_eq!(same_origin_hop("not a url", 302, Some("/y")), None);
    assert_eq!(
        same_origin_hop("https://anidb.app/x", 301, Some("http://anidb.app/x")),
        None,
        "a scheme change leaves the origin"
    );
}
