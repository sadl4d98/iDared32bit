/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! The GLKit framework (iOS 5): `GLKView` and `GLKViewController`.
//!
//! `GLKView` is a `UIView` backed by a `CAEAGLLayer` that makes and owns its
//! own framebuffer, so it is built on `EAGLContext`'s
//! `renderbufferStorage:fromDrawable:` and `presentRenderbuffer:`, like an app
//! doing it by hand. `GLKViewController` calls `update` and `-[GLKView display]`
//! from a timer at its `preferredFramesPerSecond`.
//!
//! Resources:
//! - [GLKView](https://developer.apple.com/documentation/glkit/glkview)
//! - [GLKViewController](https://developer.apple.com/documentation/glkit/glkviewcontroller)

use crate::dyld::HostDylib;
use crate::frameworks::core_graphics::{CGRect, CGSize};
use crate::frameworks::foundation::ns_timer::set_time_interval;
use crate::frameworks::foundation::{NSInteger, NSTimeInterval};
use crate::frameworks::opengles::sync_context;
use crate::frameworks::uikit::ui_view::UIViewHostObject;
use crate::frameworks::uikit::ui_view_controller::UIViewControllerHostObject;
use crate::gles::gles11_raw as gles11; // constants only
use crate::gles::gles11_raw::types::*;
use crate::objc::{
    id, impl_HostObject_with_superclass, msg, msg_class, msg_send, msg_super, nil, objc_classes,
    release, retain, Class, ClassExports, NSZonePtr, SEL,
};
use crate::Environment;
use std::time::Instant;

pub const DYLIB: HostDylib = HostDylib {
    path: "/System/Library/Frameworks/GLKit.framework/GLKit",
    aliases: &[],
    class_exports: &[CLASSES],
    constant_exports: &[],
    function_exports: &[],
};

// Not in the generated GLES 1.1 constants (they're from later extensions),
// but the GL 2 hosts we run on accept them.
const DEPTH_COMPONENT24_OES: GLenum = 0x81A6;

/// `GLKViewDrawableDepthFormat`: None, 16, 24.
type GLKViewDrawableFormat = NSInteger;
const GLK_VIEW_DRAWABLE_DEPTH_FORMAT_16: GLKViewDrawableFormat = 1;

#[derive(Default)]
struct GLKViewHostObject {
    superclass: UIViewHostObject,
    /// `EAGLContext*`, retained.
    context: id,
    /// `id<GLKViewDelegate>`, weak.
    delegate: id,
    color_format: GLKViewDrawableFormat,
    depth_format: GLKViewDrawableFormat,
    stencil_format: GLKViewDrawableFormat,
    multisample: GLKViewDrawableFormat,
    enable_set_needs_display: bool,
    framebuffer: GLuint,
    color_renderbuffer: GLuint,
    depth_renderbuffer: GLuint,
    width: GLint,
    height: GLint,
}
impl_HostObject_with_superclass!(GLKViewHostObject);

