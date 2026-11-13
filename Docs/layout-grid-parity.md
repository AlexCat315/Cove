# Grid parity audit

Cove has one grid geometry engine: Taffy 0.15.1 with the `grid` and `calc` features.
`crates/compose/src/primitives/layout/grid/` is a table policy layer (column
policies, named-column visibility, row windows); it compiles its policy into a
`GridStyle` and lets Taffy place the cells. No second track solver exists.

This file records what the authored surface maps to, what has been measured, and
where behaviour can still surprise an author. Every number below comes from
`crates/compose/tests/grid_placement_parity.rs`; re-run it to reproduce.

```sh
cargo test -p cove-ui-compose --test compose grid_placement_parity
cargo test -p cove-ui-compose --test compose grid_modifier_tests
cargo test -p cove-core --test grid_area_template
```

## Authored surface → CSS

| Authored | CSS |
|----------|-----|
| `GridSpec::new(tracks)` / `.columns(...)` | `grid-template-columns` |
| `.rows(...)` | `grid-template-rows` |
| `.auto_rows(...)` / `.auto_columns(...)` | `grid-auto-rows` / `grid-auto-columns` |
| `.flow(GridAutoFlow::Row\|RowDense\|Column\|ColumnDense)` | `grid-auto-flow` |
| `.column_gap(...)` / `.row_gap(...)` / `.gap(...)` | `column-gap` / `row-gap` |
| `.areas(GridAreaTemplate)` | `grid-template-areas` |
| `.column_names(...)` / `.row_names(...)` | line names in the template |
| `.justify_items(...)` / `.align_items(...)` | `justify-items` / `align-items` |
| `GridCell::at` / `spanning` / `lines` | `grid-column` / `grid-row` with explicit lines |
| `GridCell::span` / `.grid_span` | `grid-column: span N` / `grid-row: span N` |
| `GridCell::area` / `spec.cell(name)` | placement by area name |
| `GridCell::named((start, end), ...)` | placement by line name |
| `.grid_justify_self(...)` / `.grid_align_self(...)` | `justify-self` / `align-self` |

Track breadths: `GridTrack::points`, `percent`, `fr`, `AUTO`, `minmax`, plus
`GridTemplateTrack::repeat(GridRepeat::AutoFill, tracks)` for `repeat()`.

## Viewport fill

The layout root fills the viewport on its own, but a view's children size to
their content. A grid container without a definite height resolves `fr` rows
against a content height of zero.

Measured, 400×300 viewport, two `fr(1.0)` rows and one fixed 40pt row:

| Container | Container frame | `fr` row |
|-----------|-----------------|----------|
| no frame modifier | 400×50 | 0 |
| `.frame_height(300.0)` | 400×300 | 250 |
| `.fill()` | 400×300 | 250 |

`.fill()` is `width: 100%; height: 100%` on the shell view; `.fill_width()` and
`.fill_height()` cover a single axis. This is ordinary CSS percentage resolution
against a definite parent, not a grid special case, and it is the authored
convention for "this view covers the window".

## Placement matrix

Constants: 320×200 container, three `fr(1.0)` columns (100pt each), 10pt gaps,
20pt item height.

| Case | Authoring | Measured frames | Verdict |
|------|-----------|-----------------|---------|
| A sparse span | `span(2,1)`, then a single item | `(0,0,210,20)`, `(220,0,100,20)` | CSS sparse: the single item fills the remaining track |
| B sparse wrap | two `span(2,1)` items | `(0,0,210,20)`, `(0,30,210,20)` | CSS sparse: a span that does not fit moves to the next row instead of straddling a hole |
| C definite first | pinned `spanning(1,1,1,2)`, then `span(2,1)` | pinned `(0,0,100,20)`, span `(110,0,210,20)` | CSS: the span starts on track 2 because track 1 is occupied |
| D dense backfill | `RowDense`, pinned `at(2,1)`, then two singles | pinned `(110,0,100,20)`, first `(0,0,100,20)`, second `(220,0,100,20)` | CSS dense: it backfills the hole left by the pinned item |
| E implicit track | 3 explicit tracks, item at line 4 | first `(0,0,96.67,20)`, far `(320,0,0,20)` | CSS: an implicit `auto` track is created; empty content gives it zero width and the three `fr` tracks absorb the extra gap |
| F per-item alignment | 200pt track, 60pt item, `justify-self: end` | `(140,0,60,20)` | CSS: the item keeps its width and aligns to the track end |
| G column flow | `Column`, two 100pt columns, three items | `(0,0,100,20)`, `(110,0,100,20)`, `(220,0,0,20)` | No explicit rows means one implicit row, so items run across; CSS derives the row count the same way |
| H definite collision | two items both `at(1,1)` | both `(0,0,100,20)` | CSS: definite collisions overlap instead of erroring |

Conclusion: the earlier suspicion that Taffy diverges from CSS sparse/dense
placement is wrong. A–H all follow the CSS algorithm. The differences an author
can still hit are the ones CSS itself defines:

1. **Explicit collisions are silent.** Two items pinned to one cell overlap,
   exactly as CSS does, but a layout mistake then looks like a rendering bug.
   This is a candidate for an authored-layer diagnostic, not for an engine change.
2. **Empty implicit tracks collapse to zero.** `at(4, 1)` past a three-track
   template opens an implicit `auto` track that sizes to content; with no
   content it takes no space and the explicit tracks keep their starts. Set
   `.auto_columns([GridTrack::points(…)])` to give implicit tracks a size.
3. **`Column` flow needs explicit rows.** Without `.rows(...)` the row axis is
   one implicit row, so column flow runs across rather than stacking.
4. **A table `Grid` only honours spans.** Named-column visibility and row
   windows own the axis, so explicit lines are ignored there by design; a caller
   that needs explicit placement uses `.grid(GridSpec)`.

## Not covered yet

- `subgrid` (Taffy 0.15 has no support; CSS-only feature).
- Baseline alignment inside a grid cell (`align-self: baseline` on grid items).
- Diagnostics for overlap and for out-of-template lines; today both are
  observable only through the resulting frames.
- The Inspector does not yet show the resolved track sizes or per-item lines,
  so a parity case has to be asserted in a test rather than inspected live.
