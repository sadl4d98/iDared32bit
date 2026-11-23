/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `ifaddrs.h` (interface addresses)

use crate::dyld::FunctionExports;
use crate::export_c_func;
use crate::libc::errno::set_errno;
use crate::mem::{ConstPtr, MutPtr};
use crate::Environment;

// TODO: struct definition
#[allow(non_camel_case_types)]
struct ifaddrs {}

fn getifaddrs(env: &mut Environment, _ifap: MutPtr<MutPtr<ifaddrs>>) -> i32 {
    // TODO: handle errno properly
    set_errno(env, 0);

    // TODO: implement
    -1
}

fn freeifaddrs(_env: &mut Environment, _ifp: MutPtr<ifaddrs>) {
    // TODO
}

/// From `net/if.h`. There are no network interfaces here (as with
/// `getifaddrs`), so every name is unknown.
fn if_nametoindex(_env: &mut Environment, _ifname: ConstPtr<u8>) -> u32 {
    0
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(if_nametoindex(_)),
    export_c_func!(getifaddrs(_)),
    export_c_func!(freeifaddrs(_)),
];
