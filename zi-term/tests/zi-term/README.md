# zi-term tests

zi-term contains very little logic and is primarily responsible for rendering. To ensure that it renders correctly we use the asciicast format to record the output of the terminal and compare it to the expected output.
This works by capturing the output of the `CrosstermBackend` and converting it to the [asciicast](https://docs.asciinema.org/manual/asciicast/v2/) format.

This can be viewed in the terminal using the `asciinema` tool.

```bash
asciinema play zi-term/tests/asciicast/example.cast
```

As with all snapshot tests, the point is to manually verify the output and to prevent regressions. If there is a desired change, run tests with `UPDATE_EXPECT=1` to update the expected output. A missing snapshot fails the test unless `UPDATE_EXPECT` is set.

The syntax highlighting snapshots need the rust and go grammars. They are read from `ZI_GRAMMAR_DIR` if set, otherwise from zi's data directory (`~/.local/share/zi/grammars` on Linux, `~/Library/Application Support/zi/grammars` on macOS). `nix develop` sets `ZI_GRAMMAR_DIR` to grammars built by the flake; to install a copy, run `just build-grammars` followed by `just install-grammars`. Set `ZI_GRAMMAR_DIR` to use a custom destination.

