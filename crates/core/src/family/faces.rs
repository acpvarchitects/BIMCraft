//! GEO-2b: which face of its form each triangle of a built piece is, keyed by what the form was drawn with, so a face is
//! named the same across flexes and edits that keep the sketch's lines: a cap ([`FormFace::Start`], [`FormFace::End`]),
//! the side a profile line sweeps ([`FormFace::Side`], by the line's GUID; an arc or a circle is one face, all its chords
//! the line's), the side a blend's top line alone makes ([`FormFace::TopSide`]), an edge of a profile whose lines have no
//! GUID ([`FormFace::Edge`], by the line's place), a face a void or a solid cut into it ([`FormFace::Cut`], by the cutter's
//! GUID and its own face), and anything else ([`FormFace::All`]: a mesh form). Nothing here is kept in a file: the tags are
//! worked out with the triangles ([`super::Piece::faces`]) and never change them.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::geom::{P2, Region};
use crate::ids::Guid;

/// A face of a form, as its triangles are tagged. In JSON: `"start"`, `"end"`, `"all"`, `{"side": L}`,
/// `{"side": L, "along": P}`, `{"top_side": T}`, `{"edge": i}`, `{"cut": G, "face": …}`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FormFace {
    /// The cap at its start: an extrusion's `from` end, a blend's base, a sweep's or a swept blend's path start, a part
    /// revolve's first end.
    Start,
    /// The cap at its end: an extrusion's `to` end, a blend's top, a path's end, a part revolve's last end.
    End,
    /// The whole form: a mesh's, or a face nothing else names.
    All,
    /// The side swept from profile line `line`; a Sweep's (and a Swept Blend's) as it goes `along` a line of its path.
    Side { line: Guid, along: Option<Guid> },
    /// A blend's side that comes from a line of its top alone (the base's corner held while the top's line passes).
    TopSide(Guid),
    /// The side swept from the profile line at this place in the profile, where the lines have no GUID.
    Edge(usize),
    /// A face that form `by` (a void, or a solid joined to it) made cutting this one: `face`, that form's own.
    Cut { by: Guid, face: Box<FormFace> },
}

impl FormFace {
    /// The side profile line `line` sweeps.
    pub fn side(line: Guid) -> FormFace {
        FormFace::Side { line, along: None }
    }

