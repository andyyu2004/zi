use super::*;

#[tokio::test]
async fn file_picker() {
    let cx = new_cx_with_size(zi::Size::new(100, 8), "").await;

    cx.with(|editor| {
        editor.settings().picker_split_proportion.write((0, 100));
        editor.open_file_picker("tests/zi/testdirs");
    })
    .await;

    cx.snapshot(expect![[r#"
        "  |                                                                                                 "
        "  tests/zi/testdirs/binary.bin                                                                      "
        "  tests/zi/testdirs/test.txt                                                                        "
        "                                                                                                    "
        "                                                                                                    "
        "                                                                                                    "
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
