/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `CFProxySupport.h`. There's no proxy here, so the settings are empty.

use crate::dyld::{export_c_func, ConstantExports, FunctionExports, HostConstant};
use crate::objc::{id, msg_class};
use crate::Environment;

pub const CONSTANTS: ConstantExports = &[
    ("_kCFNetworkProxiesHTTPProxy", HostConstant::NSString("HTTPProxy")),
    ("_kCFNetworkProxiesHTTPPort", HostConstant::NSString("HTTPPort")),
    ("_kCFNetworkProxiesHTTPEnable", HostConstant::NSString("HTTPEnable")),
];

/// Returns a `CFDictionaryRef` the caller owns.
fn CFNetworkCopySystemProxySettings(env: &mut Environment) -> id {
    msg_class![env; NSDictionary new]
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(CFNetworkCopySystemProxySettings())];
