use super::*;

#[tokio::test]
async fn file_picker() {
    let cx = new(zi::Size::new(51, 10), "").await;

    // run(zi::Size::new(51, 10), "", |editor, mut snapshot| {
    //     editor.open_file_picker(".");
    //
    //     snapshot(
    //         editor.client(),
    //         expect![[r#"
    //             "   1                                               "
    //             "                                                   "
    //             "                                                   "
    //             "                                                   "
    //             "  |                                                "
    //             "                                                   "
    //             "                                                   "
    //             "                                                   "
    //             "picker:1:0                                         "
    //             "-- INSERT --                                       "
    //         "#]],
    //     );
    // })
    // .await;
}
