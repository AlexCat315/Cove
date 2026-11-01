# cove-text

Font discovery, owned font faces, shaping, rasterization, atlases and text
caches. Only this crate binds FreeType and HarfBuzz; both are built from
`third-party/freetype` and `third-party/harfbuzz` by `build.rs`. Platform font
discovery and color-glyph adapters live here too.

```sh
cargo test --package cove-text
```

The atlas is 2048×2048 by default and caches glyphs by face, logical size and
raster scale. Upload `dirty_upload_payload()` into the R8 atlas, then call
`mark_clean()`. When `is_full()` becomes true, reset the atlas and redraw the
whole frame: old vertices reference old UVs. Never reset between labels.

Fallback keeps each grapheme cluster together and picks the first face covering
its non-format scalars. Common and inherited characters keep the current script
and, where possible, its font. HarfBuzz shapes each script/font run with
inferred direction and UTF-8 source clusters. Unicode paragraph bidi and
language-specific fallback are not implemented. Missing characters use the
primary font's `.notdef` glyph.

Instances are confined to one render thread. `load_font_bytes` keeps the
current face on invalid input. The demo fonts in [fonts](fonts) ship with their
SIL OFL licenses.
