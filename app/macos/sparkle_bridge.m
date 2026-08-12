#import <AppKit/AppKit.h>
#import <Sparkle/Sparkle.h>

#import "sparkle_bridge.h"

@protocol SuperioritySparkleEventSink <NSObject>
- (void)superioritySparkleEvent:(NSString *)eventJSON;
@end

static NSString *SuperiorityNotesFormat(NSString *format)
{
    NSString *normalized = format.lowercaseString;
    if ([normalized containsString:@"markdown"]) {
        return @"markdown";
    }
    if ([normalized containsString:@"html"]) {
        return @"html";
    }
    return @"plain-text";
}

@interface SuperioritySparkleDriver : NSObject <SPUUserDriver>
@property(nonatomic, weak) id<SuperioritySparkleEventSink> sink;
@property(nonatomic, copy, nullable) void (^updateReply)(SPUUserUpdateChoice);
@property(nonatomic, copy, nullable) void (^readyReply)(SPUUserUpdateChoice);
@property(nonatomic, copy, nullable) void (^cancellation)(void);
@property(nonatomic) uint64_t expectedLength;
@property(nonatomic) uint64_t receivedLength;
@end

@implementation SuperioritySparkleDriver

- (instancetype)initWithSink:(id<SuperioritySparkleEventSink>)sink
{
    self = [super init];
    if (self != nil) {
        _sink = sink;
    }
    return self;
}

- (void)emit:(NSDictionary<NSString *, id> *)event
{
    NSData *data = [NSJSONSerialization dataWithJSONObject:event options:0 error:nil];
    if (data == nil) {
        return;
    }
    NSString *json = [[NSString alloc] initWithData:data encoding:NSUTF8StringEncoding];
    [self.sink superioritySparkleEvent:json];
}

- (void)showUpdatePermissionRequest:(SPUUpdatePermissionRequest *)request
                              reply:(void (^)(SUUpdatePermissionResponse *))reply
{
    (void)request;
    SUUpdatePermissionResponse *response = [[SUUpdatePermissionResponse alloc]
        initWithAutomaticUpdateChecks:NO
        sendSystemProfile:NO];
    reply(response);
}

- (void)showUserInitiatedUpdateCheckWithCancellation:(void (^)(void))cancellation
{
    self.cancellation = cancellation;
    [self emit:@{@"kind": @"checking"}];
}

- (void)showUpdateFoundWithAppcastItem:(SUAppcastItem *)item
                                 state:(SPUUserUpdateState *)state
                                 reply:(void (^)(SPUUserUpdateChoice))reply
{
    (void)state;
    self.updateReply = reply;
    self.cancellation = nil;
    NSString *notes = item.itemDescription.length > 0
        ? item.itemDescription
        : @"Release notes are not available for this update.";
    [self emit:@{
        @"kind": @"available",
        @"version": item.displayVersionString,
        @"title": item.title ?: @"Superiority update",
        @"notes": notes,
        @"notes_format": SuperiorityNotesFormat(item.itemDescriptionFormat),
        @"size": @(item.contentLength),
    }];
}

- (void)showUpdateReleaseNotesWithDownloadData:(SPUDownloadData *)downloadData
{
    NSString *notes = [[NSString alloc] initWithData:downloadData.data encoding:NSUTF8StringEncoding];
    [self emit:@{
        @"kind": @"notes",
        @"notes": notes ?: @"Release notes are not available for this update.",
        @"notes_format": SuperiorityNotesFormat(downloadData.MIMEType),
    }];
}

- (void)showUpdateReleaseNotesFailedToDownloadWithError:(NSError *)error
{
    [self emit:@{
        @"kind": @"notes",
        @"notes": error.localizedDescription ?: @"Release notes could not be downloaded.",
    }];
}

- (void)showUpdateNotFoundWithError:(NSError *)error acknowledgement:(void (^)(void))acknowledgement
{
    self.cancellation = nil;
    [self emit:@{
        @"kind": @"not_found",
        @"message": error.localizedRecoverySuggestion ?: error.localizedDescription ?: @"Superiority is up to date.",
    }];
    acknowledgement();
}

