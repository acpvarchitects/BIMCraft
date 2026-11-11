//! FE-4: what is selected in a family's plan or elevation copied by a Ctrl+drag, as Revit copies it. Each copy is a new
//! element: a form with a new GUID and a new GUID for every line of its sketch, top and profiles (its formulas as they are),
//! a reference plane or line, a symbol, a connector and a nested instance (the same type) each with a new GUID, ids left
//! to be given. What a copy is drawn on is the copy of it where that is copied too, else what the original is drawn on. A
//! dimension is copied where every reference of it is copied, its references the copies: one selected, or one that holds a
//! constraint (a label, which keeps its parameter, a lock, EQ, Align's lock) among what is copied; one that reaches past the
//! copies isn't, so a lock to what isn't copied is dropped, as Revit drops it. Parameters aren't copied. A datum and a plane
//! that defines the origin aren't copied ([`CopyRefused`]). The copies are made where the originals are: a move
//! ([`Family::move_by`]) then puts them where they go.

use super::*;
use crate::family::nested::NestedInstance;

/// What a copy takes: the family's own reference planes and its reference lines by GUID, its forms, symbols, connectors and
/// nested instances by place, its dimensions selected by GUID.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Copying {
    pub planes: Vec<Guid>,
    pub lines: Vec<Guid>,
    pub forms: Vec<usize>,
    pub symbols: Vec<usize>,
    pub connectors: Vec<usize>,
    pub nested: Vec<usize>,
    pub dimensions: Vec<Guid>,
}

/// Why a copy is refused, naming the plane (by its name, else `reference plane 2`).
#[derive(Clone, Debug, PartialEq)]
pub enum CopyRefused {
    /// A datum: Ref. Level and the two Center planes are the family's own, one each.
    Datum(String),
    /// A plane that defines the origin, which one plane does in its direction.
    Origin(String),
}

/// The copies, made where the originals are, each list in the order of what it copies: to be added after the family's own.
/// `selected`, the dimensions among them copied because they were selected (the rest hold constraints among the copies);
/// `guids`, each original's GUID (an element's or a sketch line's) and its copy's.
#[derive(Clone, Debug, Default)]
pub struct Copies {
    pub planes: Vec<RefPlane>,
    pub lines: Vec<RefLine>,
    pub forms: Vec<Form>,
    pub symbols: Vec<Symbol>,
    pub connectors: Vec<Connector>,
    pub nested: Vec<NestedInstance>,
    pub dimensions: Vec<FamilyDimension>,
    pub selected: Vec<Guid>,
    pub guids: BTreeMap<Guid, Guid>,
}

impl Copies {
    /// What `host` names for a copy: the copy of it where it is copied, else itself.
    fn host(&self, host: PlaneRef) -> PlaneRef {
        match host {
            PlaneRef::Plane(g) => PlaneRef::Plane(self.guids.get(&g).copied().unwrap_or(g)),
            PlaneRef::Line(g, face) => PlaneRef::Line(self.guids.get(&g).copied().unwrap_or(g), face),
        }
    }

    /// A new GUID for `g`, remembered (a nil one stays nil: nothing names it).
    fn renew(&mut self, g: Guid) -> Guid {
        if g.is_nil() {
            return g;
        }
        let new = Guid::new();
        self.guids.insert(g, new);
        new
    }

    /// `r` naming the copies, where every GUID it names is copied.
    fn reference(&self, r: DimRef) -> Option<DimRef> {
        let m = |g: Guid| self.guids.get(&g).copied();
        Some(match r {
            DimRef::Plane(g) => DimRef::Plane(m(g)?),
            DimRef::Form(g, e) => DimRef::Form(m(g)?, e),
            DimRef::Sketch(f, l) => DimRef::Sketch(m(f)?, m(l)?),
            DimRef::Point(f, l, s) => DimRef::Point(m(f)?, m(l)?, s),
            DimRef::Line(g, face) => DimRef::Line(m(g)?, face),
            DimRef::LinePoint(g, s) => DimRef::LinePoint(m(g)?, s),
        })
    }
}

impl Family {
    /// The copies of what `copying` takes, made where the originals are; or why not.
    pub fn copies(&self, copying: &Copying) -> Result<Copies, CopyRefused> {
        let mut out = Copies::default();
        // In the family's order, so a line drawn on a line copied finds its host's copy made already.
        for p in self.planes.iter().filter(|p| copying.planes.contains(&p.guid)) {
            let said = super::moving::plane_said(self, p.guid);
            if p.datum() {
                return Err(CopyRefused::Datum(said));
            }
            if p.defines_origin {
                return Err(CopyRefused::Origin(said));
            }
        }
        if let Some(g) = copying.planes.iter().find(|g| !self.planes.iter().any(|p| p.guid == **g)) {
            // A datum the family doesn't list (a file of 17's): still a datum.
            return Err(CopyRefused::Datum(super::moving::plane_said(self, *g)));
        }
        for p in self.planes.iter().filter(|p| copying.planes.contains(&p.guid)) {
            let mut c = p.clone();
            c.id = 0;
            c.guid = out.renew(p.guid);
            // A name and a named reference (Left through Top) are one plane's: the copy has neither, as Revit's.
            c.name.clear();
            if c.reference.named() {
                c.reference = Reference::Weak;
            }
            out.planes.push(c);
        }
        for l in self.lines.iter().filter(|l| copying.lines.contains(&l.guid)) {
            let mut c = l.clone();
            c.id = 0;
            c.guid = out.renew(l.guid);
            c.host = out.host(l.host);
            if c.reference.named() {
                c.reference = Reference::Weak;
            }
            out.lines.push(c);
        }
        for &i in &copying.forms {
            let Some(f) = self.forms.get(i) else { continue };
            let mut c = f.clone();
            c.id = 0;
            c.guid = out.renew(f.guid);
            c.host = out.host(f.host);
            for l in c.all_lines_mut() {
                l.id = 0;
                l.guid = out.renew(l.guid);
            }
            out.forms.push(c);
        }
        for &i in &copying.symbols {
            let Some(s) = self.symbols.get(i) else { continue };
            out.symbols.push(Symbol { id: 0, guid: if s.guid.is_nil() { s.guid } else { Guid::new() }, ..s.clone() });
        }
        for &i in &copying.connectors {
            let Some(c) = self.connectors.get(i) else { continue };
            out.connectors.push(Connector { id: 0, guid: if c.guid.is_nil() { c.guid } else { Guid::new() }, ..c.clone() });
        }
        for &i in &copying.nested {
            let Some(n) = self.nested_instances.get(i) else { continue };
            let host = out.host(n.host);
            out.nested.push(NestedInstance { id: 0, guid: if n.guid.is_nil() { n.guid } else { Guid::new() }, host, ..n.clone() });
        }
        for d in &self.dimensions {
            let selected = copying.dimensions.contains(&d.guid);
            let holds = d.label.is_some() || !d.locked.is_empty() || d.equal || d.alignment;
            if !(selected || holds) {
                continue;
            }
            let Some(refs) = d.refs.iter().map(|r| out.reference(*r)).collect::<Option<Vec<_>>>() else { continue };
            let guid = Guid::new();
            if selected {
                out.selected.push(guid);
            }
            out.dimensions.push(FamilyDimension { id: 0, guid, refs, ..d.clone() });
        }
        Ok(out)
    }
}
