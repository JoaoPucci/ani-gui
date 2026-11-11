//! The sections around a run, before they are told apart, and the
//! rules that move an entry between them by its own format: a movie is
//! a film whatever role placed it, and each id lands in one section.

use std::collections::BTreeSet;

use super::rules::Run;
use super::Lookup;

/// Section ids before they are told apart.
#[derive(Default)]
pub(super) struct Sections {
    pub(super) versions: BTreeSet<String>,
    pub(super) recaps: BTreeSet<String>,
    pub(super) films: BTreeSet<String>,
    pub(super) sides: BTreeSet<String>,
}

/// Whether what the walk knows of `id` says it is a movie.
pub(super) fn is_movie(id: &str, run: &Run, look: &Lookup<'_>) -> bool {
    look.entry(id, &run.consulted)
        .is_some_and(|e| e.subtype.as_deref() == Some("movie"))
}

impl Sections {
    /// Move every movie among the versions, recaps and sides to the
    /// films.
    pub(super) fn movies_to_films(&mut self, movie: impl Fn(&str) -> bool) {
        for set in [&mut self.versions, &mut self.recaps, &mut self.sides] {
            let films: Vec<String> = set.iter().filter(|id| movie(id)).cloned().collect();
            for id in films {
                set.remove(&id);
                self.films.insert(id);
            }
        }
    }

    /// Keep each id in its first section: versions, recaps, films,
    /// sides.
    pub(super) fn one_section_each(&mut self) {
        let (versions, recaps) = (&self.versions, &mut self.recaps);
        recaps.retain(|id| !versions.contains(id));
        let recaps = &self.recaps;
        self.films
            .retain(|id| !versions.contains(id) && !recaps.contains(id));
        let films = &self.films;
        self.sides
            .retain(|id| !versions.contains(id) && !recaps.contains(id) && !films.contains(id));
    }

    /// An entry of the anchor's own run of one-offs outside the run the
    /// walk moved to: where the run's listings placed it, else among the
    /// films when it is a movie, else the sides.
    pub(super) fn place_beside(&mut self, id: &str, film: bool) {
        let placed = [&self.versions, &self.recaps, &self.films, &self.sides]
            .iter()
            .any(|set| set.contains(id));
        if placed {
            return;
        }
        let to = if film {
            &mut self.films
        } else {
            &mut self.sides
        };
        to.insert(id.to_string());
    }

    /// A side-story anchor outside the run: among the films when it is
    /// a movie, else the sides, and nowhere else.
    pub(super) fn place_side_anchor(&mut self, anchor: &str, film: bool) {
        for set in [
            &mut self.versions,
            &mut self.recaps,
            &mut self.films,
            &mut self.sides,
        ] {
            set.remove(anchor);
        }
        let to = if film {
            &mut self.films
        } else {
            &mut self.sides
        };
        to.insert(anchor.to_string());
    }
}
