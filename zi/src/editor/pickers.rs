use super::*;
use crate::{Mark, lstypes};

impl Editor {
    pub fn open_file_explorer(&mut self, path: impl AsRef<Path>) {
        inner(self, path.as_ref());

        fn inner(editor: &mut Editor, path: &Path) {
            let mut injector = None;
            let buf = editor.buffers.insert_with_key(|id| {
                let (explorer, inj) = ExplorerBuffer::new(
                    id,
                    path.to_path_buf(),
                    nucleo::Config::DEFAULT.match_paths(),
                    request_redraw,
                    |editor, path: Relative| {
                        let path = path.into_inner();
                        if path.is_dir() {
                            editor.open_file_explorer(path);
                        } else {
                            match editor.open(path, OpenFlags::SPAWN_LANGUAGE_SERVICES) {
                                Ok(fut) => editor.spawn("explorer open", async move {
                                    let _ = fut.await?;
                                    Ok(())
                                }),
                                Err(err) => editor.set_error(err),
                            }
                        }
                    },
                );
                injector = Some(inj);
                Buffer::new(explorer)
            });

            let injector = injector.unwrap();
            editor.set_buffer(Active, buf);
            editor.set_mode(Mode::Normal);

            // Cannot use parallel iterator as it doesn't sort.
            let walk = ignore::WalkBuilder::new(path)
                .max_depth(Some(1))
                .hidden(false)
                .ignore(false)
                .sort_by_file_path(|a, b| {
                    if a.is_dir() && !b.is_dir() {
                        cmp::Ordering::Less
                    } else if !a.is_dir() && b.is_dir() {
                        cmp::Ordering::Greater
                    } else {
                        a.cmp(b)
                    }
                })
                .build();

            let path = path.to_path_buf();
            pool().spawn(move || {
                let _ = injector.push(PathBuf::from("..").display_relative_to(&path));
                for entry in walk {
                    let Ok(entry) = entry else { continue };
                    if let Err(()) = injector.push(entry.into_path().display_relative_to(&path)) {
                        break;
                    }
                }
            })
        }
    }

    pub(super) fn open_static_picker<P>(
        &mut self,
        view_group_url: Url,
        path: impl AsRef<Path>,
        split_ratio: (u16, u16),
        f: impl FnOnce(&mut Self, Injector<P::Entry>),
    ) -> ViewGroupId
    where
        P: Picker,
    {
        self.open_picker::<P>(view_group_url, path, split_ratio, None, f)
    }

    fn open_dynamic_picker<P>(
        &mut self,
        view_group_url: Url,
        path: impl AsRef<Path>,
        split_ratio: (u16, u16),
        dynamic_source: impl Fn(Injector<P::Entry>, &str) + Send + Sync + 'static,
    ) -> ViewGroupId
    where
        P: Picker,
    {
        self.open_picker::<P>(
            view_group_url,
            path,
            split_ratio,
            Some(Arc::new(dynamic_source)),
            |_, _| {},
        )
    }

    fn open_picker<P>(
        &mut self,
        view_group_url: Url,
        path: impl AsRef<Path>,
        split_ratio: (u16, u16),
        dynamic_source: Option<DynamicHandler<P::Entry>>,
        f: impl FnOnce(&mut Self, Injector<P::Entry>),
    ) -> ViewGroupId
    where
        P: Picker,
    {
        let prev_mode = mode!(self);
        let view_group = match self.create_view_group(view_group_url) {
            Ok(view_group) => view_group,
            Err(id) => return id,
        };

        self.set_mode(Mode::Insert);

        let preview_buf = self.create_readonly_buffer("preview", &b""[..]);
        let preview = self.views.insert_with_key(|id| {
            let view = View::new(id, preview_buf).with_group(view_group);
            view.settings().line_number_style.write(tui::LineNumberStyle::None);
            view
        });

        self.tree.push(Layer::new_with_area(preview, move |area| {
            tui::Layout::vertical(tui::Constraint::from_fills([split_ratio.0, split_ratio.1]))
                .areas::<2>(area)[1]
        }));

        let display_view = self.split(Active, Direction::Left, tui::Constraint::Fill(1));
        let theme = self.theme();
        let theme = theme.read();
        self.views[display_view].set_buffer(self.buffers.insert_with_key(|id| {
            Buffer::new(TextBuffer::new(
                id,
                BufferFlags::empty(),
                filetype!(text),
                path,
                Rope::new(),
                &theme,
                None,
            ))
        }));

        let search_view = self.split(Active, Direction::Up, tui::Constraint::Max(1));
        assert_eq!(self.tree().active(), search_view);

        // ensure all views are in the same group so they close together
        self.views[display_view].set_group(view_group);
        self.views[search_view].set_group(view_group);

        event::subscribe_with::<event::DidCloseView>({
            move |editor, event| {
                // restore the mode if the picker view group is closed
                if editor.views[event.view].group() == Some(view_group) {
                    editor.set_mode(prev_mode);
                    event::HandlerResult::Unsubscribe
                } else {
                    event::HandlerResult::Continue
                }
            }
        });

        let mut injector = None;
        let picker_buf = self.buffers.insert_with_key(|id| {
            let mut picker = PickerBuffer::new(id, display_view, request_redraw, P::new(preview));
            injector = Some(picker.injector());
            if let Some(source) = dynamic_source {
                picker = picker.with_dynamic_handler(source);
            }
            Buffer::new(picker)
        });

        f(self, injector.unwrap());

        self.set_buffer(search_view, picker_buf);

        view_group
    }

