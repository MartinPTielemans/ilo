// What a WKWebView tells its host about a page's media, and what the host can read and do from a
// script world of its own, for the sidebar's player on a page in Safari's engine (#3432). It loads
// a page that plays a looped tone, names it with navigator.mediaSession and handles its actions,
// then logs each answer as "webkit-media: ...", with milliseconds since launch.
//
// Usage: probe <page url>. Exits 0 once every step has logged, 1 if it runs out of time.

#import <Cocoa/Cocoa.h>
#import <MediaPlayer/MediaPlayer.h>
#import <WebKit/WebKit.h>
#import <objc/runtime.h>

static CFAbsoluteTime started;

static void Log(NSString* format, ...) NS_FORMAT_FUNCTION(1, 2);
static void Log(NSString* format, ...) {
  va_list args;
  va_start(args, format);
  NSString* text = [[NSString alloc] initWithFormat:format arguments:args];
  va_end(args);
  printf("webkit-media: +%d ms %s\n", (int)((CFAbsoluteTimeGetCurrent() - started) * 1000),
         text.UTF8String);
  fflush(stdout);
}

// The browser's own world, as the system page's (core/platform/mac/system_page.mm).
static WKContentWorld* World(void) {
  return [WKContentWorld worldWithName:@"ilo"];
}

// Media events don't bubble, but a listener on the document in the capture phase hears each of
// its media elements'. Nothing runs between them.
static NSString* const kWatch =
    @"(() => {"
     "  const post = why => {"
     "    const s = navigator.mediaSession, m = s && s.metadata;"
     "    const media = [...document.querySelectorAll('audio,video')].map(e =>"
     "      (e.paused ? 'paused ' : 'playing ') + e.currentSrc.split('/').pop() + ' at ' +"
     "      e.currentTime.toFixed(1) + ' of ' + e.duration.toFixed(1));"
     "    webkit.messageHandlers.media.postMessage(why + ': ' + (s ? 'session ' + s.playbackState :"
     "      'no session') + ', ' + (m ? JSON.stringify({title: m.title, artist: m.artist,"
     "      album: m.album, artwork: [...m.artwork].map(a => a.src)}) : 'no metadata') +"
     "      ', callActionHandler ' + (s ? typeof s.callActionHandler : 'none') +"
     "      ', elements [' + media.join('; ') + ']');"
     "  };"
     "  for (const type of ['play', 'playing', 'pause', 'ended', 'emptied', 'loadedmetadata',"
     "                      'ratechange'])"
     "    document.addEventListener(type, () => post(type), true);"
     "  window.iloPost = post;"
     "})();";

@interface Probe : NSObject <WKScriptMessageHandler, WKNavigationDelegate>
@end

@implementation Probe {
  WKWebView* _web;
  NSWindow* _window;
  NSURL* _url;
  int _messages;
  int _audioChanges;
  BOOL _playing;
}

- (instancetype)initWithURL:(NSURL*)url {
  if ((self = [super init])) {
    _url = url;
  }
  return self;
}

- (void)start {
  Log(@"macOS %@", NSProcessInfo.processInfo.operatingSystemVersionString);
  [self listSelectors];
  WKWebViewConfiguration* config = [[WKWebViewConfiguration alloc] init];
  config.mediaTypesRequiringUserActionForPlayback = WKAudiovisualMediaTypeNone;
  [config.userContentController addScriptMessageHandler:self contentWorld:World() name:@"media"];
  [config.userContentController
      addUserScript:[[WKUserScript alloc] initWithSource:kWatch
                                           injectionTime:WKUserScriptInjectionTimeAtDocumentStart
                                        forMainFrameOnly:YES
                                          inContentWorld:World()]];
  _web = [[WKWebView alloc] initWithFrame:NSMakeRect(0, 0, 480, 320) configuration:config];
  _web.navigationDelegate = self;
  BOOL observable = [_web respondsToSelector:NSSelectorFromString(@"_isPlayingAudio")];
  Log(@"_isPlayingAudio %@", observable ? @"answers, observed" : @"absent");
  if (observable) {
    [_web addObserver:self forKeyPath:@"_isPlayingAudio" options:0 context:nil];
  }
  _window = [[NSWindow alloc] initWithContentRect:NSMakeRect(40, 40, 480, 320)
                                        styleMask:NSWindowStyleMaskTitled
                                          backing:NSBackingStoreBuffered
                                            defer:NO];
  _window.contentView = _web;
  [_window orderFrontRegardless];
  [_web loadRequest:[NSURLRequest requestWithURL:_url]];
  [self after:60 run:^{
    Log(@"FAIL: ran out of time");
    exit(1);
  }];
}

