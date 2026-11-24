/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `UIDocument` (iOS 5), without iCloud: a document is the file at its URL.
//!
//! Opening reads the file and hands it to the subclass's
//! `loadFromContents:ofType:error:`; saving writes what its
//! `contentsForType:error:` returns. As on iOS, completion handlers run later
//! on the main thread, not before the method returns.

use crate::frameworks::foundation::NSInteger;
use crate::mem::{ConstVoidPtr, MutPtr};
use crate::objc::{
    _Block_copy, _Block_release, call_block_with_bool, id, msg, msg_class, nil, objc_classes,
    release, retain, ClassExports, HostObject, NSZonePtr, SEL,
};
use crate::Environment;

type UIDocumentSaveOperation = NSInteger;
const UIDOCUMENT_STATE_NORMAL: NSInteger = 0;

#[derive(Default)]
struct UIDocumentHostObject {
    /// `NSURL*`, retained.
    file_url: id,
    /// Copied completion handlers waiting to run, with their result.
    pending: Vec<(ConstVoidPtr, bool)>,
}
impl HostObject for UIDocumentHostObject {}

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation UIDocument: NSObject

+ (id)allocWithZone:(NSZonePtr)_zone {
    let host_object = Box::<UIDocumentHostObject>::default();
    env.objc.alloc_object(this, host_object, &mut env.mem)
}

- (id)initWithFileURL:(id)url { // NSURL*
    retain(env, url);
    env.objc.borrow_mut::<UIDocumentHostObject>(this).file_url = url;
    this
}

- (())dealloc {
    let url = env.objc.borrow::<UIDocumentHostObject>(this).file_url;
    release(env, url);
    env.objc.dealloc_object(this, &mut env.mem)
}

- (id)fileURL {
    env.objc.borrow::<UIDocumentHostObject>(this).file_url
}
- (id)fileType {
    nil
}
- (NSInteger)documentState {
    UIDOCUMENT_STATE_NORMAL
}
- (bool)hasUnsavedChanges {
    false
}
- (())updateChangeCount:(NSInteger)_change {
    // Nothing saves by itself here; the app saves when it wants to.
}

- (())openWithCompletionHandler:(ConstVoidPtr)handler {
    let url = env.objc.borrow::<UIDocumentHostObject>(this).file_url;
    let ok = load(env, this, url);
    finish_later(env, this, handler, ok);
}

- (())revertToContentsOfURL:(id)url // NSURL*
          completionHandler:(ConstVoidPtr)handler {
    let ok = load(env, this, url);
    finish_later(env, this, handler, ok);
}

- (())saveToURL:(id)url // NSURL*
forSaveOperation:(UIDocumentSaveOperation)_operation
completionHandler:(ConstVoidPtr)handler {
    let file_type: id = msg![env; this fileType];
    let contents: id = msg![env; this contentsForType:file_type error:(MutPtr::<id>::null())];
    let ok = if contents == nil {
        false
    } else {
        let path: id = msg![env; url path];
        msg![env; contents writeToFile:path atomically:true]
    };
    log_dbg!("[(UIDocument*){:?} saveToURL:{:?}] => {}", this, url, ok);
    finish_later(env, this, handler, ok);
}

- (())closeWithCompletionHandler:(ConstVoidPtr)handler {
    finish_later(env, this, handler, true);
}

- (())_touchHLE_runCompletionHandlers {
    let pending = std::mem::take(&mut env.objc.borrow_mut::<UIDocumentHostObject>(this).pending);
    for (handler, ok) in pending {
        call_block_with_bool(env, handler, ok);
        _Block_release(env, handler);
    }
    release(env, this);
}

@end

};

/// Reads the file at `url` into the document.
fn load(env: &mut Environment, this: id, url: id) -> bool {
    let data: id = msg_class![env; NSData dataWithContentsOfURL:url];
    let ok = data != nil && {
        let file_type: id = msg![env; this fileType];
        msg![env; this loadFromContents:data ofType:file_type error:(MutPtr::<id>::null())]
    };
    log_dbg!("[(UIDocument*){:?} load {:?}] => {}", this, url, ok);
    ok
}

fn finish_later(env: &mut Environment, this: id, handler: ConstVoidPtr, ok: bool) {
    if handler.is_null() {
        return;
    }
    let handler = _Block_copy(env, handler);
    env.objc
        .borrow_mut::<UIDocumentHostObject>(this)
        .pending
        .push((handler, ok));
    // Kept alive until the handler has run.
    retain(env, this);
    let sel: SEL = env
        .objc
        .lookup_selector("_touchHLE_runCompletionHandlers")
        .unwrap();
    () = msg![env; this performSelector:sel withObject:nil afterDelay:0.0];
}
