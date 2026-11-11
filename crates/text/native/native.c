/* Native hot paths from FontFace.swift, FontAtlas.swift and FreeTypeColorGlyphs.swift. */
#include "native.h"
#include <ft2build.h>
#include FT_FREETYPE_H
#include FT_GLYPH_H
#include FT_OUTLINE_H
#include FT_MULTIPLE_MASTERS_H
#include <hb.h>
#include <hb-ft.h>
#include <limits.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>

typedef struct { FT_Library library; FT_Face face; hb_font_t *font; float size,scale,strike_scale; int color_active; } CoveFace;
static char *copy_string(const char *text) { if (!text) text=""; const size_t n=strlen(text)+1; char *p=malloc(n); if (p) memcpy(p,text,n); return p; }
void cove_text_free(void *memory) { free(memory); }
void *cove_text_library_new(void) { FT_Library library=NULL; return FT_Init_FreeType(&library)==0 ? library : NULL; }
void cove_text_library_free(void *library) { if (library) FT_Done_FreeType(library); }
static int requested_size(CoveFace *face) {
    const int result=FT_Set_Char_Size(face->face,0,(FT_F26Dot6)(face->size*face->scale*64),72,72);
    face->color_active=0; face->strike_scale=1; return result;
}
void *cove_text_face_new(void *library,const uint8_t *bytes,size_t length,uint32_t selector,float size,float scale) {
    if (!library || !bytes || !length || length>LONG_MAX || !isfinite(size) || size<=0 || size>512 || !isfinite(scale) || scale<1 || scale>4) return NULL;
    CoveFace *owned=calloc(1,sizeof(CoveFace)); if (!owned) return NULL; owned->library=library; owned->size=size; owned->scale=scale; owned->strike_scale=1;
    if (FT_New_Memory_Face(library,bytes,(FT_Long)length,(FT_Long)(selector&0xffffu),&owned->face)) { free(owned); return NULL; }
    if ((selector>>16) && FT_Set_Named_Instance(owned->face,selector>>16)) { FT_Done_Face(owned->face); free(owned); return NULL; }
    FT_Select_Charmap(owned->face,FT_ENCODING_UNICODE); requested_size(owned);
    owned->font=hb_ft_font_create_referenced(owned->face);
    if (!owned->font) { FT_Done_Face(owned->face); free(owned); return NULL; }
    hb_ft_font_set_load_flags(owned->font,FT_LOAD_NO_HINTING|FT_LOAD_NO_BITMAP); hb_ft_font_changed(owned->font); return owned;
}
void cove_text_face_free(void *value) { if (!value) return; CoveFace *face=value; hb_font_destroy(face->font); FT_Done_Face(face->face); free(face); }
const char *cove_text_face_name(void *value) { const char *name=FT_Get_Postscript_Name(((CoveFace*)value)->face); return name ? name : ""; }
int cove_text_face_color(void *value) { return FT_HAS_COLOR(((CoveFace*)value)->face)!=0; }
int cove_text_face_size(void *value,float size,float scale) {
    if (!isfinite(size) || size<=0 || size>512 || !isfinite(scale) || scale<1 || scale>4) return 0;
    CoveFace *face=value; const float old_size=face->size,old_scale=face->scale; face->size=size; face->scale=scale;
    if (requested_size(face)) { face->size=old_size; face->scale=old_scale; requested_size(face); return 0; } hb_ft_font_changed(face->font); return 1;
}
uint32_t cove_text_glyph_index(void *value,uint32_t scalar) { return FT_Get_Char_Index(((CoveFace*)value)->face,scalar); }
int64_t cove_text_probe(void *library,const uint8_t *bytes,size_t length,const char *name) {
    if (!bytes || !length || length>LONG_MAX || !name) return -1;
    FT_Face probe=NULL; if (FT_New_Memory_Face(library,bytes,(FT_Long)length,0,&probe)) return -1;
    const FT_Long count=probe->num_faces; FT_Done_Face(probe);
    for (FT_Long index=0; index<count && index<=0xffff; ++index) {
        FT_Face face=NULL; if (FT_New_Memory_Face(library,bytes,(FT_Long)length,index,&face)) continue;
        const char *ps=FT_Get_Postscript_Name(face); if (ps && strcmp(ps,name)==0) { FT_Done_Face(face); return index; }
        FT_MM_Var *axes=NULL;
        if (!FT_Get_MM_Var(face,&axes)) {
            for (FT_UInt instance=1; instance<=axes->num_namedstyles && instance<=0xffff; ++instance) {
                if (FT_Set_Named_Instance(face,instance)) continue; ps=FT_Get_Postscript_Name(face);
                if (ps && strcmp(ps,name)==0) { FT_Done_MM_Var(library,axes); FT_Done_Face(face); return ((int64_t)instance<<16)|index; }
            }
            FT_Done_MM_Var(library,axes);
        }
        FT_Done_Face(face);
    } return -1;
}
CoveFontAxes cove_text_axes(void *value) {
    CoveFace *face=value; CoveFontAxes result={0}; FT_MM_Var *axes=NULL;
    if (!FT_HAS_MULTIPLE_MASTERS(face->face) || FT_Get_MM_Var(face->face,&axes)) return result;
    if (axes->num_axis>SIZE_MAX/sizeof(CoveFontAxis)) { FT_Done_MM_Var(face->library,axes); return result; }
    result.axes=calloc(axes->num_axis,sizeof(CoveFontAxis));
    if (result.axes) {
        result.length=axes->num_axis;
        for (size_t i=0;i<result.length;++i) { const FT_Var_Axis *axis=&axes->axis[i]; result.axes[i]=(CoveFontAxis){(uint32_t)axis->tag,copy_string(axis->name),(double)axis->minimum/65536,(double)axis->def/65536,(double)axis->maximum/65536}; }
    }
    FT_Done_MM_Var(face->library,axes); return result;
}
void cove_text_axes_free(CoveFontAxes axes) { for (size_t i=0;i<axes.length;++i) free(axes.axes[i].name); free(axes.axes); }
int cove_text_apply_axes(void *value,const uint32_t *tags,const double *values,size_t count) {
    CoveFace *face=value; if (!count || !FT_HAS_MULTIPLE_MASTERS(face->face)) return 1; FT_MM_Var *axes=NULL;
    if (FT_Get_MM_Var(face->face,&axes)) return 0;
    if (axes->num_axis>SIZE_MAX/sizeof(FT_Fixed)) { FT_Done_MM_Var(face->library,axes); return 0; }
    FT_Fixed *coordinates=calloc(axes->num_axis,sizeof(FT_Fixed)); if (!coordinates) { FT_Done_MM_Var(face->library,axes); return 0; }
    for (FT_UInt i=0;i<axes->num_axis;++i) {
        coordinates[i]=axes->axis[i].def;
        for (size_t j=0;j<count;++j) if ((uint32_t)axes->axis[i].tag==tags[j] && isfinite(values[j])) {
            coordinates[i]=(FT_Fixed)fmax((double)axes->axis[i].minimum,fmin((double)axes->axis[i].maximum,round(values[j]*65536))); break;
        }
    }
    const int status=FT_Set_Var_Design_Coordinates(face->face,axes->num_axis,coordinates);
    free(coordinates); FT_Done_MM_Var(face->library,axes); hb_ft_font_changed(face->font); return status==0;
}
static void ensure_normal(CoveFace *face) { if (face->color_active) { requested_size(face); hb_ft_font_changed(face->font); } }
static int ensure_color(CoveFace *face) {
    if (!FT_HAS_COLOR(face->face)) return 0;
    if (face->color_active) return 1;
    face->strike_scale=1;
    if (FT_IS_SCALABLE(face->face)) { if (requested_size(face)) return 0; }
    else {
        const int count=face->face->num_fixed_sizes; if (count<=0 || !face->face->available_sizes) return 0;
        const float desired=face->size*face->scale; int selected=-1; float selected_ppem=INFINITY;
        for (int i=0;i<count;++i) { float ppem=(float)face->face->available_sizes[i].y_ppem/64; if (ppem>=desired && ppem<selected_ppem) { selected=i; selected_ppem=ppem; } }
        if (selected<0) { selected=0; selected_ppem=0; for (int i=0;i<count;++i) { float ppem=(float)face->face->available_sizes[i].y_ppem/64; if (ppem>selected_ppem) { selected=i; selected_ppem=ppem; } } }
        if (FT_Select_Size(face->face,selected)) return 0; face->strike_scale=selected_ppem>0 ? desired/selected_ppem : 1;
    }
    face->color_active=1; return 1;
}
static CoveGlyphMetrics bitmap_metrics(FT_GlyphSlot slot,float unit) {
    return (CoveGlyphMetrics){slot->glyph_index,(float)slot->bitmap.width*unit,(float)slot->bitmap.rows*unit,
        (float)slot->bitmap_left*unit,(float)slot->bitmap_top*unit,(float)slot->advance.x/64*unit};
}
int cove_text_glyph_metrics(void *value,uint32_t glyph,int layout,CoveGlyphMetrics *result) {
    CoveFace *face=value; ensure_normal(face); const float scale=fmaxf(face->scale,1);
    const FT_Int32 flags=layout ? FT_LOAD_NO_HINTING|FT_LOAD_NO_BITMAP|FT_LOAD_COMPUTE_METRICS : FT_LOAD_RENDER|FT_LOAD_FORCE_AUTOHINT|FT_LOAD_TARGET_LIGHT;
    if (FT_Load_Glyph(face->face,glyph,flags) || !face->face->glyph) return 0; const FT_GlyphSlot slot=face->face->glyph;
    *result=bitmap_metrics(slot,1/scale);
    if (layout) {
        result->width=(float)slot->metrics.width/64/scale; result->height=(float)slot->metrics.height/64/scale;
        result->bearing_x=(float)slot->metrics.horiBearingX/64/scale; result->bearing_y=(float)slot->metrics.horiBearingY/64/scale;
        FT_Glyph owned=NULL; if (!FT_Get_Glyph(slot,&owned)) { FT_BBox box; FT_Glyph_Get_CBox(owned,FT_GLYPH_BBOX_PIXELS,&box);
            result->width=(float)(box.xMax-box.xMin)/scale; result->height=(float)(box.yMax-box.yMin)/scale;
            result->bearing_x=(float)box.xMin/scale; result->bearing_y=(float)box.yMax/scale; FT_Done_Glyph(owned); }
    } return 1;
}
CoveLineMetrics cove_text_line_metrics(void *value,int color) {
    CoveFace *face=value; if (color==1) ensure_color(face); else ensure_normal(face);
    if (!face->face->size) return (CoveLineMetrics){face->size*0.8f,face->size*0.2f,face->size*1.2f};
    const FT_Size_Metrics metrics=face->face->size->metrics; const float unit=(color==1 ? face->strike_scale : 1)/face->scale;
    const float ascent=(float)metrics.ascender/64*unit,descent=(float)-metrics.descender/64*unit,height=(float)metrics.height/64*unit;
    return (CoveLineMetrics){ascent,descent,color==2 ? height : fmaxf(height,ascent+descent)};
}
void cove_text_copy_rows(const uint8_t *source,size_t source_stride,uint8_t *target,size_t target_stride,size_t bytes,size_t rows) {
    for (size_t row=0;row<rows;++row) memcpy(target+row*target_stride,source+row*source_stride,bytes);
}
void cove_text_resample_rgba(const uint8_t *pixels,size_t width,size_t height,uint8_t *output,size_t to_width,size_t to_height) {
    const float sx=(float)width/(float)to_width,sy=(float)height/(float)to_height;
    for (size_t y=0;y<to_height;++y) { const size_t y0=(size_t)((float)y*sy),y1=(size_t)fmaxf((float)y0+1,fminf((float)height,ceilf((float)(y+1)*sy)));
        for (size_t x=0;x<to_width;++x) { const size_t x0=(size_t)((float)x*sx),x1=(size_t)fmaxf((float)x0+1,fminf((float)width,ceilf((float)(x+1)*sx)));
            uint64_t r=0,g=0,b=0,a=0,count=0;
            for (size_t v=y0;v<y1 && v<height;++v) for (size_t u=x0;u<x1 && u<width;++u) { const size_t i=(v*width+u)*4; const uint64_t alpha=pixels[i+3];
                r+=pixels[i]*alpha; g+=pixels[i+1]*alpha; b+=pixels[i+2]*alpha; a+=alpha; ++count; }
            const size_t at=(y*to_width+x)*4; if (!count) continue;
            if (a) { output[at]=(uint8_t)(r/a); output[at+1]=(uint8_t)(g/a); output[at+2]=(uint8_t)(b/a); } output[at+3]=(uint8_t)(a/count);
        }
    }
}
CoveGlyphBitmap cove_text_glyph_bitmap(void *value,uint32_t glyph,int color,int phase) {
    CoveFace *face=value; CoveGlyphBitmap result={0};
    if (color) {
        if (!ensure_color(face)) return result;
    } else {
        ensure_normal(face);
    }
    const int has_outline_advance = color && FT_IS_SCALABLE(face->face);
    float color_advance = 0;
    if (has_outline_advance) {
        /* Shaping uses the unhinted advance. The bitmap supplies only the ink bounds. */
        color_advance = (float)hb_font_get_glyph_h_advance(face->font, glyph) / 64 / face->scale;
    }
    const FT_Int32 flags = color ? FT_LOAD_COLOR : FT_LOAD_FORCE_AUTOHINT | FT_LOAD_TARGET_LIGHT;
    if (FT_Load_Glyph(face->face, glyph, flags) || !face->face->glyph) return result;
    const FT_GlyphSlot slot = face->face->glyph;
    const int bitmap_strike = slot->format == FT_GLYPH_FORMAT_BITMAP;
    phase &= 3;
    if (!color && phase != 0 && slot->format == FT_GLYPH_FORMAT_OUTLINE) {
        /* Apply the horizontal phase after hinting. A quarter pixel is 16 in 26.6. */
        FT_Outline_Translate(&slot->outline, (FT_Pos)(phase * 16), 0);
    }
    if (!bitmap_strike && FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL)) return result;
    const FT_Bitmap bitmap=slot->bitmap; const uint32_t w=bitmap.width,h=bitmap.rows;
    result.metrics=bitmap_metrics(slot,(color ? face->strike_scale : 1)/face->scale); result.valid=1;
    if (has_outline_advance) result.metrics.advance = color_advance;
    if (!color && bitmap_strike && phase >= 2) {
        /* The Rust emitter anchors at the start of the pixel. Round fixed bitmap strikes. */
        result.metrics.bearing_x += 1.0f / face->scale;
    }
    if (!w || !h || !bitmap.buffer) return result;
    if (w>16384 || h>16384) { result.valid=0; return result; }
    const size_t channels=color ? 4 : 1; size_t length=(size_t)w*h*channels;
    uint8_t *pixels=calloc(length,1); if (!pixels) { result.valid=0; return result; }
    const size_t pitch=(size_t)(bitmap.pitch<0 ? -(int64_t)bitmap.pitch : bitmap.pitch);
    const size_t required=bitmap.pixel_mode==FT_PIXEL_MODE_BGRA ? (size_t)w*4 : bitmap.pixel_mode==FT_PIXEL_MODE_MONO ? ((size_t)w+7)/8 : w;
    if (pitch<required) { free(pixels); result.valid=0; return result; }
    for (uint32_t y=0;y<h;++y) { const uint8_t *row=bitmap.buffer+(bitmap.pitch>=0 ? (size_t)y*pitch : (size_t)(h-1-y)*pitch);
        for (uint32_t x=0;x<w;++x) {
            if (color) { const size_t at=((size_t)y*w+x)*4;
                if (bitmap.pixel_mode==FT_PIXEL_MODE_BGRA) { unsigned alpha=row[x*4+3];
                    pixels[at]=alpha ? (uint8_t)fmin(255,(row[x*4+2]*255u+alpha/2)/alpha) : 0;
                    pixels[at+1]=alpha ? (uint8_t)fmin(255,(row[x*4+1]*255u+alpha/2)/alpha) : 0;
                    pixels[at+2]=alpha ? (uint8_t)fmin(255,(row[x*4]*255u+alpha/2)/alpha) : 0; pixels[at+3]=(uint8_t)alpha;
                } else if (bitmap.pixel_mode==FT_PIXEL_MODE_GRAY) pixels[at+3]=row[x];
                else { free(pixels); result.valid=0; return result; }
            } else { if (bitmap.pixel_mode==FT_PIXEL_MODE_GRAY) pixels[(size_t)y*w+x]=row[x];
                else if (bitmap.pixel_mode==FT_PIXEL_MODE_MONO) pixels[(size_t)y*w+x]=(row[x/8]&(0x80u>>(x%8))) ? 255 : 0;
                else { free(pixels); result.valid=0; return result; } }
        }
    }
    result.width=w; result.height=h; result.pixels=pixels; result.length=length;
    if (color && face->strike_scale!=1) {
        const double dw=fmax(1,round((double)w*face->strike_scale)),dh=fmax(1,round((double)h*face->strike_scale));
        if (dw>16384 || dh>16384) { free(pixels); result.pixels=NULL; result.length=0; result.valid=0; return result; }
        result.width=(uint32_t)dw; result.height=(uint32_t)dh; result.length=(size_t)result.width*result.height*4;
        result.pixels=calloc(result.length,1); if (!result.pixels) { free(pixels); result.valid=0; return result; }
        cove_text_resample_rgba(pixels,w,h,result.pixels,result.width,result.height); free(pixels);
        result.metrics.width=(float)result.width/face->scale; result.metrics.height=(float)result.height/face->scale;
    } return result;
}
CoveShapedRun cove_text_shape(void *value,const uint8_t *text,size_t length,uint32_t direction,uint32_t script,const char *language,uint32_t offset,int64_t font_id) {
    CoveShapedRun result={0}; CoveFace *face=value; if (length>INT_MAX || length>UINT32_MAX-offset) return result; ensure_normal(face);
    hb_buffer_t *buffer=hb_buffer_create(); if (!buffer) return result;
    hb_buffer_add_utf8(buffer,(const char*)text,(int)length,0,(int)length);
    if (direction) hb_buffer_set_direction(buffer,(hb_direction_t)direction); if (script) hb_buffer_set_script(buffer,(hb_script_t)script);
    if (language) hb_buffer_set_language(buffer,hb_language_from_string(language,-1)); hb_buffer_guess_segment_properties(buffer); hb_shape(face->font,buffer,NULL,0);
    unsigned count=0; hb_glyph_info_t *infos=hb_buffer_get_glyph_infos(buffer,&count); hb_glyph_position_t *positions=hb_buffer_get_glyph_positions(buffer,&count);
    if (infos && positions && count<=SIZE_MAX/sizeof(CoveShapedGlyph)) {
        result.glyphs=calloc(count,sizeof(CoveShapedGlyph)); if (result.glyphs) { result.length=count;
            for (unsigned i=0;i<count;++i) result.glyphs[i]=(CoveShapedGlyph){infos[i].codepoint,(float)positions[i].x_offset/64/face->scale,
                (float)positions[i].y_offset/64/face->scale,(float)positions[i].x_advance/64/face->scale,(float)positions[i].y_advance/64/face->scale,infos[i].cluster+offset,font_id}; }
    }
    hb_buffer_destroy(buffer); return result;
}
uint32_t cove_text_script(uint32_t scalar) { return (uint32_t)hb_unicode_script(hb_unicode_funcs_get_default(),scalar); }