- (void)showUpdaterError:(NSError *)error acknowledgement:(void (^)(void))acknowledgement
{
    self.cancellation = nil;
    [self emit:@{
        @"kind": @"error",
        @"message": error.localizedRecoverySuggestion ?: error.localizedDescription ?: @"The update could not be completed.",
    }];
    acknowledgement();
}

- (void)showDownloadInitiatedWithCancellation:(void (^)(void))cancellation
{
    self.cancellation = cancellation;
    self.receivedLength = 0;
    [self emit:@{@"kind": @"downloading", @"progress": @0.0}];
}

- (void)showDownloadDidReceiveExpectedContentLength:(uint64_t)expectedContentLength
{
    self.expectedLength = expectedContentLength;
}

- (void)showDownloadDidReceiveDataOfLength:(uint64_t)length
{
    self.receivedLength += length;
    double progress = self.expectedLength == 0
        ? 0.0
        : MIN(1.0, (double)self.receivedLength / (double)self.expectedLength);
    [self emit:@{@"kind": @"downloading", @"progress": @(progress)}];
}

- (void)showDownloadDidStartExtractingUpdate
{
    self.cancellation = nil;
    [self emit:@{@"kind": @"extracting", @"progress": @0.0}];
}

- (void)showExtractionReceivedProgress:(double)progress
{
    [self emit:@{@"kind": @"extracting", @"progress": @(progress)}];
}

- (void)showReadyToInstallAndRelaunch:(void (^)(SPUUserUpdateChoice))reply
{
    self.readyReply = reply;
    [self emit:@{@"kind": @"ready"}];
}

- (void)showInstallingUpdateWithApplicationTerminated:(BOOL)applicationTerminated
                         retryTerminatingApplication:(void (^)(void))retryTerminatingApplication
{
    (void)applicationTerminated;
    (void)retryTerminatingApplication;
    [self emit:@{@"kind": @"installing"}];
}

- (void)showUpdateInstalledAndRelaunched:(BOOL)relaunched acknowledgement:(void (^)(void))acknowledgement
{
    (void)relaunched;
    [self emit:@{@"kind": @"installed"}];
    acknowledgement();
}

- (void)dismissUpdateInstallation
{
    [self emit:@{@"kind": @"dismissed"}];
}

- (void)showUpdateInFocus
{
    [self emit:@{@"kind": @"focus"}];
}

- (void)performPrimaryAction
{
    if (self.readyReply != nil) {
        void (^reply)(SPUUserUpdateChoice) = self.readyReply;
        self.readyReply = nil;
        reply(SPUUserUpdateChoiceInstall);
    } else if (self.updateReply != nil) {
        void (^reply)(SPUUserUpdateChoice) = self.updateReply;
        self.updateReply = nil;
        reply(SPUUserUpdateChoiceInstall);
    }
}

- (void)dismiss
{
    if (self.readyReply != nil) {
        void (^reply)(SPUUserUpdateChoice) = self.readyReply;
        self.readyReply = nil;
        reply(SPUUserUpdateChoiceDismiss);
    } else if (self.updateReply != nil) {
        void (^reply)(SPUUserUpdateChoice) = self.updateReply;
        self.updateReply = nil;
        reply(SPUUserUpdateChoiceDismiss);
    } else if (self.cancellation != nil) {
        void (^cancellation)(void) = self.cancellation;
        self.cancellation = nil;
        cancellation();
    }
}

@end

@interface SuperioritySparkleController : NSObject
@property(nonatomic, strong) SuperioritySparkleDriver *driver;
@property(nonatomic, strong) SPUUpdater *updater;
@end

@implementation SuperioritySparkleController

- (instancetype)initWithEventSink:(id<SuperioritySparkleEventSink>)eventSink
{
    self = [super init];
    if (self == nil) {
        return nil;
    }

    _driver = [[SuperioritySparkleDriver alloc] initWithSink:eventSink];
    NSBundle *bundle = NSBundle.mainBundle;
    _updater = [[SPUUpdater alloc] initWithHostBundle:bundle
                                    applicationBundle:bundle
                                           userDriver:_driver
                                             delegate:nil];
    NSError *error = nil;
    if (![_updater startUpdater:&error]) {
        [_driver emit:@{
            @"kind": @"error",
            @"message": error.localizedDescription ?: @"Sparkle could not start.",
        }];
        return nil;
    }
    return self;
}

