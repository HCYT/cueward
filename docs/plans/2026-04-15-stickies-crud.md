# Stickies CRUD Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `cueward stickies` list/create/update/delete commands on macOS using AppleScript automation against the Stickies app.

**Architecture:** Keep Stickies isolated as its own adapter module and top-level CLI command rather than mixing it into notes or quick notes. Implement a read model for Stickies notes plus a pragmatic first-version identifier strategy: use the most stable app-visible key available, and fall back to an application-level fingerprint only if AppleScript cannot expose a usable note identifier. Keep this batch focused on plain-text title/body CRUD only.

**Tech Stack:** Rust 2024, `clap`, AppleScript via `osascript`, `serde`/`serde_json`, `cargo test`

---

## Chunk 1: CLI Contract

### Task 1: Define the `stickies` CLI surface with failing parse tests

**Files:**
- Modify: `crates/cli/src/commands/mod.rs`
- Create: `crates/cli/src/commands/stickies.rs`
- Create: `crates/cli/src/commands/stickies_tests.rs`

- [ ] **Step 1: Write the failing CLI parse tests**

Add tests covering:

```rust
#[test]
fn cli_parses_stickies_list() {
    let cli = Cli::try_parse_from(["cueward", "stickies", "list"])
        .expect("parse stickies list");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::List,
        } => {}
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_stickies_create() {
    let cli = Cli::try_parse_from([
        "cueward",
        "stickies",
        "create",
        "--title",
        "臨時待辦",
        "--body",
        "記得回覆客戶",
    ])
    .expect("parse stickies create");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::Create { title, body },
        } => {
            assert_eq!(title, "臨時待辦");
            assert_eq!(body, "記得回覆客戶");
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_stickies_update() {
    let cli = Cli::try_parse_from([
        "cueward",
        "stickies",
        "update",
        "--id",
        "sticky-1",
        "--title",
        "新標題",
        "--body",
        "更新後內容",
    ])
    .expect("parse stickies update");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::Update { id, title, body },
        } => {
            assert_eq!(id, "sticky-1");
            assert_eq!(title.as_deref(), Some("新標題"));
            assert_eq!(body.as_deref(), Some("更新後內容"));
        }
        _ => panic!("unexpected command"),
    }
}

#[test]
fn cli_parses_stickies_delete() {
    let cli = Cli::try_parse_from([
        "cueward",
        "stickies",
        "delete",
        "--id",
        "sticky-1",
    ])
    .expect("parse stickies delete");

    match cli.command {
        Command::Stickies {
            action: StickiesAction::Delete { id },
        } => assert_eq!(id, "sticky-1"),
        _ => panic!("unexpected command"),
    }
}
```

- [ ] **Step 2: Run the CLI tests to verify they fail**

Run:
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: FAIL because `Command::Stickies` / `StickiesAction` do not exist yet.

- [ ] **Step 3: Add the minimal CLI enums and dispatch stubs**

Implement:
- `Command::Stickies { action: StickiesAction }`
- `StickiesAction::{List, Create, Update, Delete}`
- `crates/cli/src/commands/stickies.rs` with a `dispatch()` function matching existing command modules

Keep dispatch compiling even if adapter calls are still stubbed.

- [ ] **Step 4: Re-run the CLI tests to verify they pass**

Run:
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/cli/src/commands/mod.rs crates/cli/src/commands/stickies.rs crates/cli/src/commands/stickies_tests.rs
git commit -m "test: define stickies cli contract"
```

## Chunk 2: Stickies Read Model And Identifier Strategy

### Task 2: Implement list/read with a stable first-version identifier

**Files:**
- Modify: `crates/adapter-macos/src/lib.rs`
- Create: `crates/adapter-macos/src/stickies.rs`

- [ ] **Step 1: Write the failing adapter tests for decoding and identifier behavior**

Add tests in `crates/adapter-macos/src/stickies.rs` for:

```rust
#[test]
fn parse_stickies_output_keeps_title_body_and_id() {
    let raw = "sticky-1\t臨時待辦\t記得回覆客戶---STICKY_SEP---";

    let notes = parse_stickies_output(raw);

    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].id, "sticky-1");
    assert_eq!(notes[0].title, "臨時待辦");
    assert_eq!(notes[0].body, "記得回覆客戶");
}

