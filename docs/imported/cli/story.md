<!-- Imported from Dendrite docs/cli/story.md @ 69ea048 on 2026-10-18. Source of truth for this repo's scope; keep in sync with the Dendrite copy until the extraction umbrella lands. -->

---
id: story
title: "dendrite story"
sidebar_label: "story"
---

# dendrite story

Autonomous multi-turn story execution and DAG branching

## Category

Postsynaptic Effectors & Actuation

## Aliases

- `stories`
- `synaptic-saga`
- `saga`
- `sty`

## Subcommands

| Subcommand | Description |
| :--- | :--- |
| `next` | Execute next story task from Plane DAG |
| `fork` | Create speculative branch for alternative implementation |
| `resolve` | Merge winning branch and reconcile ticket state |

## Flags

None

## Examples

```bash
dendrite story next
```
