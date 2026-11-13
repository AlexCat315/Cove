# Inspector frame pane

The App frame pane is the working surface for picking and frame input, so it is
sized and zoomed for that job rather than for a preview thumbnail.

## Sizing

- The pane takes `flex: 0 0 62%` of the details column with a `220px` floor. The
  node inspector keeps the rest and can shrink to its `160px` floor.
- `Expand` in the frame header overlays the pane on the whole details column
  (`.screenshot-panel.is-expanded`, absolutely positioned against
  `.details-pane`). It is a one-click trade of the node inspector for a
  full-height frame, and `Collapse` restores the docked height.
- The body is `flex: 1 1 auto`; the image is `max-width/height: 100%` with
  `margin: auto`, so a fitted frame is centered and uses the pane rather than a
  viewport-relative cap.

## Zoom and pan

- `#screenshot-zoom` cycles `fit → 1× → 2×` (`ZOOM_CYCLE` in
  `src/inspector/capture.js`). The label names the next step (`100%`, `200%`,
  `Fit`) and the meta shows the scale actually drawn.
- Numeric steps set an inline width of `capture.width * zoom`, keep the aspect
  ratio, and add `is-zoom`. Dragging the body pans the frame, so no scrollbar
  hunt; the pointer is captured only after a drag passes the 4px threshold,
  because capturing on `pointerdown` would retarget the following click to the
  pane and swallow the pick.
- Two CSS details decide whether the step is honest. A flex item's automatic
  minimum is its intrinsic width, so a zoomed image needs `flex: 0 0 auto` or it
  stays clamped at the intrinsic size and every click maps as if scale were 1.
  And an overflowing image must be anchored `flex-start`/`margin: 0` in
  `is-zoom`: centring puts its left edge at a negative offset where no scroll
  reaches it.
- `imagePoint` maps a screen point through `getBoundingClientRect()`, so the
  pick stays exact at any zoom step (2× halves the offset in image pixels).

## Verification

`client/tests/capture-harness.mjs` serves a 2560×1440 frame and checks: fit fills
more than the old `34vh` cap and reports its real scale; `Expand` covers the
details column and `Collapse` restores the docked height; 1× scrolls inside the
pane; a drag pans it; a 2× click reaches `select_ui_at` with image coordinates
scaled by the step.
