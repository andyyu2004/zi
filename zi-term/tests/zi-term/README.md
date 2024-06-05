# zi-term tests

zi-term contains very little logic and is primarily responsible for rendering. To ensure that it renders correctly we use the asciicast format to record the output of the terminal and compare it to the expected output.
This works by capturing the output of the `CrosstermBackend` and converting it to the [asciicast](https://docs.asciinema.org/manual/asciicast/v2/) format.

This can be viewed in the terminal using the `asciinema` tool.

```bash
asciinema play zi-term/tests/asciicast/example.cast
```

As with all snapshot tests, the point is to manually verify the output and to prevent regressions. If there is a desired change, run tests with `UPDATE_EXPECT=1` to update the expected output.

