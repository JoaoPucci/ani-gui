//! The relationships reader against Kitsu's recorded answers.

use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/hianime/appended")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn relation(role: &str, kind: &str, id: &str, subtype: &str, count: Option<u32>) -> KitsuRelation {
    KitsuRelation {
        role: role.into(),
        kind: kind.into(),
        id: id.into(),
        subtype: Some(subtype.into()),
        status: Some("finished".into()),
        episode_count: count,
    }
}

/// Noragami relates its sequel, its OVA, a music video and the manga,
/// each with the destination's subtype, status and count.
#[test]
fn noragami_s_relations_read_as_kitsu_lists_them() {
    let got = parse_relationships_response(&fixture("kitsu-relations-7881.json")).expect("parsed");
    assert!(got.complete);
    assert_eq!(
        got.entries,
        vec![
            relation("sequel", "anime", "10800", "TV", Some(13)),
            relation("side_story", "anime", "8152", "OVA", Some(2)),
            relation("other", "anime", "11843", "music", Some(1)),
            relation("adaptation", "manga", "17204", "manga", None),
        ]
    );
}

#[test]
fn satsuriku_no_tenshi_relates_its_four_episode_ona() {
    let got = parse_relationships_response(&fixture("kitsu-relations-13715.json")).expect("parsed");
    assert!(got.complete);
    assert!(got
        .entries
        .contains(&relation("sequel", "anime", "41963", "ONA", Some(4))));
}

/// A page holding fewer relations than Kitsu counts is read in part;
/// one without a count is not known to be whole; a relation whose
/// destination names nothing is dropped.
#[test]
fn a_page_short_of_kitsu_s_count_is_read_in_part() {
    let mut doc: serde_json::Value =
        serde_json::from_slice(&fixture("kitsu-relations-7881.json")).expect("json");
    doc["meta"]["count"] = serde_json::json!(25);
    let got = parse_relationships_response(doc.to_string().as_bytes()).expect("parsed");
    assert!(!got.complete);
    doc.as_object_mut().expect("doc").remove("meta");
    let got = parse_relationships_response(doc.to_string().as_bytes()).expect("parsed");
    assert!(!got.complete);
    doc["data"][0]["relationships"]["destination"]["data"] = serde_json::Value::Null;
    let got = parse_relationships_response(doc.to_string().as_bytes()).expect("parsed");
    assert_eq!(got.entries.len(), 3);
}

#[test]
fn a_body_that_is_not_the_document_is_a_parse_failure() {
    assert!(matches!(
        parse_relationships_response(b"<html>"),
        Err(AniError::ParseFailed { .. })
    ));
}

#[tokio::test]
async fn the_client_asks_for_the_destinations_it_reads() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/7881/media-relationships"))
        .and(query_param("include", "destination"))
        .and(query_param("fields[anime]", "subtype,status,episodeCount"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(fixture("kitsu-relations-7881.json")),
        )
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/1/media-relationships"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&kitsu)
        .await;
    let client = KitsuClient::with_base(reqwest::Client::new(), kitsu.uri());
    let got = client.media_relationships("7881").await.expect("answered");
    assert_eq!(got.entries.len(), 4);
    assert!(matches!(
        client.media_relationships("1").await,
        Err(AniError::Upstream { status: 503 })
    ));
}

/// The bodies the episode map's recorded cases came from: Ranma ½'s
/// one sequel on Kitsu is its OVA (Nettou-hen has no Kitsu entry), and
/// Tower of God's second season is chained between its first and
/// Koubou-sen.
#[test]
fn the_tracker_map_s_recorded_relations_read_their_single_sequels() {
    let read = |name: &str| {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repo root")
            .join("tests/fixtures/tracker-map/wire")
            .join(name);
        parse_relationships_response(&std::fs::read(&path).expect("fixture")).expect("parsed")
    };
    let sequels = |r: &KitsuRelations| -> Vec<String> {
        r.entries
            .iter()
            .filter(|e| e.role == "sequel" && e.kind == "anime")
            .map(|e| e.id.clone())
            .collect()
    };
    let ranma = read("kitsu-relationships-186.json");
    assert_eq!(sequels(&ranma), ["897"]);
    let tower = read("kitsu-relationships-46312.json");
    assert!(tower.complete);
    assert_eq!(sequels(&tower), ["49244"]);
    assert!(tower
        .entries
        .iter()
        .any(|e| e.role == "prequel" && e.id == "43004" && e.episode_count == Some(13)));
}
