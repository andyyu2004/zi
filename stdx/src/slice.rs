use std::ops::Range;

pub trait SliceExt<T> {
    /// Given a slice sorted by the key, return a subslice where the key is within the given range.
    fn sorted_subslice_by_key<K: Ord>(
        &self,
        range: Range<K>,
        f: impl FnMut(&T) -> K + Copy,
    ) -> &[T];
}

impl<T> SliceExt<T> for [T] {
    fn sorted_subslice_by_key<K: Ord>(
        &self,
        range: Range<K>,
        mut f: impl FnMut(&T) -> K + Copy,
    ) -> &[T] {
        debug_assert!(
            self.array_windows().all(|[a, b]| f(a) <= f(b)),
            "input slice is not sorted by key"
        );

        let i = self.binary_search_by_key(&range.start, f).unwrap_or_else(|start| start);
        let j = self.binary_search_by_key(&range.end, f).unwrap_or_else(|end| end);
        let res = &self[i..j];
        debug_assert!(res.iter().all(|t| range.start <= f(t) && f(t) < range.end));
        res
    }
}

#[cfg(test)]
mod tests {
    use core::fmt;

    use super::*;

    #[test]
    fn test_sorted_subslice() {
        #[track_caller]
        fn check<T: Copy + Ord + fmt::Debug>(xs: &[T], range: Range<T>, expected: &[T]) {
            assert_eq!(xs.sorted_subslice_by_key(range, |&x| x), expected);
        }

        check(&[], 0..0, &[]);
        check(&[], 0..1, &[]);
        check(&[1, 2, 3], 0..0, &[]);
        check(&[1, 2, 3], 0..1, &[]);
        check(&[1, 2, 3], 0..2, &[1]);
    }
}
