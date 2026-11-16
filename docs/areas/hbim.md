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

- A wall's sides: `u` along the face from square to the location line's start (the arc length along that face's own
  curve on a curved wall), `v` up from the wall's own base. On a straight wall `u` is a door's offset, as Place on Face's
  `uv` is. Seen from outside, `u` runs right to left on the exterior: `(u, v)` is the same on both sides, not mirrored.
- A wall's ends: `u` across from the exterior face, `v` up.
- Any top or bottom: `(u, v)` in plan from the element's own origin and direction (a slab's first boundary point and
  first edge, a column's point and angle, a wall's start and direction), the height and slope found under each point.
- A side: `u` along the edge from its first point, `v` up from the lowest point of the face's bottom.

Every surface is a plane, a plumb cylinder or a top made of planes, so `point`, `normal` and `uv` are exact, and
`Chart::measure` reads an outline's area on the true surface: developed on a curved wall, over the cosine on a slope.
`Chart::frame` gives the face's axes at a point (`origin`, `x`, `normal`), the frame a cut drawn on a face takes. A
chart's `outline` is the face's boundary in `(u, v)`; `openings` are what the model cuts in it (a wall's doors and windows
as rectangles, a slab's openings). Site toposolids, when they come, are a top of this kind.

Not yet: a wall's sides end square at its location line's ends (a mitre or a join doesn't lengthen them), a shaft
through a floor isn't one of its openings, and the openings of a family with its own voids are their rectangles.

## Survey areas (`survey.rs`, `commands/survey.rs`)

A `SurveyArea` (category `surveyarea`) is a survey area or, with `kind: {"item": category}`, a drawn item (a stone, a
brick, a painting) counted on a face. It holds `host` (GUID), `face`, `outline` and `holes` in the face's `(u, v)`, a
`condition` (`decay::Decay`: the 26 terms Heritage Parts uses, or `{custom}`), `severity` 1 to 5, `description`,
`intervention`, an optional `depth` (mm, for a volume), the `survey` that found it and `photos` (paths of files beside
the project, never held in it). It holds nothing in 3D: `Model::survey_resolved` works out its outline in space, its area,
what openings take out of it (`cut`, `cut_share`), its net area and volume, and `conflict` when its outline lies off its
face, wholly or in part, or the face is gone. An area in conflict keeps all it says. It goes with its host
(`delete`), moves only with it, and changes nothing of its host's geometry, quantities or IFC.

A survey (campaign) is a library entry, `Model::surveys` by name (`library::Kind::Survey`): its `date` and `method`
(visual, thermography, scan or its own). Areas name it, follow it renamed, and hold its GUID in a file.

Commands: `survey` (add, change, rename, delete, list), `survey_area` (make with `host`, `face` and `outline`; change with
`id`; `id` alone shows it with its quantities), `faces` (an element's faces with their outlines in `(u, v)`, and where
given `(u, v)` lie in space). A schedule of category `surveyarea` lists kind, condition, severity, net area, what openings
take, volume, survey, level, host, face and conflict.

Files: the `surveyarea` elements and the project's `surveys` are written only when there are any, so a project without
them saves as it always did. A BIMCraft from before refuses a file with a survey area in it (an element of a category it
doesn't know), so these need a `FORMAT` step before they go upstream, taken with Heritage Parts'.