    /// This side as it goes along path line `path`: a side keeps its line and takes the path's; anything else stays.
    pub fn along(self, path: Option<Guid>) -> FormFace {
        match (self, path) {
            (FormFace::Side { line, along: None }, Some(p)) => FormFace::Side { line, along: Some(p) },
            (k, _) => k,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Named {
    Start,
    End,
    All,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SideRaw {
    side: Guid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    along: Option<Guid>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TopRaw {
    top_side: Guid,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeRaw {
    edge: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CutRaw {
    cut: Guid,
    face: Box<FormFace>,
}

/// A face key as JSON has it: a name, or one object of its own keys.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Raw {
    Named(Named),
    Side(SideRaw),
    Top(TopRaw),
    Edge(EdgeRaw),
    Cut(CutRaw),
}

impl Serialize for FormFace {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let raw = match self.clone() {
            FormFace::Start => Raw::Named(Named::Start),
            FormFace::End => Raw::Named(Named::End),
            FormFace::All => Raw::Named(Named::All),
            FormFace::Side { line, along } => Raw::Side(SideRaw { side: line, along }),
            FormFace::TopSide(top_side) => Raw::Top(TopRaw { top_side }),
            FormFace::Edge(edge) => Raw::Edge(EdgeRaw { edge }),
            FormFace::Cut { by, face } => Raw::Cut(CutRaw { cut: by, face }),
        };
        raw.serialize(s)
    }
}

impl<'de> Deserialize<'de> for FormFace {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<FormFace, D::Error> {
        Ok(match Raw::deserialize(d)? {
            Raw::Named(Named::Start) => FormFace::Start,
            Raw::Named(Named::End) => FormFace::End,
            Raw::Named(Named::All) => FormFace::All,
            Raw::Side(SideRaw { side, along }) => FormFace::Side { line: side, along },
            Raw::Top(TopRaw { top_side }) => FormFace::TopSide(top_side),
            Raw::Edge(EdgeRaw { edge }) => FormFace::Edge(edge),
            Raw::Cut(CutRaw { cut, face }) => FormFace::Cut { by: cut, face },
        })
    }
}

/// A piece's face keys, each once, numbered as its triangles' tags index them.
#[derive(Clone, Debug, Default)]
pub struct Table {
    pub keys: Vec<FormFace>,
    at: HashMap<FormFace, u32>,
}

impl Table {
    /// The table of `keys` (each once already).
    pub fn of(keys: Vec<FormFace>) -> Table {
        let at = keys.iter().enumerate().map(|(i, k)| (k.clone(), i as u32)).collect();
        Table { keys, at }
    }

    /// `key`'s number, given it the first time.
    pub fn id(&mut self, key: FormFace) -> u32 {
        if let Some(i) = self.at.get(&key) {
            return *i;
        }
        let i = self.keys.len() as u32;
        self.at.insert(key.clone(), i);
        self.keys.push(key);
        i
    }

    /// Each edge of each ring of `edges` numbered; a ring or an edge `edges` lacks, [`FormFace::All`]. `rings` is how many
    /// edges each ring has, in the order the builders walk them.
    pub fn edges(&mut self, edges: &[Vec<FormFace>], rings: impl Iterator<Item = usize>) -> Vec<Vec<u32>> {
        rings
            .enumerate()
            .map(|(r, n)| (0..n).map(|e| self.id(edges.get(r).and_then(|x| x.get(e)).cloned().unwrap_or(FormFace::All))).collect())
            .collect()
    }
}

/// The lines a profile was drawn with, each its key and the points it is drawn through (an arc's chords), to find which
/// one an edge of the loops they make lies along.
#[derive(Clone, Debug, Default)]
pub struct Traced {
    lines: Vec<(FormFace, Vec<P2>, [P2; 2])>,
}

impl Traced {
    pub fn new(lines: impl IntoIterator<Item = (FormFace, Vec<P2>)>) -> Traced {
        let grown = crate::model::TOL;
        let lines = lines
            .into_iter()
            .map(|(k, pts)| {
                let lo = pts.iter().fold(P2(f64::INFINITY, f64::INFINITY), |a, p| P2(a.0.min(p.0), a.1.min(p.1)));
                let hi = pts.iter().fold(P2(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| P2(a.0.max(p.0), a.1.max(p.1)));
                (k, pts, [P2(lo.0 - grown, lo.1 - grown), P2(hi.0 + grown, hi.1 + grown)])
            })
            .collect();
        Traced { lines }
    }

    /// The key of the line the edge from `a` to `b` lies along: the first that has it; [`FormFace::All`] where none does.
    pub fn key(&self, a: P2, b: P2) -> FormFace {
        let inside = |p: P2, [lo, hi]: &[P2; 2]| p.0 >= lo.0 && p.0 <= hi.0 && p.1 >= lo.1 && p.1 <= hi.1;
        self.lines
            .iter()
            .find(|(_, pts, bx)| inside(a, bx) && inside(b, bx) && crate::sketch::along(pts, a, b))
            .map_or(FormFace::All, |(k, _, _)| k.clone())
    }

    /// Each edge of each ring of `regions` (each outline, then its holes), keyed.
    pub fn rings(&self, regions: &[Region]) -> Vec<Vec<FormFace>> {
        regions
            .iter()
            .flat_map(|r| std::iter::once(&r.outer).chain(&r.holes))
            .map(|ring| crate::geom::ring_edges(ring).map(|(a, b)| self.key(a, b)).collect())
            .collect()
    }
}

/// The key of a profile's line at place `i`: its side by its GUID, or by its place where it has none.
pub fn line_key(i: usize, guid: Guid) -> FormFace {
    if guid.is_nil() { FormFace::Edge(i) } else { FormFace::side(guid) }
}
