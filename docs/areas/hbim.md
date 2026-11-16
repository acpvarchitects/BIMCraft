# Historic buildings (HBIM): face coordinates and survey areas

Recording a historic building means drawing on its surfaces: where the black crust is, which stones were replaced,
how much render has come away. BIMCraft holds such things on the face of an element, in that face's own coordinates,
so they follow the element when it changes.

## Face coordinates (`face.rs`)

Every face of a wall, a floor, a ceiling, a roof or a column has its own `(u, v)`, in millimetres, worked out from the
element as it stands (`Model::face_chart`, a `Chart`). A face is named by a `FaceId`: a wall's `exterior`, `interior`,
`start`, `end`, `top` and `bottom`; a slab's or a column's `top`, `bottom` and `{"side": i}` (the upright face along edge `i`
of its outline in plan; a round column's one curved side is side 0). The name is what the face is to its element, so it
survives joins, openings, retyping and thickening; `FaceId::role` maps it to Place on Face's `FaceRole`.

The coordinates are Place on Face's where it has them (`Model::face_at`, `face_uv`):

- A wall's sides: `u` mm along the location line from its start (a door's offset), `v` up from the wall's own base. On a
  curved wall a side is longer than its line by its radius over the line's (`k`), which areas are measured by, so they
  are developed; a place in `(u, v)` stays square to the same point of the line when the wall is thickened or retyped,
  and `modify` of its `location` rescales it as it rescales the doors' offsets. Seen from outside, `u` runs right to left
  on the exterior: `(u, v)` is the same on both sides, not mirrored.
- A wall's ends: `u` across from the exterior face, `v` up.
- Any top or bottom (a slab's, a column's, a wall's): `(u, v)` is the point in plan, in the model's axes, as Place on Face
  has it; the height and slope under each point come from `hosting::piece_at`, the lookup `face_at` uses, on the pieces
  the element is cut into where what it is attached to changes plane (a wall's or a column's attached top). Moving,
  turning or mirroring the element takes what is drawn there with it.
- A side: `u` along the edge from its first point, `v` up from the lowest point of the face's bottom, which follows what a
  column is attached to.
- A round column's side: `u` the arc length counter-clockwise from its angle, round and round (`Chart::period`): an
  outline across where `u` starts again measures as it lies. Its top is laid out with 512 sides.

Every surface is a plane, a wall's side (plumb, straight or curved), a plumb cylinder or a top made of planes, so
`point`, `normal` and `uv` are exact, and `Chart::measure` reads an outline's area on the true surface: developed on a
curved wall, over the cosine on a slope. `Chart::lift` gives an outline in space, cut where it crosses from one slope to
another and along a curve. `Chart::frame` gives the face's axes at a point: `origin`, `x` (along `u`), `y` (along `v`) and
`normal`; `y` is given, not `normal × x`, because on an exterior, an end and a bottom `x × y` is the inward normal. Heritage
Parts' cuts on a face (`origin`, `x`, `normal`) read it with `y` for which side `v` is on. A chart's `outline` is the face
in `(u, v)`, openings not taken out; `openings` are what cuts it: a wall's doors and windows (their family's voids where
they have them, else their rectangles) and the voids of components on its faces; a slab's openings, the shafts through
it (`less_shafts`) and its components' voids; a hollow column's hole. A void is the hull of its points seen square on
the face, where it reaches the face. They are found through the model's lookup of what points at what, and a listing or a
schedule works each face out once (`rooms::Rooms::chart`). `Model::faces_of` leaves out a wall's end where another wall
is joined to it. Site toposolids, when they come, are a top of this kind.

Not yet: a wall's sides end square at its location line's ends (a mitre or a join doesn't lengthen them), and a column's
side faces are not carried by a mirror.

## Survey areas (`survey.rs`, `commands/survey.rs`)

A `SurveyArea` (category `surveyarea`) is a survey area or, with `kind: {"item": category}`, a drawn item (a stone, a
brick, a painting) counted on a face. It holds `host` (GUID), `face`, `outline` and `holes` in the face's `(u, v)`, a
`condition` (`decay::Decay`: the 26 terms Heritage Parts uses, or `{custom}`), `severity` 1 to 5, `description`,
`intervention`, an optional `depth` (mm, for a volume), the `survey` that found it and `photos` (paths of files beside
the project, never held in it). It holds nothing in 3D: `Model::survey_resolved` works out its outline in space, its area,
what openings take out of it (`cut`, `cut_share`), its net area and volume, and `conflict` when its outline lies off its
face, wholly or in part, or the face is gone. An area in conflict keeps all it says. It goes with its host
(`delete`), moves only with it, and changes nothing of its host's geometry, quantities or IFC. Flipping a wall puts its
areas on the other side, as far from the other end; mirroring it takes them from its other end; on a top or a bottom they
are moved, turned and mirrored with their host, and copied with it when they are copied together.

A survey (campaign) is a library entry, `Model::surveys` by name (`library::Kind::Survey`): its `date` and `method`
(visual, thermography, scan or its own). Areas name it, follow it renamed, and hold its GUID in a file.

Commands: `survey` (add, change, rename, delete, list), `survey_area` (make with `host`, `face` and `outline`; change with
`id`; `id` alone shows it with its quantities), `faces` (an element's faces with their outlines in `(u, v)`, and where
given `(u, v)` lie in space). A schedule of category `surveyarea` lists kind, condition, severity, net area, what openings
take, volume, survey, level, host, face and conflict.

Files: the `surveyarea` elements and the project's `surveys` are written only when there are any, so a project without
them saves as it always did. A BIMCraft from before refuses a file with a survey area in it (an element of a category it
doesn't know), so these need a `FORMAT` step before they go upstream, taken with Heritage Parts'.
