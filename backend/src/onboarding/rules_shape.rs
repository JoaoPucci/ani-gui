//! Each tip's shape against its tool, its window, and the walkthrough
//! an entry tip names: the rest of the registry's rules
//! ([`super::rules::validate`]).

use super::decl::{find, Kind, Moment, Surface, TipDecl, MAX_STEPS};

pub(super) fn check_shape(tip: &TipDecl) -> Result<(), String> {
    let id = &tip.id;
    let steps = tip.places.len();
    let fits = match tip.kind {
        Kind::Single | Kind::Entry => steps == 1,
        Kind::Sequence => (1..=MAX_STEPS).contains(&steps),
    };
    if !fits {
        return Err(format!(
            "{id}: a {:?} tip cannot have {steps} steps",
            tip.kind
        ));
    }
    if tip.how_to.is_some() != (tip.kind == Kind::Entry) {
        return Err(format!(
            "{id}: an entry tip, and only one, names a walkthrough"
        ));
    }
    if tip.moment == Moment::StreamLoad
        && (tip.kind != Kind::Single || tip.surface != Surface::Play)
    {
        return Err(format!(
            "{id}: a stream-load tip is a single tip in the player"
        ));
    }
    Ok(())
}

pub(super) fn check_window(tip: &TipDecl) -> Result<(), String> {
    let id = &tip.id;
    if tip.flag.is_some() && tip.until.is_none() {
        return Err(format!("{id}: a flagged tip declares its hard end"));
    }
    match tip.until {
        Some(until) if until <= tip.since => Err(format!(
            "{id}: its window closes at {until}, before it opens"
        )),
        _ => Ok(()),
    }
}

pub(super) fn check_how_to(tip: &TipDecl, tips: &[TipDecl]) -> Result<(), String> {
    let Some(target) = &tip.how_to else {
        return Ok(());
    };
    let Some(guide) = find(tips, target) else {
        return Err(format!(
            "{}: its walkthrough {target} is not declared",
            tip.id
        ));
    };
    if guide.kind != Kind::Sequence || !guide.on_demand {
        return Err(format!(
            "{}: its walkthrough {target} is not an on-demand sequence",
            tip.id
        ));
    }
    if guide.surface == tip.surface {
        return Err(format!(
            "{}: its walkthrough runs where the feature lives, not on the entry's page",
            tip.id
        ));
    }
    Ok(())
}
