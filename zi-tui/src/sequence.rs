use std::rc::Rc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::Element;

pub trait ElementSeq {
    /// Renders the views in the sequence to the given areas of the buffer
    /// You may assume `assert_eq!(areas.len(), self.len())`
    fn render(self, areas: Rc<[Rect]>, buf: &mut Buffer);

    /// Returns the number of views in the sequence (non-recursively)
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T: Element> ElementSeq for Vec<T> {
    fn render(self, areas: Rc<[Rect]>, buf: &mut Buffer) {
        assert_eq!(areas.len(), self.len());
        for (view, area) in self.into_iter().zip(areas.iter()) {
            view.render(*area, buf);
        }
    }

    fn len(&self) -> usize {
        self.len()
    }
}

impl<T: Element> ElementSeq for T {
    fn render(self, areas: Rc<[Rect]>, buf: &mut Buffer) {
        assert_eq!(areas.len(), self.len());
        self.render(areas[0], buf);
    }

    fn len(&self) -> usize {
        1
    }
}

macro_rules! impl_element_seq_tuple {
    ($len:literal $($idx:tt $t:tt),*) => {
        impl<$($t,)*> ElementSeq for ($($t,)*)
        where
            $($t: Element,)*
        {
            fn render(self, areas: Rc<[Rect]>, _buf: &mut Buffer) {
                assert_eq!(areas.len(), self.len());
                $(
                    self.$idx.render(areas[$idx], _buf);
                )*
            }

            fn len(&self) -> usize {
                $len
            }
        }
    };
}

impl_element_seq_tuple!(0);
impl_element_seq_tuple!(1 0 A);
impl_element_seq_tuple!(2 0 A, 1 B);
impl_element_seq_tuple!(3 0 A, 1 C, 2 D);
impl_element_seq_tuple!(4 0 A, 1 B, 2 C, 3 D);
impl_element_seq_tuple!(5 0 A, 1 B, 2 C, 3 D, 4 E);
impl_element_seq_tuple!(6 0 A, 1 B, 2 C, 3 D, 4 E, 5 F);
impl_element_seq_tuple!(7 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G);
impl_element_seq_tuple!(8 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H);
impl_element_seq_tuple!(9 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I);
impl_element_seq_tuple!(10 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J);
impl_element_seq_tuple!(11 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K);
impl_element_seq_tuple!(12 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L);
impl_element_seq_tuple!(13 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M);
impl_element_seq_tuple!(14 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M, 13 N);
impl_element_seq_tuple!(15 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M, 13 N, 14 O);
impl_element_seq_tuple!(16 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M, 13 N, 14 O, 15 P);
impl_element_seq_tuple!(17 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M, 13 N, 14 O, 15 P, 16 Q);
impl_element_seq_tuple!(18 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M, 13 N, 14 O, 15 P, 16 Q, 17 R);
impl_element_seq_tuple!(19 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M, 13 N, 14 O, 15 P, 16 Q, 17 R, 18 S);
impl_element_seq_tuple!(20 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M, 13 N, 14 O, 15 P, 16 Q, 17 R, 18 S, 19 T);
impl_element_seq_tuple!(21 0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K, 11 L, 12 M, 13 N, 14 O, 15 P, 16 Q, 17 R, 18 S, 19 T, 20 U);
