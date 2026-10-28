# rask

Tasks for Rust projects, planned in feature files, marked in the code, and checked rather than declared.

- **A task is a scenario.** `Scenario: SH1. A lit user shader` in any `.feature` file is task SH1. Tags carry its
  order: `@needs:SH0a,SH0b` above it means those land first.
- **The plan is in the scenario.** A table under a step that says "the plan" lists the Rust files it adds or grows,
  with each file's line budget and its one job:

  ```gherkin
  Scenario: SH1. A lit user shader on StandardMaterial
    Given the plan
      | crate             | file              | lines | job                                        |
      | raea-sys-material | src/shader_ext.rs | 260   | the user's extension over StandardMaterial |
  ```

  `rask plan SH1` makes the files, each with its job as the module doc and a task mark. Nothing is added to `lib.rs`:
  a `mod` line goes in once the file compiles. A plan that puts a file over 500 lines, or a crate over 2,000 lines of
  its own `.rs`, is refused.
- **Unfinished work is marked.** `// rask: SH1b the bind group for the user's uniforms` is a task mark (tree-sitter
  reads them). A `todo!()`, `unimplemented!()` or TODO/FIXME/XXX comment belongs to a task when it says `rask: ID`
  or sits in, or just under, a function a task mark is on. Any other stub is **loose**, and `rask check` fails on it.
- **Done is said, then checked.** The commit that finishes SH1 tags its scenario `@done` and writes its status there.
  `rask done SH1` closes it only when that tag is on `origin/main`, commits whose messages start `SH1: ` are there and
  changed the feature file, no `rask: SH1` mark is left, and no file they touched holds a loose stub. A commit named
  `SH1: ` without the tag is progress, not the end.
- **Claims are live.** `rask claim SH1 shaders` gives SH1 to the lane `shaders` and makes that GitButler branch if
  it's missing. Claims live outside git (`~/.local/state/rask`, or `RASK_BOARD`), so every branch and session sees
  them at once. `rask ls`, `rask ready` and `rask show ID` read the board.

## In rust-analyzer

Point flycheck at rask, which runs `cargo check` and adds its own findings in rustc's JSON:

```json
{ "rust-analyzer.check.overrideCommand": ["rask", "check", "--cargo", "--json", "--workspace", "--all-targets"] }
```

Open tasks show as information on their marks. Loose stubs, files and crates over their limits, and marks naming no
scenario show as errors.

## Commands

```
rask ls [--all] [--lane L]      tasks in play (--all: every scenario)
rask ready                      open tasks whose @needs are done
rask show ID                    scenario, plan, marks, log
rask check [--json] [--cargo [ARGS..]]
rask plan [FILE:]ID
rask claim ID LANE | rask block ID WHY | rask open ID | rask note ID TEXT
rask done ID [--main REF]
```

An id two feature files share is given as `FILE:ID`.

## Licence

MIT or Apache-2.0, at your option.