#[derive(Default)]
struct GLKViewControllerHostObject {
    superclass: UIViewControllerHostObject,
    /// `id<GLKViewControllerDelegate>`, weak.
    delegate: id,
    /// 0 means the default, 30.
    preferred_fps: NSInteger,
    paused: bool,
    frames_displayed: NSInteger,
    /// `NSTimer*`, retained by its run loop.
    timer: id,
    last_update: Option<Instant>,
    last_draw: Option<Instant>,
    time_since_last_update: NSTimeInterval,
    time_since_last_draw: NSTimeInterval,
    pause_on_will_resign_active: bool,
    resume_on_did_become_active: bool,
}
impl_HostObject_with_superclass!(GLKViewControllerHostObject);

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation GLKView: UIView

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(GLKViewHostObject {
        enable_set_needs_display: true,
        ..Default::default()
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

+ (Class)layerClass {
    env.objc.get_known_class("CAEAGLLayer", &mut env.mem)
}

- (id)initWithFrame:(CGRect)frame {
    msg_super![env; this initWithFrame:frame]
}

- (id)initWithFrame:(CGRect)frame context:(id)context { // EAGLContext*
    let this: id = msg![env; this initWithFrame:frame];
    () = msg![env; this setContext:context];
    this
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    fit_to_screen(env, this);
    this
}

- (())dealloc {
    delete_drawable(env, this);
    let context = env.objc.borrow::<GLKViewHostObject>(this).context;
    release(env, context);
    msg_super![env; this dealloc]
}

- (id)context {
    env.objc.borrow::<GLKViewHostObject>(this).context
}
- (())setContext:(id)context { // EAGLContext*
    let old = env.objc.borrow::<GLKViewHostObject>(this).context;
    if old == context {
        return;
    }
    // The drawable belongs to the old context.
    delete_drawable(env, this);
    retain(env, context);
    env.objc.borrow_mut::<GLKViewHostObject>(this).context = context;
    release(env, old);
}

- (id)delegate {
    env.objc.borrow::<GLKViewHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<GLKViewHostObject>(this).delegate = delegate;
}

- (GLKViewDrawableFormat)drawableColorFormat {
    env.objc.borrow::<GLKViewHostObject>(this).color_format
}
- (())setDrawableColorFormat:(GLKViewDrawableFormat)format {
    // ponytail: always RGBA8, as EAGLContext does; RGB565 isn't honoured.
    env.objc.borrow_mut::<GLKViewHostObject>(this).color_format = format;
}
- (GLKViewDrawableFormat)drawableDepthFormat {
    env.objc.borrow::<GLKViewHostObject>(this).depth_format
}
- (())setDrawableDepthFormat:(GLKViewDrawableFormat)format {
    delete_drawable(env, this);
    env.objc.borrow_mut::<GLKViewHostObject>(this).depth_format = format;
}
- (GLKViewDrawableFormat)drawableStencilFormat {
    env.objc.borrow::<GLKViewHostObject>(this).stencil_format
}
- (())setDrawableStencilFormat:(GLKViewDrawableFormat)format {
    if format != 0 {
        // ponytail: no stencil buffer yet; add a packed depth-stencil
        // renderbuffer when a game is found to need one.
        log!("TODO: [(GLKView*){:?} setDrawableStencilFormat:{}] ignored", this, format);
    }
    env.objc.borrow_mut::<GLKViewHostObject>(this).stencil_format = format;
}
- (GLKViewDrawableFormat)drawableMultisample {
    env.objc.borrow::<GLKViewHostObject>(this).multisample
}
- (())setDrawableMultisample:(GLKViewDrawableFormat)multisample {
    // Multisampling is only a quality improvement; it's not done.
    env.objc.borrow_mut::<GLKViewHostObject>(this).multisample = multisample;
}

- (bool)enableSetNeedsDisplay {
    env.objc.borrow::<GLKViewHostObject>(this).enable_set_needs_display
}
- (())setEnableSetNeedsDisplay:(bool)enable {
    env.objc.borrow_mut::<GLKViewHostObject>(this).enable_set_needs_display = enable;
}

- (NSInteger)drawableWidth {
    ensure_drawable(env, this);
    env.objc.borrow::<GLKViewHostObject>(this).width
}
- (NSInteger)drawableHeight {
    ensure_drawable(env, this);
    env.objc.borrow::<GLKViewHostObject>(this).height
}

- (())bindDrawable {
    bind_drawable(env, this);
}

- (())deleteDrawable {
    delete_drawable(env, this);
}

- (())display {
    let context = env.objc.borrow::<GLKViewHostObject>(this).context;
    if context == nil {
        return;
    }
    let _: bool = msg_class![env; EAGLContext setCurrentContext:context];
    bind_drawable(env, this);

    let bounds: CGRect = msg![env; this bounds];
    let delegate = env.objc.borrow::<GLKViewHostObject>(this).delegate;
    let draw_in_rect = env.objc.register_host_selector("glkView:drawInRect:".to_string(), &mut env.mem);
    if delegate != nil && msg![env; delegate respondsToSelector:draw_in_rect] {
        () = msg_send(env, (delegate, draw_in_rect, this, bounds));
    } else {
        () = msg![env; this drawRect:bounds];
    }

    let color_renderbuffer = env.objc.borrow::<GLKViewHostObject>(this).color_renderbuffer;
    {
        let window = env.window.as_mut().unwrap();
        let mut gles = sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread);
        unsafe { gles.BindRenderbufferOES(gles11::RENDERBUFFER_OES, color_renderbuffer) };
    }
    let _: bool = msg![env; context presentRenderbuffer:(gles11::RENDERBUFFER_OES)];
}

- (id)snapshot {
    log!("TODO: [(GLKView*){:?} snapshot] returning nil", this);
    nil
}

@end

@implementation GLKViewController: UIViewController

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::new(GLKViewControllerHostObject {
        pause_on_will_resign_active: true,
        resume_on_did_become_active: true,
        ..Default::default()
    });
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithNibName:(id)nib_name // NSString*
               bundle:(id)bundle { // NSBundle*
    let this: id = msg_super![env; this initWithNibName:nib_name bundle:bundle];
    start_timer(env, this);
    this
}

- (id)initWithCoder:(id)coder {
    let this: id = msg_super![env; this initWithCoder:coder];
    start_timer(env, this);
    this
}

- (())dealloc {
    let timer = env.objc.borrow::<GLKViewControllerHostObject>(this).timer;
    if timer != nil {
        () = msg![env; timer invalidate];
    }
    msg_super![env; this dealloc]
}

- (id)delegate {
    env.objc.borrow::<GLKViewControllerHostObject>(this).delegate
}
- (())setDelegate:(id)delegate {
    env.objc.borrow_mut::<GLKViewControllerHostObject>(this).delegate = delegate;
}

- (NSInteger)preferredFramesPerSecond {
    preferred_fps(env, this)
}
- (())setPreferredFramesPerSecond:(NSInteger)fps {
    log_dbg!("[(GLKViewController*){:?} setPreferredFramesPerSecond:{}]", this, fps);
    env.objc.borrow_mut::<GLKViewControllerHostObject>(this).preferred_fps = fps;
    let timer = env.objc.borrow::<GLKViewControllerHostObject>(this).timer;
    let interval = frame_interval(env, this);
    set_time_interval(env, timer, interval);
}
- (NSInteger)framesPerSecond {
    (1.0 / frame_interval(env, this)).round() as NSInteger
}

- (bool)isPaused {
    env.objc.borrow::<GLKViewControllerHostObject>(this).paused
}
- (())setPaused:(bool)paused {
    let host_object = env.objc.borrow_mut::<GLKViewControllerHostObject>(this);
    host_object.paused = paused;
    // A long pause shouldn't count as one long frame.
    host_object.last_update = None;
    host_object.last_draw = None;
}

- (bool)pauseOnWillResignActive {
    env.objc.borrow::<GLKViewControllerHostObject>(this).pause_on_will_resign_active
}
- (())setPauseOnWillResignActive:(bool)pause {
    env.objc.borrow_mut::<GLKViewControllerHostObject>(this).pause_on_will_resign_active = pause;
}
- (bool)resumeOnDidBecomeActive {
    env.objc.borrow::<GLKViewControllerHostObject>(this).resume_on_did_become_active
}
- (())setResumeOnDidBecomeActive:(bool)resume {
    env.objc.borrow_mut::<GLKViewControllerHostObject>(this).resume_on_did_become_active = resume;
}

- (NSInteger)framesDisplayed {
    env.objc.borrow::<GLKViewControllerHostObject>(this).frames_displayed
}
- (NSTimeInterval)timeSinceLastUpdate {
    env.objc.borrow::<GLKViewControllerHostObject>(this).time_since_last_update
}
- (NSTimeInterval)timeSinceLastDraw {
    env.objc.borrow::<GLKViewControllerHostObject>(this).time_since_last_draw
}

- (())_touchHLE_glkTick:(id)_timer { // NSTimer*
    tick(env, this);
}

@end

};

