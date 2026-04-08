# cc-audit

A local CLI tool that parses your [Claude Code](https://claude.ai/code) session transcripts and turns them into actionable usage insights.

## The Problem

Claude Code stores detailed session data in `~/.claude/projects/`, but there's no built-in way to answer questions like:

- How many tokens am I burning per day/week/month?
- Which projects cost the most?
- What's my actual spend across Opus, Sonnet, and Haiku?
- Which tools and skills do I use most often?
- What did a specific session look like?

cc-audit reads these JSONL transcripts and gives you a CLI summary and a full web dashboard — no external services, no API keys, everything stays local.

## Install

Requires [Rust toolchain](https://rustup.rs/).

```bash
git clone https://github.com/anthropics/cc-audit.git  # or your fork
cd cc-audit
cargo install --path .
```

The binary is installed to `~/.cargo/bin/cc-audit`. Re-run `cargo install --path .` after pulling updates to get the latest version.

## Usage

### CLI Summary

```bash
cc-audit stats                # last 7 days (default)
cc-audit stats --range today  # today only
cc-audit stats --range 30d    # last 30 days
cc-audit stats --range all    # all time
```

Example output:

```
CC-Audit Stats (Last 7 days)
─────────────────────────────────────────
Total sessions:  42
Total turns:     318
Total tokens:    12.3M (input: 8.1M, output: 2.0M, cache_create: 1.5M, cache_read: 0.7M)
Total cost:      $47.20 (estimated)

Top projects:
   1. my-app                                  4.2M   $16.80
   2. infra                                   3.1M   $12.40
   ...
```

### Web Dashboard

```bash
cc-audit serve              # starts on localhost:3000
cc-audit serve --port 8080  # custom port
```

The dashboard provides:

- **Overview** — token trends, model breakdown, daily usage charts
- **Projects** — per-project token consumption, sorted by cost
- **Project Detail** — session list, token trends, tool distribution for a single project
- **Session Detail** — full conversation replay with rendered markdown
- **Tools & Skills** — tool call rankings, skill usage, agent subtype statistics

All pages support time-range filtering (today / 7d / 30d / all).

## How It Works

cc-audit is a single self-contained binary. It reads JSONL transcript files from `~/.claude/projects/`, aggregates token usage and tool calls in memory, and serves an HTMX-powered dashboard with all static assets embedded in the binary. No database, no Node.js, no external dependencies at runtime.

Cost estimates are based on [Anthropic's published pricing](https://docs.anthropic.com/en/docs/about-claude/models) for each model tier (Opus, Sonnet, Haiku), including cache read/write multipliers.

## Important Notes

cc-audit depends on the undocumented JSONL session log format that Claude Code writes to `~/.claude/projects/`. This is not a public API — Anthropic may change the log schema, relocate the data, or remove session logging entirely in any future update. If that happens, cc-audit may produce incorrect results or stop working altogether until it is updated to match the new format.

## License

MIT
