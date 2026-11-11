# Parts of elements and the heritage set

Read when touching `crates/core/src/parts.rs`, `parts/solid.rs`, `parts/state.rs`, the `create_parts`, `part` and `parts`
commands, or how a part goes to IFC. Aimed at the recording and restoration of historic buildings (HBIM): a wall is its
plaster and its masonry, and each is described, dated, measured and treated on its own.

## What a part is

A part (`Element::Part(BuildingPart)`, category `part`) is an element of its own, with an id and a GUID, that is a piece of
another: a wall, a floor, a roof, a ceiling or a component (`parts::HOSTS`), as Revit's Parts. It holds a **recipe, never a
shape**: its `host` (by GUID), its `source` (`Source::Layer(guid)`, a layer of the host's type by the layer's own GUID, or
`Source::Whole`, all of it), the part it was divided from (`parent`, none for one made straight from its host), a `material` of
its own in place of its layer's, IFC's kind of part (`predefined_type`: insulation, precastpanel, userdefined, notdefined, with
`object_type`, which a user-defined one must say), `excluded` (Revit's Exclude Parts: it stays, and is left out of what is
exported), the heritage set, and its values of the project's parameters (`params`; `part` is one of
`PARAMETER_CATEGORIES`). `Source` is an enum so that a division can be one more kind of source.