/// With `--fit-screen`, a GLKView made in Interface Builder to fill the
/// device's usual screen is made to fill the fitted one instead, straight
/// away: games read its size before the view is put in the window (on a real
/// iPhone 5 they load another nib, made for its screen, for this reason).
fn fit_to_screen(env: &mut Environment, this: id) {
    let window = env.window();
    if !window.is_screen_fitted() {
        return;
    }
    let (usual_w, usual_h) = window.device_family().portrait_size();
    let (fit_w, fit_h) = window.portrait_size();
    let frame: CGRect = msg![env; this frame];
    let size = (frame.size.width.round() as u32, frame.size.height.round() as u32);
    let new_size = if size == (usual_w, usual_h) {
        (fit_w, fit_h)
    } else if size == (usual_h, usual_w) {
        (fit_h, fit_w)
    } else {
        return;
    };
    let new_frame = CGRect {
        origin: frame.origin,
        size: CGSize {
            width: new_size.0 as f32,
            height: new_size.1 as f32,
        },
    };
    log_dbg!("GLKView {:?}: {:?} -> {:?} to fill the fitted screen", this, frame, new_frame);
    () = msg![env; this setFrame:new_frame];
}

/// Makes the framebuffer if there isn't one, or remakes it if the view's
/// size has changed.
fn ensure_drawable(env: &mut Environment, this: id) {
    let &GLKViewHostObject {
        context,
        framebuffer,
        width,
        height,
        depth_format,
        ..
    } = env.objc.borrow(this);
    if context == nil {
        return;
    }
    let _: bool = msg_class![env; EAGLContext setCurrentContext:context];

    // renderbufferStorage:fromDrawable: sizes the renderbuffer like this.
    let bounds: CGRect = msg![env; this bounds];
    let scale_hack = env.options.scale_hack.get() as GLint;
    let wanted = (
        bounds.size.width.round() as GLint * scale_hack,
        bounds.size.height.round() as GLint * scale_hack,
    );
    if framebuffer != 0 && (width, height) == wanted {
        return;
    }
    delete_drawable(env, this);

    let (framebuffer, color_renderbuffer) = {
        let window = env.window.as_mut().expect("OpenGL ES is not supported in headless mode");
        let mut gles = sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread);
        let (mut framebuffer, mut color_renderbuffer) = (0, 0);
        unsafe {
            gles.GenFramebuffersOES(1, &mut framebuffer);
            gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, framebuffer);
            gles.GenRenderbuffersOES(1, &mut color_renderbuffer);
            gles.BindRenderbufferOES(gles11::RENDERBUFFER_OES, color_renderbuffer);
        }
        (framebuffer, color_renderbuffer)
    };

    let layer: id = msg![env; this layer];
    let _: bool = msg![env; context renderbufferStorage:(gles11::RENDERBUFFER_OES) fromDrawable:layer];

    let window = env.window.as_mut().unwrap();
    let mut gles = sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread);
    let (mut width, mut height, mut depth_renderbuffer) = (0, 0, 0);
    unsafe {
        gles.FramebufferRenderbufferOES(
            gles11::FRAMEBUFFER_OES,
            gles11::COLOR_ATTACHMENT0_OES,
            gles11::RENDERBUFFER_OES,
            color_renderbuffer,
        );
        gles.GetRenderbufferParameterivOES(gles11::RENDERBUFFER_OES, gles11::RENDERBUFFER_WIDTH_OES, &mut width);
        gles.GetRenderbufferParameterivOES(gles11::RENDERBUFFER_OES, gles11::RENDERBUFFER_HEIGHT_OES, &mut height);
        if depth_format != 0 {
            let internal_format = if depth_format == GLK_VIEW_DRAWABLE_DEPTH_FORMAT_16 {
                gles11::DEPTH_COMPONENT16_OES
            } else {
                DEPTH_COMPONENT24_OES
            };
            gles.GenRenderbuffersOES(1, &mut depth_renderbuffer);
            gles.BindRenderbufferOES(gles11::RENDERBUFFER_OES, depth_renderbuffer);
            gles.RenderbufferStorageOES(gles11::RENDERBUFFER_OES, internal_format, width, height);
            gles.FramebufferRenderbufferOES(
                gles11::FRAMEBUFFER_OES,
                gles11::DEPTH_ATTACHMENT_OES,
                gles11::RENDERBUFFER_OES,
                depth_renderbuffer,
            );
        }
        let status = gles.CheckFramebufferStatusOES(gles11::FRAMEBUFFER_OES);
        if status != gles11::FRAMEBUFFER_COMPLETE_OES {
            log!("Warning: GLKView {:?} framebuffer incomplete: {:#x}", this, status);
        }
        // presentRenderbuffer: presents whichever renderbuffer is bound.
        gles.BindRenderbufferOES(gles11::RENDERBUFFER_OES, color_renderbuffer);
    }
    drop(gles);
    log_dbg!("GLKView {:?} drawable {}x{}", this, width, height);

    let host_object = env.objc.borrow_mut::<GLKViewHostObject>(this);
    host_object.framebuffer = framebuffer;
    host_object.color_renderbuffer = color_renderbuffer;
    host_object.depth_renderbuffer = depth_renderbuffer;
    host_object.width = width;
    host_object.height = height;
}