// WKWebView's methods, the private ones too, that name media, playback or Now Playing: what the
// host could ask of WebKit on this system.
- (void)listSelectors {
  NSMutableArray<NSString*>* names = [NSMutableArray array];
  for (Class c = [WKWebView class]; c && c != [NSView class]; c = class_getSuperclass(c)) {
    unsigned count = 0;
    Method* methods = class_copyMethodList(c, &count);
    for (unsigned i = 0; i < count; i++) {
      NSString* name = NSStringFromSelector(method_getName(methods[i]));
      for (NSString* word in @[ @"media", @"playback", @"nowplaying", @"remotecommand", @"audio" ]) {
        if ([name.lowercaseString containsString:word]) {
          [names addObject:name];
          break;
        }
      }
    }
    free(methods);
  }
  [names sortUsingSelector:@selector(compare:)];
  Log(@"WKWebView selectors on media: %@", [names componentsJoinedByString:@" "]);
}

- (void)after:(double)seconds run:(dispatch_block_t)block {
  dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(seconds * NSEC_PER_SEC)),
                 dispatch_get_main_queue(), block);
}

- (void)inWorld:(WKContentWorld*)world
            run:(NSString*)script
           then:(void (^)(id result))then {
  [_web evaluateJavaScript:script
                   inFrame:nil
            inContentWorld:world
         completionHandler:^(id result, NSError* error) {
           if (error) {
             Log(@"script %@ failed: %@", [script substringToIndex:MIN(script.length, 60)],
                 error.userInfo[@"WKJavaScriptExceptionMessage"] ?: error.localizedDescription);
           }
           if (then) {
             then(result);
           }
         }];
}

- (void)playbackState:(NSString*)when then:(dispatch_block_t)then {
  [_web requestMediaPlaybackStateWithCompletionHandler:^(WKMediaPlaybackState state) {
    NSArray* names = @[ @"none", @"playing", @"paused", @"suspended" ];
    Log(@"%@: requestMediaPlaybackState %@", when,
        state < names.count ? names[state] : @(state).stringValue);
    if (then) {
      then();
    }
  }];
}

- (void)userContentController:(WKUserContentController*)controller
      didReceiveScriptMessage:(WKScriptMessage*)message {
  _messages++;
  Log(@"page %@ (world %@)", message.body, message.world.name ?: @"page");
  if (!_playing && [message.body hasPrefix:@"playing:"]) {
    _playing = YES;
    [self after:1 run:^{
      [self steps];
    }];
  }
}

- (void)observeValueForKeyPath:(NSString*)keyPath
                      ofObject:(id)object
                        change:(NSDictionary*)change
                       context:(void*)context {
  _audioChanges++;
  Log(@"_isPlayingAudio %@", [_web valueForKey:@"_isPlayingAudio"]);
}

- (void)webView:(WKWebView*)web didFinishNavigation:(WKNavigation*)navigation {
  Log(@"loaded %@, title %@", web.URL.absoluteString, web.title);
  [self after:5 run:^{
    if (!self->_playing) {
      Log(@"no playing event 5 s after the load; title %@", self->_web.title);
    }
  }];
}

