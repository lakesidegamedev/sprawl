//! Save sections for features added after save format 5.
//!
//! Each feature keeps its own state under a four-byte tag, written after the main body. A save
//! without a section (an older save, or a game where the feature never ran) leaves the feature
//! at its defaults, so adding a feature never needs a new save format. A tag this build does not
//! know is skipped.
//!
//! To add a feature: pick a new tag, append one `push` line to `sections` and one arm to
//! `load_section`, and write the two functions in the feature's own module. Never reuse or
//! change the meaning of a tag once released; add a new tag instead.

use crate::Sim;
use crate::save::{R, Res, W};

/// Every section of this game, in a fixed order.
pub(crate) fn sections(s: &Sim) -> Vec<([u8; 4], Vec<u8>)> {
    let mut out: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    let mut push = |tag: &[u8; 4], f: &dyn Fn(&Sim, &mut W)| {
        let mut w = W(Vec::new());
        f(s, &mut w);
        out.push((*tag, w.0));
    };
    // features add their sections here, one line each, in the order they were added:
    // push(b"EXMP", &|s, w| s.save_example(w));
    push(b"AVEN", &|s, w| s.save_avenues(w));
    let _ = &mut push;
    out
}

/// Read one section into the game. Unknown tags are skipped.
#[allow(clippy::match_single_binding)]
pub(crate) fn load_section(s: &mut Sim, tag: [u8; 4], bytes: &[u8]) -> Res<()> {
    let mut r = R::new(bytes);
    let r = &mut r;
    match &tag {
        // b"EXMP" => s.load_example(r)?,
        b"AVEN" => s.load_avenues(r)?,
        _ => {
            let _ = (s, r.left());
        }
    }
    Ok(())
}
