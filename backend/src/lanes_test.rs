use super::*;
use proptest::prelude::*;

fn bases(ports: &[u16]) -> Vec<(Lane, String)> {
    Lane::ALL
        .iter()
        .zip(ports)
        .map(|(lane, port)| (*lane, format!("http://127.0.0.1:{port}")))
        .collect()
}

#[test]
fn lanes_come_before_the_two_lines_the_handshake_ends_with() {
    let lines = handshake_lines(&bases(&[1, 2, 3, 4, 5, 6]), "ab");
    assert_eq!(
        lines,
        vec![
            "ANI_GUI_LANE stream http://127.0.0.1:2",
            "ANI_GUI_LANE image http://127.0.0.1:3",
            "ANI_GUI_LANE probe http://127.0.0.1:4",
            "ANI_GUI_LANE resolve http://127.0.0.1:5",
            "ANI_GUI_LANE download http://127.0.0.1:6",
            "ANI_GUI_LISTENING http://127.0.0.1:1",
            "ANI_GUI_INTERNAL_SECRET ab",
        ]
    );
}

#[test]
fn lane_names_are_distinct() {
    let names: std::collections::BTreeSet<&str> = Lane::ALL.iter().map(|l| l.name()).collect();
    assert_eq!(names.len(), Lane::ALL.len());
}

#[tokio::test]
async fn every_lane_binds_a_port_of_its_own_on_the_loopback() {
    let lanes = Lanes::bind().await.expect("bind");
    let mut ports = std::collections::BTreeSet::new();
    for lane in Lane::ALL {
        let addr = lanes.addr(lane);
        assert!(addr.ip().is_loopback(), "{lane:?} at {addr}");
        ports.insert(addr.port());
    }
    assert_eq!(ports.len(), Lane::ALL.len());
    assert_eq!(lanes.bases().len(), Lane::ALL.len());
    assert_eq!(lanes.into_listeners().len(), Lane::ALL.len());
}

proptest! {
    /// Whatever the ports, the handshake announces every lane once,
    /// ends with the API's line and then the secret, and puts no lane
    /// after them.
    #[test]
    fn the_handshake_ends_with_the_api_and_the_secret(ports in proptest::collection::vec(1u16.., 6)) {
        let all = bases(&ports);
        let lines = handshake_lines(&all, "s");
        prop_assert_eq!(lines.len(), Lane::ALL.len() + 1);
        prop_assert_eq!(lines.last().map(String::as_str), Some("ANI_GUI_INTERNAL_SECRET s"));
        prop_assert_eq!(&lines[lines.len() - 2], &format!("ANI_GUI_LISTENING {}", all[0].1));
        for (lane, base) in &all[1..] {
            let line = format!("ANI_GUI_LANE {} {base}", lane.name());
            prop_assert_eq!(lines.iter().filter(|l| **l == line).count(), 1);
        }
    }
}
