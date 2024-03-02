use expect_test::expect;
use slotmap::KeyData;

use super::*;

#[test]
fn test_split() {
    let a = ViewId::from(KeyData::from_ffi(0));
    let b = ViewId::from(KeyData::from_ffi(1));

    let mut layer = Layer::new(Rect::new(0, 0, 100, 100), a);
    expect![[r#"
        Layer {
            active: ViewId(
                0v1,
            ),
            area: Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            root: View(
                ViewId(
                    0v1,
                ),
            ),
        }
    "#]]
    .assert_debug_eq(&layer);

    layer.split(a, b, Direction::Right);

    expect![[r#"
        Layer {
            active: ViewId(
                1v1,
            ),
            area: Rect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            root: Container(
                Container {
                    layout: Layout {
                        direction: Horizontal,
                        constraints: [
                            Fill(
                                1,
                            ),
                            Fill(
                                1,
                            ),
                        ],
                        margin: Margin {
                            horizontal: 0,
                            vertical: 0,
                        },
                        flex: Start,
                        spacing: 0,
                    },
                    children: [
                        View(
                            ViewId(
                                0v1,
                            ),
                        ),
                        View(
                            ViewId(
                                1v1,
                            ),
                        ),
                    ],
                },
            ),
        }
    "#]]
    .assert_debug_eq(&layer);
}
