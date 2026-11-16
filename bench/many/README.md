# Many agents on one Hush host

Every `hush mcp` and shell command runs in one per-user host process, each agent in its own
session (see the README's "Many agents at once"). These scripts check that it stays fast,
light and correct with many sessions at once. They only use Hush's MCP tools, through real
`hush mcp` pipes, so they measure what agents get.

- `sessions.py HUSH N ROUNDS`: N sessions at once, each repeating observe, `see`, the menu
  catalog and an AppleScript. Prints the median, p95 and worst time per tool and the host's
  peak CPU, memory and threads.
- `same_app.py HUSH K`: K sessions type different letters into one text field of the fixture
  app at the same moment. Passes when the field holds K whole blocks.

## Result (16 Nov 2026, M4 Pro, macOS 27.2)

| | Result |
|---|---|
| 10 sessions × 50 calls on FilmCraft and Finder | 500 calls in 2.2 s, 0 errors |
| Median / p95 | observe 48 / 96 ms, `see` 29 / 75 ms, AppleScript 5 / 52 ms |
| Host peak | 91% of one core, 103 MB, 27 threads; 0% CPU when idle |
| 1 session, same calls | observe 33 ms, `see` 30 ms median; host 45 MB, 12 threads |
| 5 sessions typing into one field at once | 0.20 s, five whole 12-letter blocks, the app never activated |

Before the host, each agent's `hush mcp` was its own process, and macOS serves window captures
to one live process per program file: the second agent's captures waited forever.