fn bind_drawable(env: &mut Environment, this: id) {
    ensure_drawable(env, this);
    let &GLKViewHostObject {
        context,
        framebuffer,
        width,
        height,
        ..
    } = env.objc.borrow(this);
    if context == nil {
        return;
    }
    let window = env.window.as_mut().unwrap();
    let mut gles = sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread);
    unsafe {
        gles.BindFramebufferOES(gles11::FRAMEBUFFER_OES, framebuffer);
        gles.Viewport(0, 0, width, height);
    }
}

fn delete_drawable(env: &mut Environment, this: id) {
    let host_object = env.objc.borrow_mut::<GLKViewHostObject>(this);
    let context = host_object.context;
    let buffers = [
        std::mem::take(&mut host_object.framebuffer),
        std::mem::take(&mut host_object.color_renderbuffer),
        std::mem::take(&mut host_object.depth_renderbuffer),
    ];
    if context == nil || buffers[0] == 0 {
        return;
    }
    let _: bool = msg_class![env; EAGLContext setCurrentContext:context];
    let window = env.window.as_mut().unwrap();
    let mut gles = sync_context(&mut env.framework_state.opengles, &mut env.objc, window, env.current_thread);
    unsafe {
        gles.DeleteFramebuffersOES(1, &buffers[0]);
        gles.DeleteRenderbuffersOES(2, buffers[1..].as_ptr());
    }
}