    pub fn open_jump_list(&mut self, selector: impl Selector<ViewId>) -> ViewGroupId {
        #[derive(Clone, Debug)]
        struct Jump {
            path: PathBuf,
            point: Point,
        }

        impl fmt::Display for Jump {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.path.display(), self.point)
            }
        }

        impl BufferPickerEntry for Jump {
            #[inline]
            fn buffer_or_path(&self) -> Result<BufferId, &Path> {
                Err(&self.path)
            }

            #[inline]
            fn point(&self) -> Option<lstypes::EncodedPoint> {
                Some(self.point.into())
            }
        }

        // Save the view so the jumps we get are from the right view.
        let view = self.view(selector).id();
        let split_ratio = *self.settings().generic_picker_split_ratio.read();
        self.open_static_picker::<BufferPicker<_>>(
            Url::parse("view-group://jumps").unwrap(),
            "jumps",
            split_ratio,
            move |editor, injector| {
                for loc in editor.view(view).jump_list().iter() {
                    let Some(path) = editor.buffer(loc.buf).file_path() else { continue };
                    if let Err(()) = injector.push(Jump { path, point: loc.point }) {
                        break;
                    }
                }
            },
        )
    }

    pub fn open_file_picker(&mut self, path: impl AsRef<Path>) -> ViewGroupId {
        let path = path.as_ref();
        let split_ratio = *self.settings().file_picker_split_ratio.read();
        self.open_static_picker::<BufferPicker<stdx::path::Display>>(
            Url::parse("view-group://files").unwrap(),
            path,
            split_ratio,
            |_editor, injector| {
                let mut entries =
                    ignore::WalkBuilder::new(path).build().filter_map(|entry| match entry {
                        Ok(entry) => match entry.file_type() {
                            Some(ft) if ft.is_file() => Some(entry),
                            _ => None,
                        },
                        Err(err) => {
                            tracing::error!(%err, "file picker error");
                            None
                        }
                    });

                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(50);
                for entry in entries.by_ref() {
                    if let Err(()) = injector.push(entry.into_path().display_owned()) {
                        break;
                    }

                    if std::time::Instant::now() > deadline {
                        pool().spawn(move || {
                            for entry in entries {
                                if let Err(()) = injector.push(entry.into_path().display_owned()) {
                                    break;
                                }
                            }
                        });
                        break;
                    }
                }
            },
        )
    }

    pub fn open_marks(&mut self, selector: impl Selector<BufferId>) -> Option<ViewGroupId> {
        #[derive(Clone, Debug)]
        struct MarkEntry {
            buf: BufferId,
            namespace: Ustr,
            range: PointRange,
            // TODO show something once there's more mark metadata
            #[allow(unused)]
            mark: Mark,
        }

        impl fmt::Display for MarkEntry {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}", self.namespace, self.range)
            }
        }

        impl BufferPickerEntry for MarkEntry {
            #[inline]
            fn buffer_or_path(&self) -> Result<BufferId, &Path> {
                Ok(self.buf)
            }

            #[inline]
            fn point(&self) -> Option<lstypes::EncodedPoint> {
                Some(self.range.start().into())
            }
        }

        let buf = selector.select(self);

        let ratio = *self.settings().generic_picker_split_ratio.read();

        let vg_id = self.open_static_picker::<BufferPicker<MarkEntry>>(
            Url::parse("view-group://marks").unwrap(),
            "marks",
            ratio,
            |editor, injector| {
                for (ns, range, mark) in editor.buffer(buf).marks(..) {
                    if let Err(()) = injector.push(MarkEntry {
                        buf,
                        namespace: editor[ns].name(),
                        range: editor[buf].text().byte_range_to_point_range(&range),
                        mark: mark.clone(),
                    }) {
                        break;
                    }
                }
            },
        );

        Some(vg_id)
    }

    pub fn open_diagnostics(&mut self) -> ViewGroupId {
        #[derive(Clone, Debug)]
        struct DiagnosticEntry {
            path: PathBuf,
            range: lstypes::EncodedRange,
            message: String,
        }

        impl fmt::Display for DiagnosticEntry {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{}: {}", self.path.display(), self.range, self.message)
            }
        }

        impl BufferPickerEntry for DiagnosticEntry {
            #[inline]
            fn buffer_or_path(&self) -> Result<BufferId, &Path> {
                Err(&self.path)
            }

            #[inline]
            fn point(&self) -> Option<lstypes::EncodedPoint> {
                Some(self.range.start())
            }
        }

        let split_ratio = *self.settings().diagnostics_picker_split_ratio.read();
        self.open_static_picker::<BufferPicker<DiagnosticEntry>>(
            Url::parse("view-group://diagnostics").unwrap(),
            "diagnostics",
            split_ratio,
            |editor, injector| {
                for (path, server_diags) in editor.diagnostics.iter() {
                    for diag in &server_diags.read().1 {
                        if let Err(()) = injector.push(DiagnosticEntry {
                            path: path.clone(),
                            range: diag.range.clone(),
                            message: diag.message.clone(),
                        }) {
                            break;
                        }
                    }
                }
            },
        )
    }

    pub fn open_global_search(&mut self, path: impl AsRef<Path>) -> ViewGroupId {
        #[derive(Clone, Debug)]
        struct Entry {
            #[allow(unused)]
            // TODO can be used to highlight the matching portion of the line
            byte_range: ops::Range<usize>,
            path: PathBuf,
            line: usize,
            content: String,
        }

        impl BufferPickerEntry for Entry {
            #[inline]
            fn buffer_or_path(&self) -> Result<BufferId, &Path> {
                Err(&self.path)
            }

            #[inline]
            fn point(&self) -> Option<lstypes::EncodedPoint> {
                Some(Point::new(self.line, 0).into())
            }
        }

        impl fmt::Display for Entry {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}:{} {}", self.path.display(), self.line, self.content)
            }
        }

        let path = path.as_ref().to_path_buf();
        let split_ratio = *self.settings().global_search_split_ratio.read();
        self.open_dynamic_picker::<BufferPicker<Entry>>(
            Url::parse("view-group://search").unwrap(),
            "search",
            split_ratio,
            move |injector, query| {
                tracing::debug!(%query, "global search update");

                let matcher = search::matcher(query);
                let searcher = search::searcher();

                let walk = ignore::WalkBuilder::new(&path).build_parallel();

                pool().spawn(move || {
                    walk.run(|| {
                        let injector = injector.clone();
                        let mut searcher = searcher.clone();
                        let matcher = matcher.clone();

                        Box::new(move |entry| {
                            let entry = match entry {
                                Ok(entry) => match entry.file_type() {
                                    Some(ft) if ft.is_file() => entry,
                                    _ => return WalkState::Continue,
                                },
                                Err(_) => return WalkState::Continue,
                            };

                            let mut quit = false;
                            let sink = search::Sink(|line, content, byte_range| {
                                quit = injector
                                    .push(Entry {
                                        byte_range,
                                        line: line.checked_sub(1).expect("1-indexed") as usize,
                                        path: entry.path().to_path_buf(),
                                        content: content.trim_end().to_string(),
                                    })
                                    .is_err();
                                Ok(!quit)
                            });

                            // TODO search buffers first so unsaved content will show

                            if let Err(err) = searcher.search_path(&matcher, entry.path(), sink) {
                                tracing::error!(%err, "global search error");
                            }

                            if quit {
                                tracing::debug!("global search cancelled");
                                WalkState::Quit
                            } else {
                                WalkState::Continue
                            }
                        })
                    })
                });
            },
        )
    }
}
