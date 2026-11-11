# Project parameters and schedules

Moved from the root `CLAUDE.md`, word for word: the root keeps the rules and the map, and this file is read when the area is touched.

A project parameter (`Model::parameters`) is a value each element of its categories may hold in its `params`, of its kind;
a component's `params` hold its family's instance values too, so a parameter for components can't take a family
parameter's name, nor any category's own field's (`schedule::own_field`). A schedule's fields are its category's own
(`OWN` in `schedule.rs`, read from `element_json`), the project parameters for it and, for components, the families'
parameters, or calculated: a formula over the row's others, read as the family formula engine reads (lengths in mm, areas in
m², volumes in m³), a field with no value leaving it with none (an unset yes or no is no). Filters compare numbers as the schedule shows
them, so two that show the same are equal. What a field holds is its `schedule::Kind`: text, a number, a length, an area, a
volume (m³, shown to three places, a litre, in m³ or ft³ as `Units::volume` says, its heading saying which, as an area's
does) or yes or no; numbers, lengths, areas and volumes total (`Kind::numeric`). A total, and a grouped row's sum, of rows
none of which has a number is no value, never 0 (`sum`). The six schedules every project has (`schedule::VIEWS`) aren't elements
until changed or placed (`schedule::standard`), and a schedule named as one of them, in any language or case, is that one; renaming a parameter renames it in schedules, but a formula naming it holds
the rename back.
A component's Room Number and Room Name (as Revit's Room: Number and Room: Name) are the room it stands in, found where it is
read and never stored (`Rooms::room_at`): the room whose boundary at the walls' centre lines holds the
component's point on its level, so an outlet on a wall's face is in the room on that side whatever the project measures rooms
at. `element_json` gives it as `room` (the room's GUID), `room_number` and `room_name`; a component outside every room has none.
A multi-category schedule (`of: "multicategory"`, `schedule::MULTI`, as Revit's Multi-Category Schedule) has a row for each
element of the categories it lists (`Schedule::categories`; none, every built one, `schedule::built`: not rooms, levels and
masses) and the fields they share (`SHARED`): Category (the element's, as elements name theirs), Family, Type, Level, Count,
Width, Height, Length, Thickness and Area, each its own category's field of that name where it has one holding the same and
empty where not (an area worked out for walls, length by height, and doors and windows, width by height; a roof's plan
area), and Material, its type's structure layer's (or its thickest's), a run's type's or a family's first material
parameter's. Its filters, sorting, grouping, totals and calculated fields are any schedule's; only it lists categories.
A table goes to a file as CSV (`schedule::to_csv`, text a spreadsheet would read as a formula guarded with `'`) or, for a path
ending `.xlsx`, as a workbook of one sheet (`schedule::to_xlsx` over `xlsx.rs`, which writes the parts and the zip by hand on
`flate2`): headings in bold, numbers as numbers so they sum, text as inline strings, which a spreadsheet never takes for a formula,
no value as no cell, and the totals row in bold (`xlsx::workbook_with`).

## Parts: the restoration's bill of quantities

Parts ([parts](parts.md)) are a category schedules are of, `part` (`Parts`, as Revit lists it): `schedule {name, category:
"parts"}`, or Parts in the desktop's New Schedule dialog. A row is a part, and lists it (`Table::ids`), so a click on it
selects the part as any schedule's row selects its element. Its fields are a part's values by name (`OWN`'s `part`, read from
`parts::json`): `Id`, `Host`, `Host Category`, `Host Type`, `Level`, `Layer`, `Function`, `Material`, `Thickness`, `Net Area`
(m²) and `Net Volume` (m³), which a schedule shows unless it chooses, then `Density`, `Mass`, `Thermal Conductivity`, `Thermal
Resistance`, `Specific Heat`, `Predefined Type`, `Object Type`, `Excluded`, the heritage set (`SU_Code`, `Historical_Period`,
`Decay_Pathology`, `Decay_Severity`, `Intervention_Type`), `Conflict`, `Count`, the phases, the design option, and the
project's parameters for parts. Nothing of them is stored: a row is what the part is of its host now, so the schedule follows
a wall that is stretched or takes a window. A value nobody gave is no value, never 0.

- **Excluded parts are left out**, as Revit leaves them out of schedules (`sources`), but of a schedule with a filter on
  `Excluded`, which says for itself which it lists (`Excluded equals true` lists the excluded).
- **Parts in conflict are listed**, counted, with their `Conflict` and with no `Net Area` and no `Net Volume`
  (`From::Measured`: a listing says 0 of a conflicted part's volume, a schedule none), so they add nothing to a sum and are
  not silently missing either: the table names them in `Table::notes` (`notes` in the command's output, a line over the rows
  in the desktop). `Conflict has-no-value` as a filter leaves them out, `has-value` lists only them.
- A part that parts were divided from is listed with no area and no volume: its divisions hold them, so nothing is summed
  twice (its `Count` is still 1).
- A multi-category schedule lists no parts (`NOT_LISTED`): they are of what it lists, and would be counted beside it.

**The bill of quantities** is a schedule of parts sorted by what the works are priced by, `grouped`, with `total` on its
quantities: a row for each `Intervention_Type` (or `Decay_Pathology`, `Historical_Period`, `Material`, `Host`, or several in
turn, a row for each set of their values) with its parts' `Net Area`, `Net Volume` and `Count` summed, a field whose values
differ left empty, and the totals of all of them under the rows. Parts with no value of a sort field come together under a
blank heading, first among text (after every number, for a number such as `Decay_Severity`): neither dropped nor counted as
0. Filters narrow it (`Decay_Severity greater 2`). Areas show to two places and volumes to three; the table, the CSV and the
workbook hold them as fine as a part's listing does (the square centimetre, the cubic centimetre), and sums are taken of
those, not of what is shown.

**The starter schedule.** `schedule {starter: "restoration-quantities"}` (`schedule::starter`, `STARTERS`; Restoration
Quantities in the desktop's Schedules list until the project has it) makes the schedule Restoration Quantities, named in the
project's language, or under `name`, in one go, and reads it once the project has one of that name; it is the project's own
from then on, changed as any other. It is this, which the command takes as well:

```json
{"name": "Restoration Quantities", "category": "parts", "grouped": true,
 "fields": ["Intervention_Type", "Decay_Pathology", "SU_Code", "Host Type", "Material",
            {"name": "Net Area", "total": true}, {"name": "Net Volume", "total": true}, {"name": "Count", "total": true}],
 "sort": ["Intervention_Type", "Decay_Pathology"]}
```

With `path`, it goes to a file as any schedule: `schedule {starter: "restoration-quantities", path: "works.xlsx"}` writes
the workbook, its headings `Net Area (m²)` and `Net Volume (m³)`, its quantities numbers to be priced, its totals in bold.
Prices are not part of it. A unit price by `Intervention_Type` would be a project parameter of parts (a number, `Unit
Price`) and a calculated field (`Net Area * Unit Price`, totalled); a price list keyed by the intervention, one row to
fill in, is what schedules lack for it (a key schedule).

**The file.** A schedule of parts is a `schedule` element whose `of` is `part`: a BIMCraft from before parts refuses the
whole file on opening (`Model::check`: there's no schedule of `part`), and one from before `Kind::Volume` refuses a file
with a calculated field of `kind: "volume"` (an unknown variant). Both wait for the `FORMAT` step parts take
([parts](parts.md)).

## The same numbers as Revit

`crosscheck.rs` holds the numbers schedules show against Revit's ([tools/revit-crosscheck](../../tools/revit-crosscheck/README.md)):
`export_crosscheck` describes a project in plain numbers (walls by their centre lines, floors, shafts, doors and windows,
rooms by their points), a script of ours builds it again in Revit and writes what Revit reports, `kind,id,quantity,value`
by our element ids, and `compare_crosscheck` (and the test `the_small_tower_has_the_numbers_revit_gives_it`, once Revit's
table of the small two-storey tower is committed) compares them quantity by quantity within `crosscheck::tolerance`.
Compared: each room's area, perimeter, whether it is enclosed, its unbounded height and its volume, each wall's length
along its centre line and its doors and windows, each floor's area less its openings and shafts, and its volume. Revit
reports more (a wall's area and volume) than we work out yet: those come back as `only_theirs`, the list of what is still
to do. Where the two define a quantity differently, the difference goes here, with the rule the comparison applies.

- A room's volume is its area times its unbounded height (from its level and base offset up to its upper limit and limit
  offset), which is Revit's while nothing under its top bounds it. Revit also stops a room's volume at a room-bounding
  ceiling, floor or roof lower than its top, and a ceiling bounds rooms unless told not to; we don't yet. The description
  gives Revit each room's limits as ours. The tower's rooms reach 300 mm under the level above (3.7 m, the `room`
  command's default), over their ceilings at 2.7 m: Revit's volume of them is their area times 2.7 m where ours is times
  3.7 m, the one difference the comparison is expected to show until volumes stop at ceilings.
- Each level's computation height (where walls and columns are cut to bound rooms) goes to Revit as the level's.
