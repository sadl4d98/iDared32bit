/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `NSMetadataQuery` (iOS 5), which on iOS only searches iCloud.
//!
//! There's no iCloud here (`URLForUbiquityContainerIdentifier:` returns nil,
//! as on a device signed out of it), so a query never starts and finds
//! nothing. Games that check for iCloud first only make one up front.

use crate::dyld::{ConstantExports, HostConstant};
use crate::frameworks::foundation::NSUInteger;
use crate::objc::{id, msg_class, nil, objc_classes, ClassExports};

pub const CONSTANTS: ConstantExports = &[
    (
        "_NSMetadataQueryDidFinishGatheringNotification",
        HostConstant::NSString("NSMetadataQueryDidFinishGatheringNotification"),
    ),
    (
        "_NSMetadataQueryDidUpdateNotification",
        HostConstant::NSString("NSMetadataQueryDidUpdateNotification"),
    ),
    (
        "_NSMetadataQueryUbiquitousDocumentsScope",
        HostConstant::NSString("NSMetadataQueryUbiquitousDocumentsScope"),
    ),
    (
        "_NSMetadataQueryUbiquitousDataScope",
        HostConstant::NSString("NSMetadataQueryUbiquitousDataScope"),
    ),
    ("_NSMetadataItemFSNameKey", HostConstant::NSString("kMDItemFSName")),
    ("_NSMetadataItemURLKey", HostConstant::NSString("kMDItemURL")),
];

pub const CLASSES: ClassExports = objc_classes! {

(env, this, _cmd);

@implementation NSMetadataQuery: NSObject

- (())setPredicate:(id)_predicate {}
- (())setSearchScopes:(id)_scopes {}
- (())setSortDescriptors:(id)_descriptors {}
- (())setDelegate:(id)_delegate {}
- (id)delegate {
    nil
}

- (bool)startQuery {
    log!("[(NSMetadataQuery*){:?} startQuery]: no iCloud, so not started", this);
    false
}
- (())stopQuery {}
- (())enableUpdates {}
- (())disableUpdates {}
- (bool)isStarted {
    false
}
- (bool)isGathering {
    false
}

- (NSUInteger)resultCount {
    0
}
- (id)results {
    msg_class![env; NSArray array]
}

@end

};