fn preferred_fps(env: &mut Environment, this: id) -> NSInteger {
    match env.objc.borrow::<GLKViewControllerHostObject>(this).preferred_fps {
        0 => 30,
        fps => fps,
    }
}

/// As on iOS, the rate is the nearest one the 60Hz display can do at or
/// below the preferred one.
fn frame_interval(env: &mut Environment, this: id) -> NSTimeInterval {
    let fps = preferred_fps(env, this).clamp(1, 60);
    (60.0 / fps as f64).ceil() / 60.0
}

fn start_timer(env: &mut Environment, this: id) {
    let interval = frame_interval(env, this);
    let sel: SEL = env.objc.lookup_selector("_touchHLE_glkTick:").unwrap();
    let timer: id = msg_class![env; NSTimer scheduledTimerWithTimeInterval:interval
                                                                    target:this
                                                                  selector:sel
                                                                  userInfo:nil
                                                                   repeats:true];
    env.objc.borrow_mut::<GLKViewControllerHostObject>(this).timer = timer;
}

fn tick(env: &mut Environment, this: id) {
    let &GLKViewControllerHostObject { paused, delegate, last_update, .. } = env.objc.borrow(this);
    // Nothing to do until the view has been loaded, by whatever asks for it.
    if paused || !msg![env; this isViewLoaded] {
        return;
    }

    let now = Instant::now();
    let default_interval = frame_interval(env, this);
    env.objc.borrow_mut::<GLKViewControllerHostObject>(this).time_since_last_update =
        last_update.map_or(default_interval, |t| (now - t).as_secs_f64());
    env.objc.borrow_mut::<GLKViewControllerHostObject>(this).last_update = Some(now);

    let delegate_update = env.objc.register_host_selector("glkViewControllerUpdate:".to_string(), &mut env.mem);
    let update = env.objc.register_host_selector("update".to_string(), &mut env.mem);
    if delegate != nil && msg![env; delegate respondsToSelector:delegate_update] {
        () = msg_send(env, (delegate, delegate_update, this));
    } else if msg![env; this respondsToSelector:update] {
        () = msg_send(env, (this, update));
    }

    let view: id = msg![env; this view];
    let glk_view_class: Class = env.objc.get_known_class("GLKView", &mut env.mem);
    if !msg![env; view isKindOfClass:glk_view_class] {
        return;
    }
    // On iOS the controller is the view's delegate unless told otherwise.
    let draw_in_rect = env.objc.register_host_selector("glkView:drawInRect:".to_string(), &mut env.mem);
    let view_delegate: id = msg![env; view delegate];
    if view_delegate == nil && msg![env; this respondsToSelector:draw_in_rect] {
        () = msg![env; view setDelegate:this];
    }

    let last_draw = env.objc.borrow::<GLKViewControllerHostObject>(this).last_draw;
    let host_object = env.objc.borrow_mut::<GLKViewControllerHostObject>(this);
    host_object.time_since_last_draw = last_draw.map_or(0.0, |t| (now - t).as_secs_f64());
    host_object.last_draw = Some(now);
    host_object.frames_displayed += 1;
    () = msg![env; view display];
}