#[test]
fn parse_stickies_output_falls_back_when_title_missing() {
    let raw = "sticky-1\t\t只有內容---STICKY_SEP---";

    let notes = parse_stickies_output(raw);

    assert_eq!(notes.len(), 1);
    assert!(!notes[0].title.is_empty());
}
```

Also cover:
- escaping/unescaping tab/newline/record separator
- update validation requires at least one field

- [ ] **Step 2: Run the adapter tests to verify they fail**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: FAIL because the module and parser do not exist yet.

- [ ] **Step 3: Implement the read model and parser**

Create:
- `StickiesNote { id, title, body }`

Implement:
- output separator constants like reminders/calendar
- field escape/decode helpers reusing `applescript.rs` primitives where possible
- `parse_stickies_output()`
- `ensure_update_fields()` or equivalent validator

Identifier rule:
- First try to read a real app-level key from AppleScript
- If unavailable, generate a deterministic fallback fingerprint from the note’s visible fields in a way that survives a list → update/delete round-trip during normal use

- [ ] **Step 4: Add the AppleScript list builder**

Build a list script that:
- opens `tell application "Stickies"`
- enumerates all notes/windows
- emits `id`, `title`, `body`
- leaves title empty if app provides none so Rust fallback logic owns the default label

Do not add create/update/delete yet.

- [ ] **Step 5: Re-run the adapter tests**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`

Expected: PASS for parser and validation tests.

- [ ] **Step 6: Commit**

```bash
git add crates/adapter-macos/src/lib.rs crates/adapter-macos/src/stickies.rs
git commit -m "feat: add stickies read model"
```

## Chunk 3: Create / Update / Delete Operations

### Task 3: Implement Stickies CRUD operations behind the adapter

**Files:**
- Modify: `crates/adapter-macos/src/stickies.rs`
- Modify: `crates/cli/src/commands/stickies.rs`

- [ ] **Step 1: Write failing operation tests**

Add unit tests for the script builders:

```rust
#[test]
fn create_sticky_script_escapes_multiline_body() {
    let script = build_create_script("臨時待辦", "line1\nline2");

    assert!(script.contains(r#"make new note"#));
    assert!(script.contains("\"line1\" & linefeed & \"line2\""));
}

#[test]
fn update_sticky_script_targets_id() {
    let script = build_update_script("sticky-1", Some("新標題"), Some("更新後內容"))
        .expect("script");

    assert!(script.contains("sticky-1"));
    assert!(script.contains("新標題"));
    assert!(script.contains("更新後內容"));
}

#[test]
fn update_sticky_script_requires_title_or_body() {
    let err = build_update_script("sticky-1", None, None).unwrap_err();
    assert!(err.to_string().contains("no sticky updates specified"));
}
```

- [ ] **Step 2: Run the targeted tests to verify they fail**

Run:
`cargo test -p cueward-adapter-macos sticky_ -- --nocapture`

Expected: FAIL because operation builders and adapter functions are missing.

- [ ] **Step 3: Implement create/update/delete in the adapter**

Add functions:
- `list_stickies() -> Result<Vec<StickiesNote>, MacosError>`
- `create_sticky(title: &str, body: &str) -> Result<StickiesNote, MacosError>`
- `update_sticky(id: &str, title: Option<&str>, body: Option<&str>) -> Result<StickiesNote, MacosError>`
- `delete_sticky(id: &str) -> Result<(), MacosError>`

Rules:
- update/delete must resolve by `id`
- not found → explicit error
- ambiguous resolution → explicit error
- create/update should return the post-operation note shape if practical; if not, CLI may emit the requested payload plus `updated: true`

- [ ] **Step 4: Wire CLI dispatch to adapter**

In `crates/cli/src/commands/stickies.rs`:
- `list` prints JSON array
- `create` / `update` / `delete` print structured JSON matching the spec
- failures go to stderr and exit 1

- [ ] **Step 5: Re-run targeted tests**

Run:
`cargo test -p cueward-adapter-macos stickies -- --nocapture`
`cargo test -p cueward-cli cli_parses_stickies -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/adapter-macos/src/stickies.rs crates/cli/src/commands/stickies.rs
git commit -m "feat: add stickies crud operations"
```

## Chunk 4: Full Verification And Output Sanity

### Task 4: Verify the whole feature end to end at the code level

**Files:**
- Modify only if implementation drift requires docs alignment:
  - `docs/specs/2026-04-15-stickies-crud.md`
  - `docs/plans/2026-04-15-stickies-crud.md`

- [ ] **Step 1: Run full verification**

Run:
`cargo test`
`cargo build --release`

Expected: PASS

- [ ] **Step 2: Confirm success criteria against the issue**

Check explicitly:
- `stickies list` has `id/title/body`
- `stickies create` exists
- `stickies update` refuses empty updates
- `stickies delete` resolves by `id`
- no-title notes do not break list output

- [ ] **Step 3: Prepare PR summary**

PR summary should call out:
- first-version `id` strategy used
- plain-text-only scope
- what was intentionally deferred (style/position/rich text)

Plan complete and saved to `docs/plans/2026-04-15-stickies-crud.md`. Ready to execute?
