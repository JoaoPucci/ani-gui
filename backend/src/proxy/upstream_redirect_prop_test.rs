//! Properties of [`redirect_target`]: which responses send a fetch on,
//! and where. Mounted by `#[path]` beside the module.

use super::redirect_target;
use proptest::prelude::*;
use reqwest::StatusCode;
use url::Url;

const REDIRECTS: [u16; 5] = [301, 302, 303, 307, 308];

proptest! {
    /// Only the five redirect statuses send the fetch on; any other
    /// status is the answer, whatever Location it carries.
    #[test]
    fn only_a_redirect_status_sends_the_fetch_on(
        status in 100u16..600,
        location in proptest::option::of("/[a-z0-9/._-]{0,30}"),
    ) {
        let from = Url::parse("https://cdn.example:8443/dir/master.m3u8").expect("url");
        let status = StatusCode::from_u16(status).expect("status");
        let target = redirect_target(status, location.as_deref(), &from);
        if REDIRECTS.contains(&status.as_u16()) && location.is_some() {
            prop_assert!(target.is_some());
        } else {
            prop_assert_eq!(target, None);
        }
    }

    /// A relative Location resolves against the URL that answered:
    /// the hop stays on that host and port.
    #[test]
    fn a_relative_location_stays_on_the_host_that_answered(
        status in prop::sample::select(REDIRECTS.to_vec()),
        host in "[a-z][a-z0-9]{0,12}(\\.[a-z][a-z0-9]{0,12}){0,3}",
        port in 1u16..=65535,
        // A segment that is a dot or two is a step, not a name.
        path in "[a-z0-9][a-z0-9._-]{0,19}",
    ) {
        let from = Url::parse(&format!("https://{host}:{port}/dir/master.m3u8")).expect("url");
        let status = StatusCode::from_u16(status).expect("status");
        let target = redirect_target(status, Some(&path), &from).expect("resolves");
        prop_assert_eq!(target.host_str(), Some(host.as_str()));
        prop_assert_eq!(target.port_or_known_default(), Some(port));
        prop_assert_eq!(target.path(), format!("/dir/{path}"));
    }

    /// An absolute Location is where the hop goes, whatever host
    /// answered.
    #[test]
    fn an_absolute_location_is_the_hop(
        status in prop::sample::select(REDIRECTS.to_vec()),
        host in "[a-z][a-z0-9]{0,12}(\\.[a-z][a-z0-9]{0,12}){0,3}",
        path in "/[a-z0-9/._-]{0,30}",
    ) {
        let from = Url::parse("https://cdn.example/dir/master.m3u8").expect("url");
        let status = StatusCode::from_u16(status).expect("status");
        let location = format!("https://{host}{path}");
        let target = redirect_target(status, Some(&location), &from).expect("resolves");
        let expected = Url::parse(&location).expect("url");
        prop_assert_eq!(target.as_str(), expected.as_str());
    }
}
