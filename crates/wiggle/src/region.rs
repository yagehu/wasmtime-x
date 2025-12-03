use std::ops;

use crate::Width;

/// Represents a contiguous region in memory.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Region<W: Width> {
    pub start: W,
    pub len: W,
}

impl<W> Region<W>
where
    W: Width + ops::Sub<Output = W> + ops::Add<Output = W> + ops::Mul<Output = W> + PartialOrd,
{
    pub fn new(start: W, len: W) -> Self {
        Self { start, len }
    }

    /// Checks if this `Region` overlaps with `rhs` `Region`.
    pub fn overlaps(&self, rhs: Region<W>) -> bool {
        // Zero-length regions can never overlap!
        if self.len == W::try_from(0).unwrap() || rhs.len == W::try_from(0).unwrap() {
            return false;
        }

        let self_end = self.start + (self.len - W::try_from(1).unwrap());
        let rhs_end = rhs.start + (rhs.len - W::try_from(1).unwrap());

        if self.start <= rhs.start {
            self_end >= rhs.start
        } else {
            rhs_end >= self.start
        }
    }

    pub fn extend(&self, times: W) -> Self {
        let len = self.len * times;
        Self {
            start: self.start,
            len,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn zero_length() {
        let r1 = Region::<u32>::new(0, 0);
        let r2 = Region::<u32>::new(0, 1);
        assert!(!r1.overlaps(r2));

        let r1 = Region::<u32>::new(0, 1);
        let r2 = Region::<u32>::new(0, 0);
        assert!(!r1.overlaps(r2));
    }

    #[test]
    fn nonoverlapping() {
        let r1 = Region::<u32>::new(0, 10);
        let r2 = Region::<u32>::new(10, 10);
        assert!(!r1.overlaps(r2));

        let r1 = Region::<u32>::new(10, 10);
        let r2 = Region::<u32>::new(0, 10);
        assert!(!r1.overlaps(r2));
    }

    #[test]
    fn overlapping() {
        let r1 = Region::<u32>::new(0, 10);
        let r2 = Region::<u32>::new(9, 10);
        assert!(r1.overlaps(r2));

        let r1 = Region::<u32>::new(0, 10);
        let r2 = Region::<u32>::new(2, 5);
        assert!(r1.overlaps(r2));

        let r1 = Region::<u32>::new(9, 10);
        let r2 = Region::<u32>::new(0, 10);
        assert!(r1.overlaps(r2));

        let r1 = Region::<u32>::new(2, 5);
        let r2 = Region::<u32>::new(0, 10);
        assert!(r1.overlaps(r2));
    }
}
