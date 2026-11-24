/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! Rayman Fiesta Run 1.2.3 with `--fit-screen`.
//!
//! The game doesn't size itself to the screen: it looks the device's model
//! up in a table of screen sizes. Models it doesn't know, the original
//! iPhone included, get 480x320 pixels and a 1024x682 layout. Its UbiArt
//! engine copes with other shapes (the iPhone 5's entry widens the layout
//! to 1211x682, and the iPad's makes it 1024x768), so for a fitted screen
//! the numbers in that fallback entry are changed to match it. Only the
//! armv7s code of 1.2.3 is patched, and only if it is exactly as expected.

use crate::mem::{Mem, Ptr};
use crate::window::{DeviceFamily, Window};

const BUNDLE_ID: &str = "com.ubisoft.rayman.fiesta";

/// The four instructions setting the fallback entry's layout height, layout
/// width, pixel height and pixel width, and their original encodings:
/// `movw r0, #682`, `mov.w r0, #1024`, `mov.w r0, #320`, `mov.w r0, #480`.
const SITES: [(u32, [u16; 2]); 4] = [
    (0x15f08, [0xf240, 0x20aa]),
    (0x15f0e, [0xf44f, 0x6080]),
    (0x15f14, [0xf44f, 0x70a0]),
    (0x15f1a, [0xf44f, 0x70f0]),
];

/// The layout's usual size, which the engine keeps one side of.
const LAYOUT: (u32, u32) = (1024, 682);

pub fn fit_screen(bundle_id: &str, window: Option<&Window>, mem: &mut Mem) {
    let Some(window) = window else {
        return;
    };
    if bundle_id != BUNDLE_ID
        || !window.is_screen_fitted()
        || window.device_family() != DeviceFamily::iPhone
    {
        return;
    }
    let read = |mem: &Mem, addr: u32| -> [u16; 2] {
        [
            mem.read(Ptr::<u16, false>::from_bits(addr)),
            mem.read(Ptr::<u16, false>::from_bits(addr + 2)),
        ]
    };
    if SITES.iter().any(|&(addr, original)| read(mem, addr) != original) {
        log!("Rayman Fiesta Run: not the code expected (another version?), so not fitted to the screen.");
        return;
    }

    // The game is landscape; the window's size is portrait.
    let (short, long) = window.portrait_size();
    let sizes = layout_and_pixels(long, short);
    let [layout_h, layout_w, pixels_h, pixels_w] = sizes;
    log!(
        "Applying game-specific hack for Rayman Fiesta Run: {}x{} pixels, {}x{} layout, to fill the fitted screen.",
        pixels_w, pixels_h, layout_w, layout_h
    );
    for (&(addr, _), value) in SITES.iter().zip(sizes) {
        let [hw1, hw2] = movw_r0(value);
        mem.write(Ptr::<u16, true>::from_bits(addr), hw1);
        mem.write(Ptr::<u16, true>::from_bits(addr + 2), hw2);
    }
}

/// For a screen of `width`x`height` pixels (landscape), the layout height,
/// layout width, pixel height and pixel width to give the game: like the
/// iPhone 5's entry, a wider screen widens the layout, and like the iPad's, a
/// taller one heightens it.
fn layout_and_pixels(width: u32, height: u32) -> [u32; 4] {
    let (layout_w, layout_h) = LAYOUT;
    if width * layout_h > height * layout_w {
        [layout_h, (layout_h * width + height / 2) / height, height, width]
    } else {
        [(layout_w * height + width / 2) / width, layout_w, height, width]
    }
}

/// The Thumb-2 `movw r0, #value` (encoding T3).
fn movw_r0(value: u32) -> [u16; 2] {
    assert!(value <= 0xffff);
    let (imm4, i, imm3, imm8) = (value >> 12, (value >> 11) & 1, (value >> 8) & 7, value & 0xff);
    [(0xf240 | (i << 10) | imm4) as u16, ((imm3 << 12) | imm8) as u16]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_the_original() {
        // The original first instruction is `movw r0, #682`.
        assert_eq!(movw_r0(682), SITES[0].1);
        assert_eq!(movw_r0(0xffff), [0xf64f, 0x70ff]);
    }

    #[test]
    fn sizes() {
        // The game's own entry for the iPhone 5 (1136x640) comes out the same.
        assert_eq!(layout_and_pixels(1136, 640), [682, 1211, 640, 1136]);
        // The iPhone Duo's inner screen, fitted: taller.
        assert_eq!(layout_and_pixels(480, 338), [721, 1024, 338, 480]);
        // A 19.5:9 iPhone, fitted: wider.
        assert_eq!(layout_and_pixels(693, 320), [682, 1477, 320, 693]);
    }
}
