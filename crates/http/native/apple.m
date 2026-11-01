// NSURLSession data task exposed as a blocking, streamed C API.
//
// `cove_http_apple_send` returns once the response head arrives; body bytes
// are buffered by the delegate and handed out by `cove_http_apple_read`. The
// task is suspended while the buffer is above its high-water mark so a slow
// reader bounds memory.

#import <Foundation/Foundation.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

enum { COVE_OK = 0, COVE_ERROR = -1, COVE_TIMEOUT = -2, COVE_REDIRECTS = -3, COVE_URL = -4 };

static const NSUInteger HighWater = 4 << 20;

@interface CoveHTTPTask : NSObject <NSURLSessionDataDelegate>
@property(nonatomic, strong) NSCondition *condition;
@property(nonatomic, strong) NSMutableData *buffer;
@property(nonatomic, strong) NSHTTPURLResponse *response;
@property(nonatomic, strong) NSError *error;
@property(nonatomic, strong) NSURLSession *session;
@property(nonatomic, strong) NSURLSessionDataTask *task;
@property(nonatomic) BOOL finished;
@property(nonatomic) BOOL suspended;
@property(nonatomic) BOOL tooManyRedirects;
@property(nonatomic) long redirects;
@property(nonatomic) long maximumRedirects;
@property(nonatomic, copy) NSArray<NSString *> *headerNames;
@property(nonatomic, copy) NSArray<NSString *> *headerValues;
@end

@implementation CoveHTTPTask

- (void)URLSession:(NSURLSession *)session
                          task:(NSURLSessionTask *)task
    willPerformHTTPRedirection:(NSHTTPURLResponse *)response
                    newRequest:(NSURLRequest *)request
             completionHandler:(void (^)(NSURLRequest *))completionHandler {
    if (self.maximumRedirects == 0) {
        completionHandler(nil);
        return;
    }
    self.redirects += 1;
    if (self.redirects > self.maximumRedirects) {
        [self.condition lock];
        self.tooManyRedirects = YES;
        [self.condition unlock];
        completionHandler(nil);
        [task cancel];
        return;
    }
    NSURL *from = task.originalRequest.URL;
    NSURL *to = request.URL;
    BOOL sameOrigin = [from.scheme caseInsensitiveCompare:to.scheme] == NSOrderedSame &&
                      [from.host caseInsensitiveCompare:to.host] == NSOrderedSame &&
                      ((from.port == nil && to.port == nil) || [from.port isEqual:to.port]);
    if (sameOrigin) {
        completionHandler(request);
        return;
    }
    NSMutableURLRequest *next = [request mutableCopy];
    for (NSString *name in @[ @"Authorization", @"Cookie", @"Proxy-Authorization" ]) {
        [next setValue:nil forHTTPHeaderField:name];
    }
    completionHandler(next);
}

- (void)URLSession:(NSURLSession *)session
              dataTask:(NSURLSessionDataTask *)dataTask
    didReceiveResponse:(NSURLResponse *)response
     completionHandler:(void (^)(NSURLSessionResponseDisposition))completionHandler {
    [self.condition lock];
    if ([response isKindOfClass:[NSHTTPURLResponse class]]) {
        self.response = (NSHTTPURLResponse *)response;
    }
    [self.condition broadcast];
    [self.condition unlock];
    completionHandler(NSURLSessionResponseAllow);
}

- (void)URLSession:(NSURLSession *)session
          dataTask:(NSURLSessionDataTask *)dataTask
    didReceiveData:(NSData *)data {
    [self.condition lock];
    [self.buffer appendData:data];
    if (self.buffer.length > HighWater && !self.suspended) {
        self.suspended = YES;
        [dataTask suspend];
    }
    [self.condition broadcast];
    [self.condition unlock];
}

- (void)URLSession:(NSURLSession *)session
                    task:(NSURLSessionTask *)task
    didCompleteWithError:(NSError *)error {
    [self.condition lock];
    self.error = error;
    self.finished = YES;
    [self.condition broadcast];
    [self.condition unlock];
}

@end

static char *copy_string(NSString *text) {
    const char *utf8 = text.UTF8String;
    return strdup(utf8 ? utf8 : "");
}

static int classify(CoveHTTPTask *state, char **error) {
    if (state.tooManyRedirects) return COVE_REDIRECTS;
    NSError *failure = state.error;
    if (!failure) return COVE_ERROR;
    if (error) *error = copy_string(failure.localizedDescription);
    if ([failure.domain isEqualToString:NSURLErrorDomain] && failure.code == NSURLErrorTimedOut) {
        return COVE_TIMEOUT;
    }
    return COVE_ERROR;
}

