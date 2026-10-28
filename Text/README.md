# CoveUIText

Desktop and browser hosts share font discovery, owned font faces, shaping,
rasterization, atlases and text caches through `CoveUIText`. Only this package
binds FreeType and HarfBuzz. It depends on Core and has no window, GPU or socket
dependency. Platform font discovery and color-glyph adapters live here too.

```sh
./cove setup
./cove test Text --no-parallel
```

The browser's `cove-dev browser build-fonts` operation builds these pinned
sources with the installed Swift WASI SDK and stages separate static-library
artifact bundles. Its build script sets `COVE_TEXT_ARTIFACT_ROOT`; desktop
artifacts remain in `CoveUI/vendor`.
Do not export this override globally when switching back to desktop builds.

```swift
let fonts = FontCollection()
fonts.append(bytes: fontData) // owned copy; add fallback faces in priority order
fonts.draw("office 你好", size: 16, rasterScale: 2,
           into: drawList, x: 10, y: 10, color: .white, textureID: 1)
if let upload = fonts.atlas.dirtyUploadPayload() {
    // Copy upload.pixels into the R8 atlas at upload.region, then:
    fonts.atlas.markClean()
}
```

The atlas is 2048×2048 by default and caches glyphs by face, logical size and
raster scale. When `isFull` becomes true, reset the atlas and redraw the entire
frame: old vertices reference old UVs. A frame that itself exceeds atlas capacity
still needs a larger atlas or pagination. Never reset between individual labels.

Fallback keeps each Swift grapheme together and chooses the first face covering
its non-format scalars. Common/inherited characters keep the current script and,
where possible, its font. HarfBuzz shapes each script/font run with inferred
direction, preserving UTF-8 source clusters. This handles Latin ligatures,
Chinese, Arabic joining and Devanagari reordering; it does **not** implement
Unicode paragraph bidi, language-specific fallback policy, or color emoji.
Mixed LTR/RTL paragraphs need a bidi/itemization layer before visual run layout.
Missing characters use the primary font's `.notdef` glyph.

Instances are confined to one render thread. Keep the atlas/collection alive
while its faces are used by shapers. Memory font storage belongs to the FreeType
face finalizer, so a HarfBuzz reference can survive replacement of the current
face. `FontAtlas.loadFont(bytes:)` preserves the current face on invalid input.

The demo fonts in [Fonts](Fonts/README.md) have their SIL OFL licenses and exact
checksums included. They are demo assets, not a platform font-discovery service.
