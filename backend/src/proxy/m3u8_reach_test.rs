//! The rewrite keeps a URI as written when it is already on the proxy,
//! and decides that by the proxy's scheme, host and port, never by how
//! the URI's text begins. `http://127.0.0.1:<port>@evil.example/` is a
//! request to `evil.example`, and `http://127.0.0.1:<port>7/` one to
//! another port; read as the proxy by a string prefix, either would
//! reach the player as written and be fetched past the proxy.

use proptest::prelude::*;
use url::Url;

use super::{reach, Reach};
use crate::proxy::m3u8::ProxyOrigin;

/// A proxy whose port is a prefix of another valid port.
fn origin() -> ProxyOrigin {
    ProxyOrigin::new("127.0.0.1", 4233)
}

fn reach_of(uri: &str) -> Reach {
    reach(&Url::parse(uri).expect("url"), &origin(), true)
}

#[test]
fn a_uri_dressed_as_the_proxy_by_its_userinfo_is_proxied() {
    for uri in [
        "http://127.0.0.1:4233@evil.example/v.m3u8",
        "http://127.0.0.1:4233@127.0.0.1:8080/v.m3u8",
        "http://127.0.0.1:4233:pw@evil.example/v.m3u8",
    ] {
        assert_eq!(reach_of(uri), Reach::Proxied, "{uri}");
    }
}

#[test]
fn a_uri_on_a_neighbouring_port_is_proxied() {
    for uri in [
        "http://127.0.0.1:42337/v.m3u8",
        "http://127.0.0.1:4234/v.m3u8",
        "https://127.0.0.1:4233/v.m3u8",
        "http://localhost:4233/v.m3u8",
    ] {
        assert_eq!(reach_of(uri), Reach::Proxied, "{uri}");
    }
}

#[test]
fn a_uri_on_the_proxy_is_kept_as_written() {
    assert_eq!(
        reach_of("http://127.0.0.1:4233/s/x/seg?u=a&t=b"),
        Reach::AsWritten
    );
}

/// Whether `url` is on the proxy's own scheme, host and port.
fn on_proxy(url: &Url) -> bool {
    url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port_or_known_default() == Some(4233)
}

proptest! {
    /// Built from parts, a network URI is kept as written exactly when
    /// its scheme, host and port are the proxy's, whatever userinfo it
    /// carries; every other one is proxied.
    #[test]
    fn only_the_proxys_scheme_host_and_port_are_kept_as_written(
        scheme in "https?",
        userinfo in proptest::option::of("(u|u:p|127\\.0\\.0\\.1:4233|127\\.0\\.0\\.1)@"),
        host in "(127\\.0\\.0\\.1|127\\.0\\.0\\.2|localhost|evil\\.example|\\[::1\\]|0\\.0\\.0\\.0)",
        port in proptest::option::of(prop_oneof![Just(4233u16), Just(42337), Just(4234), Just(80), 1u16..]),
        path in "[a-z/]{0,10}",
    ) {
        let port = port.map(|p| format!(":{p}")).unwrap_or_default();
        let uri = format!("{scheme}://{}{host}{port}/{path}", userinfo.unwrap_or_default());
        let url = Url::parse(&uri).expect("url");
        let expected = if on_proxy(&url) { Reach::AsWritten } else { Reach::Proxied };
        prop_assert_eq!(reach(&url, &origin(), true), expected, "{}", uri);
    }

    /// A URI whose text starts with the proxy's origin is kept as
    /// written only when it parses to the proxy's scheme, host and port.
    #[test]
    fn the_proxys_origin_as_a_prefix_is_not_the_proxy(
        tail in "([0-9]|@evil\\.example|@127\\.0\\.0\\.1:[0-9]{1,4}|:pw@evil\\.example|\\.evil\\.example)?(/[a-z]{0,6})?",
    ) {
        let uri = format!("{}{tail}", origin().base);
        let Ok(url) = Url::parse(&uri) else {
            return Ok(());
        };
        let expected = if on_proxy(&url) { Reach::AsWritten } else { Reach::Proxied };
        prop_assert_eq!(reach(&url, &origin(), true), expected, "{}", uri);
    }
}
