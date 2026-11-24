/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! stdlib's qsort

use crate::abi::{CallFromHost, GuestFunction};
use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::MutPtr;
use crate::Environment;

fn qsort(
    env: &mut Environment,
    base: MutPtr<u8>,
    nitems: u32,
    size: u32,
    compar: GuestFunction, // int (*compar)(const void *, const void*))
) {
    qsort_generic(
        env,
        nitems,
        &mut |env, i, j| {
            let i_ptr = base + i * size;
            let j_ptr = base + j * size;
            compar.call_from_host(env, (i_ptr.cast_const(), j_ptr.cast_const()))
        },
        &mut |env: &mut Environment, mut i: u32, mut j: u32| {
            if i == j {
                return;
            }
            if i > j {
                std::mem::swap(&mut i, &mut j);
            }
            let base_slice = env.mem.bytes_at_mut(base, nitems * size);
            let (_, slice_after_first) = base_slice.split_at_mut((i * size) as usize);
            let offset = (j * size - i * size) as usize;
            let (left, right) = slice_after_first.split_at_mut(offset);
            left[..size as usize].swap_with_slice(&mut right[..size as usize]);
        },
    );
}

/// Sorts `nitems` items through `compar(i, j)` (comparing the items at `i`
/// and `j`) and `swap(i, j)`.
///
/// A merge sort of the items' positions, which compares the items where they
/// are, then a pass that moves each item into place with at most one swap.
/// Merging is skipped where two halves are already in order, so a sorted
/// list takes about one comparison per item: games sort nearly sorted lists
/// every frame (things to draw, by depth), and each comparison is a call into
/// the guest. (A quicksort with the first item as pivot, as before, took
/// about n²/2 comparisons for such a list.) It never panics on a comparator
/// that isn't a consistent ordering; the items just end up in some order.
pub fn qsort_generic<T, Compare, Swap>(
    user_data: &mut T,
    nitems: u32,
    compar: &mut Compare, // int (*compar)(const void *, const void*))
    swap: &mut Swap,
) where
    Compare: FnMut(&mut T, u32, u32) -> i32,
    Swap: FnMut(&mut T, u32, u32),
{
    if nitems < 2 {
        return;
    }
    let mut order: Vec<u32> = (0..nitems).collect();
    let mut buffer = vec![0; nitems as usize];
    merge_sort(&mut order, &mut buffer, &mut |i, j| compar(user_data, i, j));

    // order[k] is where the item that belongs at k is now.
    let n = nitems as usize;
    let mut item_at: Vec<u32> = (0..nitems).collect(); // position -> item
    let mut pos_of: Vec<u32> = (0..nitems).collect(); // item -> position
    for k in 0..n {
        let p = pos_of[order[k] as usize] as usize;
        if p != k {
            swap(user_data, k as u32, p as u32);
            item_at.swap(k, p);
            pos_of[item_at[k] as usize] = k as u32;
            pos_of[item_at[p] as usize] = p as u32;
        }
    }
}

fn merge_sort(v: &mut [u32], buffer: &mut [u32], cmp: &mut impl FnMut(u32, u32) -> i32) {
    let len = v.len();
    if len <= 8 {
        for i in 1..len {
            let mut j = i;
            while j > 0 && cmp(v[j - 1], v[j]) > 0 {
                v.swap(j - 1, j);
                j -= 1;
            }
        }
        return;
    }
    let mid = len / 2;
    merge_sort(&mut v[..mid], &mut buffer[..mid], cmp);
    merge_sort(&mut v[mid..], &mut buffer[mid..], cmp);
    if cmp(v[mid - 1], v[mid]) <= 0 {
        return; // already in order
    }
    let (mut i, mut j, mut k) = (0, mid, 0);
    while i < mid && j < len {
        if cmp(v[i], v[j]) <= 0 {
            buffer[k] = v[i];
            i += 1;
        } else {
            buffer[k] = v[j];
            j += 1;
        }
        k += 1;
    }
    buffer[k..k + mid - i].copy_from_slice(&v[i..mid]);
    k += mid - i;
    buffer[k..k + len - j].copy_from_slice(&v[j..len]);
    v.copy_from_slice(&buffer[..len]);
}

#[cfg(test)]
mod tests {
    use super::qsort_generic;

    fn sort(items: &mut Vec<i32>, compar: impl Fn(i32, i32) -> i32) -> u32 {
        let mut comparisons = 0;
        let n = items.len() as u32;
        qsort_generic(
            items,
            n,
            &mut |items, i, j| {
                comparisons += 1;
                compar(items[i as usize], items[j as usize])
            },
            &mut |items, i, j| items.swap(i as usize, j as usize),
        );
        comparisons
    }

    #[test]
    fn sorts() {
        let by_value = |a: i32, b: i32| (a > b) as i32 - (a < b) as i32;
        // A scrambled list, with duplicates.
        let mut v: Vec<i32> = (0..1000).map(|i| (i * 7919) % 613).collect();
        let mut expected = v.clone();
        expected.sort();
        sort(&mut v, by_value);
        assert_eq!(v, expected);
        // Reversed.
        let mut v: Vec<i32> = (0..1000).rev().collect();
        sort(&mut v, by_value);
        assert_eq!(v, (0..1000).collect::<Vec<_>>());
        // Already sorted: about one comparison per item.
        let mut v: Vec<i32> = (0..1000).collect();
        let comparisons = sort(&mut v, by_value);
        assert_eq!(v, (0..1000).collect::<Vec<_>>());
        assert!(comparisons < 1200, "{comparisons} comparisons");
        // A comparator that isn't an ordering doesn't panic, and loses nothing.
        let mut v: Vec<i32> = (0..100).collect();
        sort(&mut v, |a, b| if (a + b) % 3 == 0 { -1 } else { 1 });
        v.sort();
        assert_eq!(v, (0..100).collect::<Vec<_>>());
    }
}

pub const FUNCTIONS: FunctionExports = &[export_c_func!(qsort(_, _, _, _))];