// One step after another, each given time to answer, with what arrived logged as it came.
- (void)steps {
  NSMutableArray<dispatch_block_t>* steps = [NSMutableArray array];
  __block void (^next)(void);
  next = ^{
    if (steps.count == 0) {
      Log(@"done: %d messages, %d _isPlayingAudio changes in all", self->_messages,
          self->_audioChanges);
      exit(0);
    }
    dispatch_block_t step = steps.firstObject;
    [steps removeObjectAtIndex:0];
    step();
    [self after:3 run:next];
  };

  // 1. What the host hears without asking the page.
  [steps addObject:^{
    [self playbackState:@"1 playing" then:nil];
    MPNowPlayingInfoCenter* center = [MPNowPlayingInfoCenter defaultCenter];
    Log(@"1 playing: the host's MPNowPlayingInfoCenter %@, state %ld",
        center.nowPlayingInfo ?: @"has no info", (long)center.playbackState);
    Log(@"1 playing: the host's MPRemoteCommandCenter next track enabled %d",
        [MPRemoteCommandCenter sharedCommandCenter].nextTrackCommand.enabled);
    [self inWorld:WKContentWorld.pageWorld
              run:@"typeof navigator.mediaSession.callActionHandler"
             then:^(id result) {
               Log(@"1 playing: callActionHandler in the page's world: %@", result);
             }];
  }];
  // 2. The page's next track handler, from the browser's world, where it has callActionHandler.
  [steps addObject:^{
    [self inWorld:World()
              run:@"typeof navigator.mediaSession.callActionHandler == 'function' ? "
                   "(navigator.mediaSession.callActionHandler({action: 'nexttrack'}), 'called') : "
                   "'no callActionHandler'"
             then:^(id result) {
               Log(@"2 next track from the browser's world: %@; the page's title %@", result,
                   self->_web.title);
             }];
  }];
  // 3. Pause and play through the media element, from the browser's world.
  [steps addObject:^{
    [self inWorld:World()
              run:@"document.querySelector('audio').pause(), 'paused'"
             then:^(id result) {
               Log(@"3 pause from the browser's world: %@", result);
             }];
  }];
  [steps addObject:^{
    [self playbackState:@"4 after the pause" then:nil];
    [self inWorld:World()
              run:@"document.querySelector('audio').play(), 'asked'"
             then:^(id result) {
               Log(@"4 play from the browser's world: %@", result);
             }];
  }];
  // 5. WebKit's own pause of everything, public.
  [steps addObject:^{
    [self->_web pauseAllMediaPlaybackWithCompletionHandler:^{
      Log(@"5 pauseAllMediaPlayback done");
      [self playbackState:@"5 after pauseAll" then:nil];
    }];
  }];
  // 6. The page changes its metadata with no media event: the browser's world hears nothing, and
  // a read sees it.
  [steps addObject:^{
    self->_messages = 0;
    [self inWorld:WKContentWorld.pageWorld
              run:@"document.querySelector('audio').play(), navigator.mediaSession.metadata = "
                   "new MediaMetadata({title: 'Track C', artist: 'Artist C'}), 'set'"
             then:nil];
  }];
  [steps addObject:^{
    Log(@"6 metadata set by the page with a play: %d messages since", self->_messages);
    [self inWorld:WKContentWorld.pageWorld
              run:@"navigator.mediaSession.metadata = new MediaMetadata({title: 'Track D'}), 'set'"
             then:nil];
  }];
  [steps addObject:^{
    Log(@"6 metadata set by the page alone: %d messages since the play", self->_messages);
    [self inWorld:World() run:@"iloPost('read'), 'read'" then:nil];
  }];
  // 7. Sound from an element the document doesn't hold, as some players make.
  [steps addObject:^{
    self->_messages = 0;
    int changes = self->_audioChanges;
    [self inWorld:WKContentWorld.pageWorld
              run:@"document.querySelector('audio').pause(), window.detached = new Audio('tone.wav'),"
                   "detached.loop = true, detached.play(), 'detached'"
             then:nil];
    [self after:2.5 run:^{
      Log(@"7 a detached element playing: %d messages, %d _isPlayingAudio changes",
          self->_messages, self->_audioChanges - changes);
      [self playbackState:@"7 detached" then:nil];
    }];
  }];
  [steps addObject:^{
    [self inWorld:WKContentWorld.pageWorld run:@"detached.pause(), 'paused'" then:nil];
  }];
  // 8. Nothing plays: what arrives in 5 s.
  [steps addObject:^{
    self->_messages = 0;
    int changes = self->_audioChanges;
    [self after:5 run:^{
      Log(@"8 nothing playing for 5 s: %d messages, %d _isPlayingAudio changes", self->_messages,
          self->_audioChanges - changes);
    }];
  }];
  [steps addObject:^{
    [self after:3 run:^{
    }];
  }];
  next();
}

@end

int main(int argc, const char* argv[]) {
  @autoreleasepool {
    started = CFAbsoluteTimeGetCurrent();
    if (argc < 2) {
      fprintf(stderr, "usage: probe <page url>\n");
      return 2;
    }
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
    Probe* probe = [[Probe alloc] initWithURL:[NSURL URLWithString:@(argv[1])]];
    dispatch_async(dispatch_get_main_queue(), ^{
      [probe start];
    });
    [NSApp run];
  }
  return 0;
}
