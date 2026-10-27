//! The age rating hianime's entry page gives the show — split from
//! `detail` so each file stays inside the complexity ratchet's per-file
//! bar.

use super::super::markup::decode_entities;

/// The age rating the entry page gives the show: the text of the
/// rating mark (`tick-pg`) in the stats under the show's own heading,
/// entity-decoded and trimmed — "G", "PG-13", "R+", "Rx". The show's
/// own block runs from its heading to the page's buttons that follow
/// it, and the stats are read inside that block alone, so a mark on
/// another show's card further down the page is never the show's.
/// `None` when the block carries no such mark, when the page carries no
/// buttons to end the block, and when the buttons come before the
/// stats: nothing then says where the show's own stats are.
#[must_use]
pub fn parse_detail_rating(html: &str) -> Option<String> {
    let (_, heading) = html.split_once("<h2 class=\"film-name")?;
    let (own, _) = heading.split_once("class=\"film-buttons\"")?;
    let (_, stats) = own.split_once("class=\"film-stats\"")?;
    let (_, mark) = stats.split_once("tick-item tick-pg\"")?;
    let text = mark.split_once('>')?.1.split('<').next()?;
    let text = decode_entities(text);
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}
