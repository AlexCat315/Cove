# Component tree presentation

The Inspector tree shows the published node graph as it is. Two presentation rules
exist because that graph is a modifier stack, not a widget hierarchy, and a tree
that renders it literally is unreadable past a few levels.

## Why the tree reads as one long chain

Each modifier in a composition is its own node with exactly one child. A live
gallery snapshot showed it directly: root → `AdaptiveApplicationContent` →
`SurfaceRoot` → `ProvideCompositionLocalModifier<…>` → `FrameModifier<Flex…>` →
… before any content node appears, and no sampled node carried a `debugName`. So
the depth is structural, not a client bug: nothing in the runtime flattens a
modifier stack into one node.

That graph stays the source of truth — the client does not invent parents. What
changed is what the tree draws.

## Folding wrappers

`tree.js` folds a *wrapper run* on load: a node is a wrapper when it has no
`debugName`, no `semanticRole`, no `source`, no `recomposition`, is not its own
owner scope, and has exactly one child. Consecutive wrappers collapse into the
node they wrap, which keeps its id and label and gains a `+N` chip listing the
folded type names on hover.

Consequences:

* Folding follows the published graph, so it works on any snapshot without a
  protocol change.
* A chain that ends in a leaf folds too; the leaf keeps the chip.
* Named scopes never fold, so `Components` and `Primitives` filters still map to
  real nodes, and selection, details and editing keep using the unfolded ids.
* `Fold wrappers` in the filter row turns it off, which restores every wrapper as
  its own row. Expansion state survives the switch.

On the same 40-deep synthetic chain the harness measures 85 unfolded rows and 3
folded rows.

## Rows grow, the pane scrolls

A row is `min-width: max-content`, so it is as wide as its indent, twisty, glyph
and label together; the pane (`overflow: auto`) scrolls horizontally. Labels are
`flex: 0 0 auto`, so a deep row never squeezes its label to nothing.

Before that, a row was a fixed-width flex line and `.tree-label` was the only
shrinkable child, so past a certain depth every label ellipsized to zero width and
the row showed only a twisty and a glyph — exactly what a deep gallery tree looked
like. The harness pins the fix with numbers: 85 rows scroll 714px inside a 500px
pane, and every label keeps a non-zero width.

## What is still open

The runtime could publish a modifier run as one node carrying a modifier list,
which would flatten the graph for every consumer instead of folding it in one
client. That changes node identity (`ownerScopeID`, recomposition scope, style
editing targets) and is not done.
