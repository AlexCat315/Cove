#pragma once
#include <stddef.h>
#include <stdint.h>
typedef struct { uint32_t glyph_index; float width,height,bearing_x,bearing_y,advance; } CoveGlyphMetrics;
typedef struct { float ascent,descent,line_height; } CoveLineMetrics;
typedef struct { uint8_t *pixels; size_t length; uint32_t width,height; CoveGlyphMetrics metrics; int valid; } CoveGlyphBitmap;
typedef struct { uint32_t glyph_id; float x_offset,y_offset,x_advance,y_advance; uint32_t cluster; int64_t font_id; } CoveShapedGlyph;
typedef struct { CoveShapedGlyph *glyphs; size_t length; } CoveShapedRun;
typedef struct { uint32_t tag; char *name; double minimum,default_value,maximum; } CoveFontAxis;
typedef struct { CoveFontAxis *axes; size_t length; } CoveFontAxes;
void *cove_text_library_new(void);
void cove_text_library_free(void *library);
void *cove_text_face_new(void *library,const uint8_t *bytes,size_t length,uint32_t selector,float size,float scale);
void cove_text_face_free(void *face);
const char *cove_text_face_name(void *face);
int cove_text_face_color(void *face);
int cove_text_face_size(void *face,float size,float scale);
uint32_t cove_text_glyph_index(void *face,uint32_t scalar);
int64_t cove_text_probe(void *library,const uint8_t *bytes,size_t length,const char *name);
CoveFontAxes cove_text_axes(void *face);
void cove_text_axes_free(CoveFontAxes axes);
int cove_text_apply_axes(void *face,const uint32_t *tags,const double *values,size_t count);
int cove_text_glyph_metrics(void *face,uint32_t glyph,int layout,CoveGlyphMetrics *metrics);
CoveLineMetrics cove_text_line_metrics(void *face,int color);
CoveGlyphBitmap cove_text_glyph_bitmap(void *face,uint32_t glyph,int color);
CoveShapedRun cove_text_shape(void *face,const uint8_t *text,size_t length,uint32_t direction,uint32_t script,const char *language,uint32_t offset,int64_t font_id);
uint32_t cove_text_script(uint32_t scalar);
void cove_text_free(void *memory);
void cove_text_copy_rows(const uint8_t *source,size_t source_stride,uint8_t *target,size_t target_stride,size_t bytes,size_t rows);
void cove_text_resample_rgba(const uint8_t *source,size_t width,size_t height,uint8_t *target,size_t target_width,size_t target_height);

/* Apple implementations are compiled for both macOS and iOS. */
void *cove_apple_font_new(const char *name,float size,float weight,int italic);
void *cove_apple_font_copy(void *font,float size);
void cove_apple_font_free(void *font);
char *cove_apple_font_name(void *font,int family);
char *cove_apple_font_path(void *font);
size_t cove_apple_font_variations(void *font,uint32_t *tags,double *values,size_t capacity);
void *cove_apple_font_fallback(void *font,const uint8_t *text,size_t length,size_t location,size_t range_length);
int cove_apple_color_metrics(void *font,float scale,uint32_t glyph,CoveGlyphMetrics *metrics);
CoveLineMetrics cove_apple_line_metrics(void *font);
CoveGlyphBitmap cove_apple_color_bitmap(void *font,float scale,uint32_t glyph);
CoveShapedRun cove_apple_shape(void *font,const uint8_t *text,size_t length,uint32_t offset,int64_t font_id);
