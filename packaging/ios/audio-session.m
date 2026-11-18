#import <AVFoundation/AVFoundation.h>
#import <dispatch/dispatch.h>
#include <stdbool.h>
#include <stdatomic.h>

static atomic_bool output_changed;

void urban_sprawl_ios_prepare_audio(void) {
    AVAudioSession *session = [AVAudioSession sharedInstance];
    static dispatch_once_t once;
    dispatch_once(&once, ^{
        NSNotificationCenter *center = [NSNotificationCenter defaultCenter];
        [center addObserverForName:AVAudioSessionRouteChangeNotification object:session
                             queue:nil usingBlock:^(NSNotification *notice) {
            NSInteger reason = [notice.userInfo[AVAudioSessionRouteChangeReasonKey] integerValue];
            if (reason == AVAudioSessionRouteChangeReasonNewDeviceAvailable ||
                reason == AVAudioSessionRouteChangeReasonOldDeviceUnavailable) {
                atomic_store(&output_changed, true);
            }
        }];
        [center addObserverForName:AVAudioSessionInterruptionNotification object:session
                             queue:nil usingBlock:^(NSNotification *notice) {
            NSInteger kind = [notice.userInfo[AVAudioSessionInterruptionTypeKey] integerValue];
            if (kind == AVAudioSessionInterruptionTypeEnded) {
                atomic_store(&output_changed, true);
            }
        }];
    });
    if (![session.category isEqualToString:AVAudioSessionCategoryAmbient]) {
        [session setCategory:AVAudioSessionCategoryAmbient error:NULL];
    }
    [session setActive:YES error:NULL];
}

void urban_sprawl_ios_pause_audio(void) {
    [[AVAudioSession sharedInstance] setActive:NO
        withOptions:AVAudioSessionSetActiveOptionNotifyOthersOnDeactivation error:NULL];
}

bool urban_sprawl_ios_audio_output_changed(void) {
    return atomic_exchange(&output_changed, false);
}
