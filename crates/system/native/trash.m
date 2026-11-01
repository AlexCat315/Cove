#import <Foundation/Foundation.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

char *cove_trash_item(const char *path, uint32_t *error) {
    @autoreleasepool {
        NSString *name = [NSString stringWithUTF8String:path];
        if (!name) { *error = 22; return NULL; }
        NSURL *destination = nil;
        NSError *failure = nil;
        BOOL success = [[NSFileManager defaultManager] trashItemAtURL:[NSURL fileURLWithPath:name]
            resultingItemURL:&destination error:&failure];
        if (!success) { *error = (uint32_t)(failure ? failure.code : 5); return NULL; }
        const char *result = destination.path.fileSystemRepresentation;
        *error = 0;
        return strdup(result ? result : path);
    }
}
