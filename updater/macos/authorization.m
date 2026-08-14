#import <Foundation/Foundation.h>
#import <AppKit/AppKit.h>
#import <Security/Security.h>
#import <ServiceManagement/ServiceManagement.h>
#include <string.h>

enum {
    SuperiorityAuthorizationSuccess = 0,
    SuperiorityAuthorizationCancelled = 1,
    SuperiorityAuthorizationFailed = 2,
};

int superiority_updater_launch_privileged_worker(
    const char *worker_path,
    const char *plan_path,
    const char *result_path,
    const char *job_label,
    const char *authorization_prompt,
    const char *application_path)
{
    @autoreleasepool {
        if (worker_path == NULL || plan_path == NULL || result_path == NULL ||
            job_label == NULL || authorization_prompt == NULL ||
            application_path == NULL) {
            return SuperiorityAuthorizationFailed;
        }

        NSString *worker = [NSString stringWithUTF8String:worker_path];
        NSString *plan = [NSString stringWithUTF8String:plan_path];
        NSString *result = [NSString stringWithUTF8String:result_path];
        NSString *label = [NSString stringWithUTF8String:job_label];
        NSString *prompt = [NSString stringWithUTF8String:authorization_prompt];
        NSString *application = [NSString stringWithUTF8String:application_path];
        if (worker == nil || plan == nil || result == nil || label == nil ||
            prompt == nil || application == nil) {
            return SuperiorityAuthorizationFailed;
        }

        AuthorizationRef authorization = NULL;
        if (AuthorizationCreate(NULL, kAuthorizationEmptyEnvironment,
                                kAuthorizationFlagDefaults, &authorization) !=
            errAuthorizationSuccess || authorization == NULL) {
            return SuperiorityAuthorizationFailed;
        }

        AuthorizationItem item = {
            .name = kSMRightModifySystemDaemons,
            .valueLength = 0,
            .value = NULL,
            .flags = 0,
        };
        AuthorizationRights rights = { .count = 1, .items = &item };
        const char *promptValue = prompt.UTF8String;
        NSString *icon = [[NSBundle bundleWithPath:application]
            pathForResource:@"Superiority" ofType:@"icns"];
        const char *iconValue = icon.fileSystemRepresentation;
        AuthorizationItem environmentItems[] = {
            {
                .name = kAuthorizationEnvironmentPrompt,
                .valueLength = promptValue == NULL ? 0 : strlen(promptValue),
                .value = (void *)promptValue,
                .flags = 0,
            },
            {
                .name = kAuthorizationEnvironmentIcon,
                .valueLength = iconValue == NULL ? 0 : strlen(iconValue),
                .value = (void *)iconValue,
                .flags = 0,
            },
        };
        AuthorizationEnvironment environment = {
            .count = iconValue == NULL ? 1 : 2,
            .items = environmentItems,
        };
        AuthorizationFlags flags =
            (AuthorizationFlags)(kAuthorizationFlagExtendRights |
                                 kAuthorizationFlagInteractionAllowed);
        OSStatus authorizationStatus = AuthorizationCopyRights(
            authorization, &rights, &environment, flags, NULL);
        if (authorizationStatus != errAuthorizationSuccess) {
            AuthorizationFree(authorization, kAuthorizationFlagDefaults);
            return authorizationStatus == errAuthorizationCanceled
                ? SuperiorityAuthorizationCancelled
                : SuperiorityAuthorizationFailed;
        }

        NSArray<NSString *> *arguments = @[
            worker,
            @"--execute-plan",
            plan,
            @"--result",
            result,
        ];
        NSDictionary *job = @{
            @"Label": label,
            @"ProgramArguments": arguments,
            @"RunAtLoad": @YES,
            @"LaunchOnlyOnce": @YES,
            @"EnableTransactions": @NO,
            @"ProcessType": @"Interactive",
        };

        CFErrorRef removeError = NULL;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        SMJobRemove(kSMDomainSystemLaunchd,
                    (__bridge CFStringRef)label,
                    authorization,
                    true,
                    &removeError);
#pragma clang diagnostic pop
        if (removeError != NULL) {
            CFRelease(removeError);
        }

        CFErrorRef submitError = NULL;
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
        Boolean submitted = SMJobSubmit(
            kSMDomainSystemLaunchd,
            (__bridge CFDictionaryRef)job,
            authorization,
            &submitError);
#pragma clang diagnostic pop
        if (submitError != NULL) {
            CFRelease(submitError);
        }
        AuthorizationFree(authorization, kAuthorizationFlagDefaults);
        return submitted ? SuperiorityAuthorizationSuccess
                         : SuperiorityAuthorizationFailed;
    }
}

void superiority_updater_show_progress(const char *application_name,
                                       const char *application_path)
{
    @autoreleasepool {
        NSString *name = application_name == NULL
            ? @"Superiority"
            : [NSString stringWithUTF8String:application_name];
        NSString *path = application_path == NULL
            ? nil
            : [NSString stringWithUTF8String:application_path];
        if (name == nil) {
            name = @"Superiority";
        }

        NSApplication *application = NSApplication.sharedApplication;
        [application setActivationPolicy:NSApplicationActivationPolicyRegular];
        if (path != nil) {
            application.applicationIconImage = [NSWorkspace.sharedWorkspace iconForFile:path];
        }

        NSRect frame = NSMakeRect(0.0, 0.0, 420.0, 150.0);
        NSWindow *window = [[NSWindow alloc]
            initWithContentRect:frame
                      styleMask:(NSWindowStyleMaskTitled)
                        backing:NSBackingStoreBuffered
                          defer:NO];
        window.title = [@"Updating " stringByAppendingString:name];
        window.releasedWhenClosed = NO;

        NSTextField *status = [NSTextField labelWithString:@"Installing update…"];
        status.frame = NSMakeRect(28.0, 96.0, 364.0, 24.0);
        status.font = [NSFont systemFontOfSize:13.0 weight:NSFontWeightSemibold];

        NSProgressIndicator *progress = [[NSProgressIndicator alloc]
            initWithFrame:NSMakeRect(28.0, 65.0, 364.0, 14.0)];
        progress.indeterminate = YES;
        progress.style = NSProgressIndicatorStyleBar;
        [progress startAnimation:nil];

        NSButton *cancel = [[NSButton alloc]
            initWithFrame:NSMakeRect(274.0, 20.0, 118.0, 32.0)];
        cancel.title = @"Cancel Update";
        cancel.bezelStyle = NSBezelStyleRounded;
        cancel.enabled = NO;

        [window.contentView addSubview:status];
        [window.contentView addSubview:progress];
        [window.contentView addSubview:cancel];
        [window center];
        [window makeKeyAndOrderFront:nil];
        [application activate];
        [application run];
    }
}