Everything else is **worked out from the host as it stands, each time it is asked for** (`parts/state.rs`), so nothing goes
stale: move the wall, stretch it, put a window in it, change its type's layer thicknesses, and its parts are what they are of
it now. `Model::part_solid(&BuildingPart) -> Result<Vec<[P3; 3]>, String>` is the solid: a closed surface of triangles in
**model coordinates** (mm, x east, y north, z up; not the host's axes), each counter-clockwise seen from outside, so its normal
points out. A wall's part is its layer between the layer's two faces (cut as the 3D view cuts a wall into layers: by the
distance from the location line, the outermost and innermost layers taking what lies past them at the ends), with the ends the
wall's joins give it, its top and base where what it is attached to puts them, and its doors and windows cut through. A
floor's, a roof's or a ceiling's is its layer between two planes, from the top down, less its openings and the shafts through
it; over a variable layer those above follow a shaped top and those below stay level. A component's is its family's forms.
The host's prisms (`walls::prisms`, `floor_prisms`, `roof_prisms`) are welded into one surface by `solid::mesh`: no face lies
inside it and every edge of a triangle is another's the other way round (`solid::closed`), which is what `csg::subtract` takes
and IFC calls a closed face set. `Model::part_prisms` gives the prisms themselves, for what would rather cut in plan.

`Model::part_quantities` reads the **net volume** and the **net area** off that solid, so openings, shafts, joins and, later,
divisions, gaps and offsets are in them with no code of their own. The volume is the solid's (`solid::volume`). The area is
that of the solid's faces that lie in the plane of the layer's own face and look the way it does (`solid::facing`,
`Model::part_facing`): for a wall's part the face toward the wall's exterior side (its elevation, less doors and windows), for
a floor's, a roof's or a ceiling's part its top (less openings and shafts; a sloped one's along its slope). A component's part
has a volume and no area. `part_layer`, `part_material` (its own, else its layer's, else its host's structure's),
`part_thickness`, `parts_of`, `part_divisions` and `part_state` (all of it at once) give the rest. `Material` holds a colour,
patterns, a texture and a transparency: no density or thermal values yet, so a part has none to show.

## Conflicts

When the recipe no longer applies, the part is **in conflict**: it stays in the model, `Model::check` passes, and
`Model::part_conflict` (and `conflict` in every listing) says why and what to do. The cases: the host's type has no layer of
that GUID (the type was swapped, or lost the layer; a type's layers keep their GUIDs by place when its list is given again);
the layer has no thickness; the host has no layers at all; the part it was divided from is in conflict; or it is all of an
element that has parts of its layers too, which would be counted twice. A part in conflict has no solid, measures nothing and
is not exported. `part {id, layer: n}` says which layer of the host's type it is now, which keeps what was said of it.
`Model::check` asks only what never depends on the host's state: a host of a kind that has parts, a parent that is another part
of the same host and none of its own divisions, a layer named by a real GUID, a material of the library, an `object_type` for a
user-defined part, a severity of 1 to 5, text within `MAX_TEXT`, and no second part of the same thing.

Deleting an element deletes its parts, as Revit deletes them with the original, and a part's divisions go with it; copying an
element copies them (and what was said of them); a part has no place of its own to move. A part with no phase or design
option of its own is in its host's (`phasing::value`), and `Session::stamp` gives a new part none.

## Commands

`create_parts {ids}` (Revit's Create Parts) makes a part for each layer with some thickness of each element's type, or one of
all of it where the type has no layers or the element is a component; a thermal layer's starts as INSULATION. An element that
has parts gets those it lacks; refused when none lacks any. `part {id | ids, …}` sets what parts say: `material`,
`predefined_type`, `object_type`, `excluded`, the heritage set (keys in any case) and `layer`. `parts {host?, ids?, conflicts?}`
lists them with what they hold and what they are now. `modify` and `elements` work on a part as on any element.

## The heritage set

Five values every part may have (`Heritage`), shown everywhere under `Heritage::NAMES`: `SU_Code` (the stratigraphic unit's
code), `Historical_Period` (in the project's own names), `Decay_Pathology` (`Decay`: one of a list of the words conservation
uses for forms of decay, or `Custom(text)`; the words only, never a standard's definitions), `Decay_Severity` (1 to 5) and
`Intervention_Type`. Anything else is a project parameter for parts.

## Read by name

`schedule::fields(m, "part")` lists a part's fields and `schedule::field_text`, `picks` and `check_rules` read them from
`parts::json` (what `elements` shows): `Id`, `Host`, `Host Category`, `Host Type`, `Level`, `Layer`, `Function`, `Material`,
`Thickness`, `Net Area`, `Net Volume`, `Predefined Type`, `Object Type`, `Excluded`, the five heritage names, `Conflict`,
`Count`, the phases and the design option. A value nobody gave is no value, never 0 (`From::Held`). `part` is in
`schedule::MORE`: there is no schedule of parts and no view filter of them yet. A part that parts were divided from lists them
(`divisions`) and no area or volume: they hold them.

## IFC

Each part that is not excluded and not in conflict (`parts::exported`) is an `IfcBuildingElementPart`: its GlobalId its GUID,
its name what it is made of (else its host's type), its `ObjectType` and `PredefinedType` as it says, placed relative to its
host's placement with its body a closed `IfcTriangulatedFaceSet` in the host's axes, associated with its `IfcMaterial`, with
`BIMCraft_PartQuantities` (`Width`, `NetArea`, `NetVolume`) and, for what was said, `BIMCraft_Heritage`. Its host is the sum
of its parts (`IfcRelAggregates`, the same relation that holds a host's bars), and so is a part of its divisions. **What is
decomposed has no `Body` of its own** (a wall keeps its `Axis`), as a stair has none beside its flights: IFC asks it, and a
body on both would be drawn and measured twice. Parts are in no storey but through their host. An element with no part
written (all excluded or in conflict) keeps its body. **Reading parts back from IFC is not done**, and BIMCraft's own import
does not yet read a decomposed element's body from its parts: it leaves such a wall out as one with no body it can read (its
doors and windows with it), and says so in `skipped`. After touching this, export a project with parts and check it as
[looking at the result](looking-at-results.md) says: `ci/validate_ifc.py`, then each part's shape built by IfcOpenShell holds
its `NetVolume`. `ci/ifc-coverage.txt` takes a wall and a floor apart for CI's IFC job.

## The file, for now

`FORMAT` stays 18. A BIMCraft from before parts refuses a file with an element of a category it doesn't know, so a file keeps
its parts under a top-level key, `parts` (`parts::file_text`, `parts::from_file`), which that one leaves out and names; and
**nothing but a part may point at a part** (`Model::check`), or what the older one keeps would point at what it left out.
Both are **provisional**: that older BIMCraft drops every part when it saves, which is a misreading, so before parts go
upstream they move into `elements` with a step of `FORMAT` and `Model::migrate` (the fork's other lines hold 19 and 20, so 21
or whatever is free then). Until then a per-view Parts Visibility field and rule-based overrides are fine; a per-element
reference to a part (a tag's host, a view's override of one element, a dimension) is not. A project parameter for parts names
the category `part`, which an older one refuses on opening, saying so.

## Next

Divide Parts (the engine is `divide.rs`; a division is a new `Source`, its cuts in the host's own coordinates, and the divided
part the sum of its divisions), edge offsets and gaps, parts in plans, sections and the 3D view with Parts Visibility (Show
Original, Show Parts, Show Both) and colours by `Decay_Pathology` or `Historical_Period`, schedules and tags of parts, the
desktop's tools, reading parts from IFC, density and thermal values on materials, and the `FORMAT` step.