/// Returns a retained task handle once the head arrives, or NULL with
/// `*result` set to a negative code and `*error` to a malloc'd message.
void *cove_http_apple_send(const char *method, const char *url, const char *const *names,
                           const char *const *values, size_t count, const uint8_t *body,
                           size_t body_length, double timeout, long maximum_redirects,
                           const char *user_agent, int *status, int *result, char **error) {
    @autoreleasepool {
        *status = 0;
        *result = COVE_OK;
        *error = NULL;
        NSString *text = [NSString stringWithUTF8String:url];
        NSURL *address = text ? [NSURL URLWithString:text] : nil;
        if (!address) {
            *result = COVE_URL;
            return NULL;
        }
        NSMutableURLRequest *request = [NSMutableURLRequest requestWithURL:address];
        request.HTTPMethod = [NSString stringWithUTF8String:method];
        request.timeoutInterval = timeout;
        [request setValue:[NSString stringWithUTF8String:user_agent] forHTTPHeaderField:@"User-Agent"];
        for (size_t index = 0; index < count; index++) {
            NSString *name = [NSString stringWithUTF8String:names[index]];
            NSString *value = [NSString stringWithUTF8String:values[index]];
            if (!name || !value) continue;
            [request addValue:value forHTTPHeaderField:name];
        }
        if (body_length > 0) request.HTTPBody = [NSData dataWithBytes:body length:body_length];

        NSURLSessionConfiguration *configuration =
            [NSURLSessionConfiguration ephemeralSessionConfiguration];
        configuration.timeoutIntervalForRequest = timeout;
        configuration.HTTPShouldSetCookies = NO;
        configuration.URLCache = nil;
        NSOperationQueue *queue = [[NSOperationQueue alloc] init];
        queue.maxConcurrentOperationCount = 1;

        CoveHTTPTask *state = [[CoveHTTPTask alloc] init];
        state.condition = [[NSCondition alloc] init];
        state.buffer = [NSMutableData data];
        state.maximumRedirects = maximum_redirects;
        state.session = [NSURLSession sessionWithConfiguration:configuration
                                                      delegate:state
                                                 delegateQueue:queue];
        state.task = [state.session dataTaskWithRequest:request];
        [state.task resume];

        [state.condition lock];
        while (!state.response && !state.finished) [state.condition wait];
        NSHTTPURLResponse *response = state.response;
        BOOL tooManyRedirects = state.tooManyRedirects;
        [state.condition unlock];
        if (tooManyRedirects) {
            *result = COVE_REDIRECTS;
            [state.session invalidateAndCancel];
            return NULL;
        }
        if (!response) {
            *result = classify(state, error);
            if (*result == COVE_ERROR && !*error) *error = strdup("the response was not HTTP");
            [state.session invalidateAndCancel];
            return NULL;
        }
        *status = (int)response.statusCode;
        NSMutableArray<NSString *> *headerNames = [NSMutableArray array];
        NSMutableArray<NSString *> *headerValues = [NSMutableArray array];
        [response.allHeaderFields enumerateKeysAndObjectsUsingBlock:^(id key, id value, BOOL *stop) {
            [headerNames addObject:[key description]];
            [headerValues addObject:[value description]];
            (void)stop;
        }];
        state.headerNames = headerNames;
        state.headerValues = headerValues;
        return (__bridge_retained void *)state;
    }
}

size_t cove_http_apple_header_count(void *handle) {
    CoveHTTPTask *state = (__bridge CoveHTTPTask *)handle;
    return state.headerNames.count;
}

/// malloc'd copies; free with `cove_http_apple_free`.
char *cove_http_apple_header_name(void *handle, size_t index) {
    @autoreleasepool {
        return copy_string(((__bridge CoveHTTPTask *)handle).headerNames[index]);
    }
}

char *cove_http_apple_header_value(void *handle, size_t index) {
    @autoreleasepool {
        return copy_string(((__bridge CoveHTTPTask *)handle).headerValues[index]);
    }
}

char *cove_http_apple_url(void *handle) {
    @autoreleasepool {
        CoveHTTPTask *state = (__bridge CoveHTTPTask *)handle;
        return copy_string(state.response.URL.absoluteString ?: @"");
    }
}

/// Bytes copied (> 0), 0 at end of body, or a negative code with `*error`.
long cove_http_apple_read(void *handle, uint8_t *buffer, size_t length, char **error) {
    @autoreleasepool {
        *error = NULL;
        CoveHTTPTask *state = (__bridge CoveHTTPTask *)handle;
        [state.condition lock];
        while (state.buffer.length == 0 && !state.finished) [state.condition wait];
        size_t available = state.buffer.length;
        if (available == 0) {
            int code = state.error ? classify(state, error) : COVE_OK;
            [state.condition unlock];
            return code;
        }
        size_t count = available < length ? available : length;
        memcpy(buffer, state.buffer.bytes, count);
        [state.buffer replaceBytesInRange:NSMakeRange(0, count) withBytes:NULL length:0];
        if (state.suspended && state.buffer.length < HighWater / 2) {
            state.suspended = NO;
            [state.task resume];
        }
        [state.condition unlock];
        return (long)count;
    }
}

void cove_http_apple_close(void *handle) {
    @autoreleasepool {
        CoveHTTPTask *state = (__bridge_transfer CoveHTTPTask *)handle;
        [state.session invalidateAndCancel];
    }
}

void cove_http_apple_free(char *text) { free(text); }
