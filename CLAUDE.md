# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

CC-Audit is a personal, local tool that parses Claude Code session transcripts (`~/.claude/projects/`) to provide token usage visualization and behavioral analysis. Single Rust binary with an embedded web dashboard.

## Commands

```bash
# Build
cargo build --release

# Run CLI stats (default range: 7d)
cargo run -- stats --range 30d    # options: today, 7d, 30d, all

# Start web dashboard (default port: 3000)
cargo run -- serve --port 3000

# Tests (integration tests run against real data in ~/.claude/projects/)
cargo test -- --nocapture

# Lint & format
cargo fmt
cargo clippy
```

## Architecture

**Tech stack**: Rust + axum + askama (compile-time templates) + HTMX + Chart.js. All static assets embedded in the binary via rust-embed.

**Data flow**:
```
~/.claude/projects/**/*.jsonl
    → Parser (discovery + JSONL parsing)
    → TranscriptEntry enum (typed message variants)
    → Aggregator (stats, cost, session detail)
    → CLI output OR axum handlers → askama templates → HTML with HTMX
```

**Key modules**:
- `src/parser/` — JSONL parsing layer: `discovery.rs` finds projects, `jsonl.rs` parses lines, `models.rs` defines the `TranscriptEntry` enum, `session_index.rs` reads sessions-index.json
- `src/aggregator/` — Data computation: `stats.rs` (aggregation + GlobalStats/ProjectDetailStats), `cost.rs` (model-specific pricing), `session.rs` (conversation replay with markdown rendering)
- `src/web/mod.rs` — All axum routes and askama template structs
- `src/cli/mod.rs` — CLI subcommands (stats, serve)
- `templates/` — Askama HTML templates using `{% extends "base.html" %}` pattern; `*_partial.html` variants for HTMX partial updates
- `static/` — Embedded frontend assets (htmx.min.js, chart.umd.min.js, style.css)

**Important implementation details**:
- Only assistant messages with `stop_reason` set are counted (filters out intermediate streaming chunks)
- Sessions are deduplicated via HashSet on session_id
- Project path resolution: tries sessions-index.json `projectPath` first, then `cwd` from first assistant message, then decodes from directory name
- Chart.js canvases must be destroyed and recreated on HTMX partial swaps (see `freshCanvas()` in base.html)
- `TranscriptEntry::Other` captures unknown message types for forward compatibility
- Tool/skill/agent extraction happens from `ContentBlock::ToolUse` entries in assistant messages

## Adding New Features

- **New message type**: Add variant to `TranscriptEntry` enum in `src/parser/models.rs`, update parsing in `jsonl.rs`
- **New aggregation metric**: Modify `aggregate()` in `src/aggregator/stats.rs`
- **New dashboard page**: Create template in `templates/` (full + partial), add handler + route in `src/web/mod.rs`
- **New model pricing**: Update `src/aggregator/cost.rs`
