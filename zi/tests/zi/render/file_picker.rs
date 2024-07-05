use super::*;
use crate::new;

async fn new_() -> TestContext {
    let cx = new("").with_size((100, 8)).await;
    cx.with(|editor| {
        editor.settings().file_picker_split_ratio.write((0, 100));
        editor.open_file_picker("tests/zi/testdirs");
    })
    .await;
    cx
}

#[tokio::test]
async fn file_picker() {
    let cx = new_().await;

    cx.snapshot(expect![[r#"
        "  |                                                                                                 "
        "  tests/zi/testdirs/a.txt                                                                           "
        "  tests/zi/testdirs/b.txt                                                                           "
        "  tests/zi/testdirs/binary.bin                                                                      "
        "  tests/zi/testdirs/c.txt                                                                           "
        "  tests/zi/testdirs/d.txt                                                                           "
        "buffer://picker:1:0                                                                                 "
        "-- INSERT --                                                                                        "
    "#]])
        .await;

    cx.with(|editor| editor.input("bin").unwrap()).await;

    // Preview the binary file shouldn't break. Currently just renders an empty buffer.
    cx.snapshot(expect![[r#"
        "  bin|                                                                                              "
        "  tests/zi/testdirs/binary.bin                                                                      "
        "                                                                                                    "
        "                                                                                                    "
        "                                                                                                    "
        "                                                                                                    "
        "buffer://picker:1:3                                                                                 "
        "-- INSERT --                                                                                        "
    "#]])
        .await;

    cx.cleanup().await;
}
