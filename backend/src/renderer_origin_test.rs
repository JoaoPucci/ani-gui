//! Which `Origin` values the renderer's CORS answers.

use super::*;
use proptest::prelude::*;

#[test]
fn the_packaged_origin_is_the_renderers() {
    assert!(is_renderer_origin(b"app://localhost"));
}

#[test]
fn the_dev_servers_origins_are_the_renderers() {
    for origin in [
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "http://[::1]:5173",
        "http://localhost",
        "http://localhost:65535",
    ] {
        assert!(is_renderer_origin(origin.as_bytes()), "{origin}");
    }
}

#[test]
fn other_origins_are_not() {
    for origin in [
        "",
        "null",
        "file://",
        "app://localhost:1",
        "app://localhost.evil.example",
        "app://evil.example",
        "APP://localhost",
        "https://localhost:5173",
        "https://evil.example",
        "http://evil.example",
        "http://localhost.evil.example",
        "http://localhost:5173.evil.example",
        "http://127.0.0.1.evil.example",
        "http://127.0.0.2:5173",
        "http://user@localhost:5173",
        "http://localhost:5173/",
        "http://localhost:",
        "http://localhost:65536",
        "http://localhost:123456",
        "http://localhost:+80",
        "http://[::1]x",
        " app://localhost",
        "app://localhost ",
    ] {
        assert!(!is_renderer_origin(origin.as_bytes()), "{origin:?}");
    }
}

#[test]
fn bytes_that_are_not_utf8_are_not_an_origin() {
    assert!(!is_renderer_origin(&[0xff, 0xfe]));
}

proptest! {
    /// Every port a dev server can listen on, on each loopback name.
    #[test]
    fn any_port_on_a_loopback_name_is_the_renderers(
        port in 0u16..=u16::MAX,
        host in prop::sample::select(vec!["localhost", "127.0.0.1", "[::1]"]),
    ) {
        let origin = format!("http://{host}:{port}");
        prop_assert!(is_renderer_origin(origin.as_bytes()));
    }

    /// Anything after an accepted origin makes it another origin.
    #[test]
    fn a_suffix_on_an_accepted_origin_is_refused(
        base in prop::sample::select(vec![
            "app://localhost",
            "http://localhost",
            "http://localhost:5173",
            "http://127.0.0.1:5173",
            "http://[::1]:5173",
        ]),
        suffix in "[^0-9]{1,12}",
    ) {
        let origin = format!("{base}{suffix}");
        prop_assert!(!is_renderer_origin(origin.as_bytes()), "{}", origin);
    }

    /// No host but the loopback names, under any scheme.
    #[test]
    fn another_host_is_refused(
        scheme in prop::sample::select(vec!["http", "https", "app"]),
        host in "[a-z0-9.-]{1,24}",
        port in prop::option::of(0u16..=u16::MAX),
    ) {
        prop_assume!(!["localhost", "127.0.0.1"].contains(&host.as_str()));
        let origin = match port {
            Some(p) => format!("{scheme}://{host}:{p}"),
            None => format!("{scheme}://{host}"),
        };
        prop_assert!(!is_renderer_origin(origin.as_bytes()), "{}", origin);
    }

    /// Arbitrary bytes never panic, and anything accepted is one of the
    /// shapes above.
    #[test]
    fn arbitrary_bytes_are_accepted_only_in_a_known_shape(
        bytes in prop::collection::vec(any::<u8>(), 0..40),
    ) {
        if is_renderer_origin(&bytes) {
            let s = std::str::from_utf8(&bytes).expect("accepted origins are utf-8");
            prop_assert!(
                s == PACKAGED
                    || s.starts_with("http://localhost")
                    || s.starts_with("http://127.0.0.1")
                    || s.starts_with("http://[::1]"),
                "{}", s
            );
        }
    }
}