@end

void *superiority_sparkle_create(void *event_sink)
{
    id<SuperioritySparkleEventSink> sink = (__bridge id<SuperioritySparkleEventSink>)event_sink;
    SuperioritySparkleController *controller = [[SuperioritySparkleController alloc] initWithEventSink:sink];
    return (__bridge_retained void *)controller;
}

void superiority_sparkle_check(void *opaque_controller)
{
    SuperioritySparkleController *controller = (__bridge SuperioritySparkleController *)opaque_controller;
    [controller.updater checkForUpdates];
}

void superiority_sparkle_primary_action(void *opaque_controller)
{
    SuperioritySparkleController *controller = (__bridge SuperioritySparkleController *)opaque_controller;
    [controller.driver performPrimaryAction];
}

void superiority_sparkle_dismiss(void *opaque_controller)
{
    SuperioritySparkleController *controller = (__bridge SuperioritySparkleController *)opaque_controller;
    [controller.driver dismiss];
}

void superiority_sparkle_destroy(void *opaque_controller)
{
    if (opaque_controller != NULL) {
        CFBridgingRelease(opaque_controller);
    }
}

void superiority_sparkle_render_release_notes(void *opaque_text_view, const char *html_utf8)
{
    if (opaque_text_view == NULL || html_utf8 == NULL) {
        return;
    }
    NSTextView *textView = (__bridge NSTextView *)opaque_text_view;
    NSString *html = [NSString stringWithUTF8String:html_utf8];
    NSData *data = [html dataUsingEncoding:NSUTF8StringEncoding];
    NSDictionary *options = @{NSDocumentTypeDocumentOption: NSHTMLTextDocumentType};
    NSAttributedString *rendered = [[NSAttributedString alloc] initWithData:data
                                                                    options:options
                                                         documentAttributes:nil
                                                                      error:nil];
    if (rendered != nil) {
        NSMutableAttributedString *styled = [rendered mutableCopy];
        NSString *plainText = styled.string;
        NSUInteger location = 0;
        while (location < styled.length) {
            NSRange paragraphRange = [plainText paragraphRangeForRange:NSMakeRange(location, 0)];
            NSParagraphStyle *source = [styled attribute:NSParagraphStyleAttributeName
                                                  atIndex:location
                                           effectiveRange:nil];
            NSMutableParagraphStyle *paragraph = source != nil
                ? [source mutableCopy]
                : [[NSMutableParagraphStyle alloc] init];
            NSUInteger listDepth = paragraph.textLists.count;

            // the html importer emits literal markers and NSTextList metadata;
            // NSTextView otherwise paints both markers for the same list item
            if (listDepth > 0) {
                paragraph.textLists = @[];
                paragraph.lineSpacing = 4.0;
                paragraph.paragraphSpacing = listDepth > 1 ? 6.0 : 8.0;
            } else {
                NSFont *font = [styled attribute:NSFontAttributeName
                                         atIndex:location
                                  effectiveRange:nil];
                CGFloat pointSize = font.pointSize;
                paragraph.lineSpacing = 5.0;
                paragraph.paragraphSpacing = 14.0;
                if (pointSize >= 19.0) {
                    paragraph.paragraphSpacing = 20.0;
                } else if (pointSize >= 16.0) {
                    paragraph.paragraphSpacingBefore = 10.0;
                    paragraph.paragraphSpacing = 14.0;
                } else if (pointSize >= 14.0 && font.fontDescriptor.symbolicTraits & NSFontBoldTrait) {
                    paragraph.paragraphSpacingBefore = 8.0;
                    paragraph.paragraphSpacing = 12.0;
                }
            }
            [styled addAttribute:NSParagraphStyleAttributeName
                           value:paragraph
                           range:paragraphRange];
            location = NSMaxRange(paragraphRange);
        }
        [textView.textStorage setAttributedString:styled];
    }
}
