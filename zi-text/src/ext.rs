use zi_core::{Point, PointRange};

use crate::{Text, TextBase};

pub trait PointRangeExt {
    /// Turn a multi-line range into a iterator of single-line ranges.
    fn explode(self, text: impl Text) -> impl Iterator<Item = PointRange>;
}

impl PointRangeExt for PointRange {
    fn explode(self, text: impl Text) -> impl Iterator<Item = PointRange> {
        let start_line = self.start().line();
        let end_line = self.end().line();
        (start_line..=end_line).map(move |line| {
            let start_col = if line == start_line { self.start().col() } else { 0 };
            let end_col = if line == end_line {
                self.end().col()
            } else {
                text.line(line).unwrap().len_bytes()
            };
            PointRange::new(Point::new(line, start_col), Point::new(line, end_col.max(start_col)))
        })
    }
}

#[cfg(test)]
mod tests {
    use zi_core::{point, point_range};

    use super::*;

    #[test]
    fn explode() {
        let text = "abc\ndefghi\n";
        let range = PointRange::new(Point::new(0, 1), Point::new(1, 2));
        let actual = range.explode(text).collect::<Vec<_>>();
        let expected = vec![point_range!(0:1..0:3), point_range!(1:0..1:2)];

        assert_eq!(actual, expected);
    }
}
