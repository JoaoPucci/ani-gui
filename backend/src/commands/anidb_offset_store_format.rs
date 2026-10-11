//! The offset store's file format — reading its rows and writing them
//! back; split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use super::*;

pub(in crate::commands::anidb_offset) fn parse(body: &str) -> Vec<Row> {
    body.lines()
        .filter_map(|line| {
            let mut cols = line.split('\t');
            let slug = cols.next()?;
            if slug.is_empty() {
                return None;
            }
            let offset = cols.next()?.trim().parse().ok()?;
            // Optional third+fourth columns; rows written before the
            // display stamp existed have two and parse the same.
            let display = match (cols.next(), cols.next()) {
                (Some(slot), Some(tag)) if !tag.is_empty() => {
                    slot.trim().parse().ok().map(|n| (n, tag.to_string()))
                }
                _ => None,
            };
            Some(Row {
                slug: slug.to_string(),
                offset,
                display,
            })
        })
        .collect()
}

pub(super) fn serialize(rows: &[Row]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&row.slug);
        out.push('\t');
        out.push_str(&row.offset.to_string());
        if let Some((slot, tag)) = &row.display {
            out.push('\t');
            out.push_str(&slot.to_string());
            out.push('\t');
            out.push_str(tag);
        }
        out.push('\n');
    }
    out
}
