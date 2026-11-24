/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Thread-specific data keys.
//!
//! As on iOS, each thread's values live in an array in guest memory, indexed
//! by key, which the thread ID register TPIDRURO points to while the thread
//! runs. So `pthread_getspecific` and `pthread_setspecific` can be, and for
//! the guest are, three-instruction guest routines (see [GUEST_ROUTINES])
//! rather than calls into the host: C++ exception handling (SjLj, in
//! libgcc_s) makes them on entry to and exit from many functions, so going
//! to the host for each was a large share of some games' host calls.

use crate::abi::GuestFunction;
use crate::dyld::{export_c_func, FunctionExports};
use crate::libc::errno::{EAGAIN, EINVAL};
use crate::mem::{ConstVoidPtr, Mem, MutPtr, MutVoidPtr};
use crate::Environment;

const PTHREAD_KEYS_MAX: i32 = 512;
/// Keys are 1 to [PTHREAD_KEYS_MAX], so a thread's array has a slot more.
const TLS_SLOTS: u32 = PTHREAD_KEYS_MAX as u32 + 1;

#[derive(Default)]
pub struct State {
    /// The `pthread_key_t` value, with 1 subtracted, is the index into this
    /// vector; [Some] holds the key's destructor pointer.
    keys: Vec<Option<GuestFunction>>,
}
impl State {
    fn get(env: &mut Environment) -> &mut Self {
        &mut env.libc_state.pthread.key
    }
}

type pthread_key_t = u32;

/// A new thread's array of values, all null.
pub fn alloc_tls(mem: &mut Mem) -> MutPtr<MutVoidPtr> {
    let tls: MutPtr<MutVoidPtr> = mem.alloc(TLS_SLOTS * 4).cast();
    mem.bytes_at_mut(tls.cast(), TLS_SLOTS * 4).fill(0);
    tls
}

/// `pthread_getspecific(key)` (ARM): `mrc p15, 0, r1, c13, c0, 3`,
/// `ldr r0, [r1, r0, lsl #2]`, `bx lr`.
const GETSPECIFIC: [u32; 3] = [0xee1d1f70, 0xe7910100, 0xe12fff1e];
/// `pthread_setspecific(key, value)` (ARM): `mrc p15, 0, r2, c13, c0, 3`,
/// `str r1, [r2, r0, lsl #2]`, `mov r0, #0`, `bx lr`.
const SETSPECIFIC: [u32; 4] = [0xee1d2f70, 0xe7821100, 0xe3a00000, 0xe12fff1e];

/// Guest code the dynamic linker links these symbols to, instead of the host
/// functions below (which the host itself can still use).
pub const GUEST_ROUTINES: &[(&str, &[u32])] = &[
    ("_pthread_getspecific", &GETSPECIFIC),
    ("_pthread_setspecific", &SETSPECIFIC),
];

fn current_slot(env: &mut Environment, key: pthread_key_t) -> MutPtr<MutVoidPtr> {
    env.threads[env.current_thread].tls + key
}

fn pthread_key_create(
    env: &mut Environment,
    key_ptr: MutPtr<pthread_key_t>,
    destructor: GuestFunction, // void (*destructor)(void *), may be NULL
) -> i32 {
    let keys = &mut State::get(env).keys;
    let idx = match keys.iter().position(Option::is_none) {
        Some(idx) => idx, // reuse an old slot
        None if keys.len() < PTHREAD_KEYS_MAX as usize => {
            keys.push(None);
            keys.len() - 1
        }
        None => return EAGAIN, // failure, no slots left
    };
    keys[idx] = Some(destructor);
    let key: pthread_key_t = (idx + 1).try_into().unwrap(); // PTHREAD_KEYS_MAX is small
    // A new key has no value in any thread, whatever a deleted one left.
    for i in 0..env.threads.len() {
        let slot = env.threads[i].tls + key;
        env.mem.write(slot, MutVoidPtr::null());
    }
    env.mem.write(key_ptr, key);
    0 // success
}

fn pthread_getspecific(env: &mut Environment, key: pthread_key_t) -> MutVoidPtr {
    // Use of an invalid key is undefined, as it is with the guest routine.
    assert!((1..TLS_SLOTS).contains(&key));
    let slot = current_slot(env, key);
    env.mem.read(slot)
}

fn pthread_setspecific(env: &mut Environment, key: pthread_key_t, value: ConstVoidPtr) -> i32 {
    if !(1..TLS_SLOTS).contains(&key) {
        return EINVAL; // failure, out of bounds
    }
    let slot = current_slot(env, key);
    env.mem.write(slot, value.cast_mut());
    0 // success
}

fn pthread_key_delete(env: &mut Environment, key: pthread_key_t) -> i32 {
    // Use of invalid key is undefined, panicking is fine.
    let idx: usize = key.checked_sub(1).unwrap().try_into().unwrap();
    match State::get(env).keys.get_mut(idx) {
        Some(slot @ Some(_)) => {
            *slot = None;
            0 // success
        }
        _ => EINVAL, // failure, unused key or out of bounds
    }
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(pthread_key_create(_, _)),
    export_c_func!(pthread_getspecific(_)),
    export_c_func!(pthread_setspecific(_, _)),
    export_c_func!(pthread_key_delete(_)),
];
