/* Apple branches from FontProvider.swift and NativeColorGlyphFont.swift. */
#import <Foundation/Foundation.h>
#import <CoreText/CoreText.h>
#import <CoreGraphics/CoreGraphics.h>
#include "native.h"
#include <limits.h>
#include <math.h>
#include <stdlib.h>
#include <string.h>

static char *copy_utf8(NSString *value) { const char *text=value.UTF8String; if (!text) return NULL; const size_t n=strlen(text)+1; char *copy=malloc(n); if (copy) memcpy(copy,text,n); return copy; }
static NSString *text_string(const uint8_t *bytes,size_t length) { return [[NSString alloc] initWithBytes:bytes length:length encoding:NSUTF8StringEncoding]; }
void *cove_apple_font_new(const char *name,float size,float weight,int italic) {
    @autoreleasepool {
        NSString *requested=name ? [NSString stringWithUTF8String:name] : nil;
        CTFontRef base=(!requested || [requested isEqualToString:@".AppleSystemUIFont"])
            ? CTFontCreateUIFontForLanguage(kCTFontUIFontSystem,size,NULL) : CTFontCreateWithName((__bridge CFStringRef)requested,size,NULL);
        if (!base) base=CTFontCreateWithName(CFSTR("Helvetica Neue"),size,NULL);
        if (weight!=0) { NSDictionary *traits=@{(__bridge NSString*)kCTFontWeightTrait:@(weight)};
            NSDictionary *attributes=@{(__bridge NSString*)kCTFontTraitsAttribute:traits};
            CTFontDescriptorRef descriptor=CTFontCopyFontDescriptor(base);
            CTFontDescriptorRef modified=CTFontDescriptorCreateCopyWithAttributes(descriptor,(__bridge CFDictionaryRef)attributes);
            CTFontRef weighted=CTFontCreateWithFontDescriptor(modified,size,NULL); CFRelease(modified); CFRelease(descriptor);
            if (weighted) { CFRelease(base); base=weighted; }
        }
        if (italic) { CTFontRef styled=CTFontCreateCopyWithSymbolicTraits(base,size,NULL,kCTFontItalicTrait,kCTFontItalicTrait); if (styled) { CFRelease(base); base=styled; } }
        return (void*)base;
    }
}
void *cove_apple_font_copy(void *font,float size) { return (void*)CTFontCreateCopyWithAttributes((CTFontRef)font,size,NULL,NULL); }
void cove_apple_font_free(void *font) { if (font) CFRelease(font); }
char *cove_apple_font_name(void *font,int family) {
    @autoreleasepool { CFStringRef value=family==2 ? CTFontCopyName((CTFontRef)font,kCTFontSubFamilyNameKey) : family ? CTFontCopyFamilyName((CTFontRef)font) : CTFontCopyPostScriptName((CTFontRef)font);
        char *result=copy_utf8((__bridge NSString*)value); if (value) CFRelease(value); return result; }
}
char *cove_apple_font_path(void *font) {
    @autoreleasepool { CTFontDescriptorRef descriptor=CTFontCopyFontDescriptor((CTFontRef)font);
        CFTypeRef attribute=CTFontDescriptorCopyAttribute(descriptor,kCTFontURLAttribute); CFRelease(descriptor);
        if (!attribute) return NULL; char *result=NULL;
        if (CFGetTypeID(attribute)==CFURLGetTypeID()) result=copy_utf8([(__bridge NSURL*)attribute path]);
        CFRelease(attribute); return result; }
}
size_t cove_apple_font_variations(void *font,uint32_t *tags,double *values,size_t capacity) {
    @autoreleasepool { CFDictionaryRef variation=CTFontCopyVariation((CTFontRef)font); if (!variation) return 0;
        NSDictionary *dict=(__bridge NSDictionary*)variation; const size_t count=dict.count; size_t index=0;
        for (NSNumber *key in dict) { if (index<capacity && tags && values) { tags[index]=key.unsignedIntValue; values[index]=[dict[key] doubleValue]; } ++index; }
        CFRelease(variation); return count; }
}
void *cove_apple_font_fallback(void *font,const uint8_t *bytes,size_t length,size_t location,size_t range_length) {
    @autoreleasepool { NSString *string=text_string(bytes,length); if (!string || location>string.length || range_length>string.length-location) return NULL;
        return (void*)CTFontCreateForString((CTFontRef)font,(__bridge CFStringRef)string,CFRangeMake((CFIndex)location,(CFIndex)range_length)); }
}
static int glyph_bounds(CTFontRef font,float scale,uint32_t index,CGRect *output) {
    if (index>UINT16_MAX || !isfinite(scale) || scale<=0) return 0; CGGlyph glyph=(CGGlyph)index;
    CGRect bounds=CTFontGetBoundingRectsForGlyphs(font,kCTFontOrientationHorizontal,&glyph,NULL,1);
    bounds=CGRectApplyAffineTransform(bounds,CGAffineTransformMakeScale(scale,scale));
    if (!isfinite(CGRectGetMinX(bounds)) || !isfinite(CGRectGetMinY(bounds)) || !isfinite(bounds.size.width) || !isfinite(bounds.size.height) || CGRectIsEmpty(bounds)) return 0;
    *output=CGRectMake(floor(CGRectGetMinX(bounds))-1,floor(CGRectGetMinY(bounds))-1,ceil(CGRectGetMaxX(bounds))-floor(CGRectGetMinX(bounds))+2,ceil(CGRectGetMaxY(bounds))-floor(CGRectGetMinY(bounds))+2); return 1;
}
int cove_apple_color_metrics(void *value,float scale,uint32_t index,CoveGlyphMetrics *result) {
    CGRect bounds; CTFontRef font=(CTFontRef)value; if (!glyph_bounds(font,scale,index,&bounds)) return 0;
    CGGlyph glyph=(CGGlyph)index; CGSize advance=CGSizeZero; CTFontGetAdvancesForGlyphs(font,kCTFontOrientationHorizontal,&glyph,&advance,1);
    *result=(CoveGlyphMetrics){index,(float)(bounds.size.width/scale),(float)(bounds.size.height/scale),(float)(CGRectGetMinX(bounds)/scale),(float)(CGRectGetMaxY(bounds)/scale),(float)advance.width}; return 1;
}
CoveLineMetrics cove_apple_line_metrics(void *value) {
    CTFontRef font=(CTFontRef)value; const float ascent=(float)CTFontGetAscent(font),descent=(float)CTFontGetDescent(font);
    return (CoveLineMetrics){ascent,descent,ascent+descent+(float)CTFontGetLeading(font)};
}
CoveGlyphBitmap cove_apple_color_bitmap(void *value,float scale,uint32_t index) {
    CoveGlyphBitmap result={0}; CGRect bounds; CTFontRef font=(CTFontRef)value;
    if (!glyph_bounds(font,scale,index,&bounds) || !cove_apple_color_metrics(value,scale,index,&result.metrics) || bounds.size.width>4096 || bounds.size.height>4096) return result;
    result.width=(uint32_t)bounds.size.width; result.height=(uint32_t)bounds.size.height; result.length=(size_t)result.width*result.height*4;
    result.pixels=calloc(result.length,1); if (!result.pixels) return result;
    CGColorSpaceRef space=CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
    CGContextRef context=CGBitmapContextCreate(result.pixels,result.width,result.height,8,result.width*4,space,kCGBitmapByteOrder32Big|kCGImageAlphaPremultipliedLast); CGColorSpaceRelease(space);
    if (!context) { free(result.pixels); result.pixels=NULL; result.length=0; return result; }
    CGContextTranslateCTM(context,-CGRectGetMinX(bounds),-CGRectGetMinY(bounds)); CGContextScaleCTM(context,scale,scale);
    CGGlyph glyph=(CGGlyph)index; CGPoint position=CGPointZero; CTFontDrawGlyphs(font,&glyph,&position,1,context); CGContextRelease(context);
    for (size_t offset=0;offset<result.length;offset+=4) { const unsigned alpha=result.pixels[offset+3]; if (!alpha) continue;
        for (size_t channel=0;channel<3;++channel) { unsigned value=(result.pixels[offset+channel]*255u+alpha/2)/alpha; result.pixels[offset+channel]=(uint8_t)(value>255 ? 255 : value); } }
    result.valid=1; return result;
}
CoveShapedRun cove_apple_shape(void *value,const uint8_t *bytes,size_t length,uint32_t offset,int64_t font_id) {
    CoveShapedRun result={0}; if (length>UINT32_MAX-offset) return result;
    @autoreleasepool {
        NSString *string=text_string(bytes,length); if (!string) return result;
        const size_t units=string.length; if (units>SIZE_MAX/sizeof(uint32_t)-1) return result;
        uint32_t *byte_by_unit=malloc((units+1)*sizeof(uint32_t)); if (!byte_by_unit) return result;
        for (size_t i=0;i<=units;++i) byte_by_unit[i]=offset;
        for (size_t i=0,unit=0;i<length;) { byte_by_unit[unit]=(uint32_t)i+offset; const uint8_t first=bytes[i];
            const size_t width=first<0x80 ? 1 : first<0xe0 ? 2 : first<0xf0 ? 3 : 4; unit+=width==4 ? 2 : 1; i+=width; }
        NSAttributedString *attributed=[[NSAttributedString alloc] initWithString:string attributes:@{(__bridge NSString*)kCTFontAttributeName:(__bridge id)value}];
        CTLineRef line=CTLineCreateWithAttributedString((__bridge CFAttributedStringRef)attributed); CFArrayRef runs=CTLineGetGlyphRuns(line); const CFIndex count=CFArrayGetCount(runs);
        size_t total=0; for (CFIndex i=0;i<count;++i) { const CFIndex glyphs=CTRunGetGlyphCount((CTRunRef)CFArrayGetValueAtIndex(runs,i));
            if (glyphs<0 || (size_t)glyphs>SIZE_MAX/sizeof(CoveShapedGlyph)-total) { CFRelease(line); free(byte_by_unit); return result; } total+=(size_t)glyphs; }
        result.glyphs=calloc(total,sizeof(CoveShapedGlyph)); if (!result.glyphs) { CFRelease(line); free(byte_by_unit); return result; }
        CGFloat pen_x=0;
        for (CFIndex r=0;r<count;++r) {
            CTRunRef run=(CTRunRef)CFArrayGetValueAtIndex(runs,r); const CFIndex n=CTRunGetGlyphCount(run);
            CGGlyph *glyphs=calloc((size_t)n,sizeof(CGGlyph)); CFIndex *indices=calloc((size_t)n,sizeof(CFIndex));
            CGSize *advances=calloc((size_t)n,sizeof(CGSize)); CGPoint *positions=calloc((size_t)n,sizeof(CGPoint));
            if (!glyphs || !indices || !advances || !positions) { free(glyphs); free(indices); free(advances); free(positions); free(result.glyphs); result=(CoveShapedRun){0}; break; }
            CTRunGetGlyphs(run,CFRangeMake(0,0),glyphs); CTRunGetStringIndices(run,CFRangeMake(0,0),indices);
            CTRunGetAdvances(run,CFRangeMake(0,0),advances); CTRunGetPositions(run,CFRangeMake(0,0),positions);
            for (CFIndex i=0;i<n;++i) { result.glyphs[result.length++]=(CoveShapedGlyph){glyphs[i],(float)(positions[i].x-pen_x),-(float)positions[i].y,(float)advances[i].width,0,
                    indices[i]>=0 && (size_t)indices[i]<units ? byte_by_unit[indices[i]] : offset,font_id}; pen_x+=advances[i].width; }
            free(glyphs); free(indices); free(advances); free(positions);
        }
        CFRelease(line); free(byte_by_unit); return result;
    }
}
