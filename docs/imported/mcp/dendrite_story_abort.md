<!-- Imported from Dendrite docs/mcp/dendrite_story_abort.md @ 9cbfbeb on 2026-10-18. Source of truth for this repo's scope; keep in sync with the Dendrite copy until the extraction umbrella lands. -->

---
id: dendrite_story_abort
title: "dendrite_story_abort"
sidebar_label: "dendrite_story_abort"
---

# dendrite_story_abort

Safely abort and purge all speculative worktrees and branches for a story without merging

## Input Schema

```json
{
  "properties": {
    "story_id": {
      "description": "Target story identifier to clean up",
      "type": "string"
    }
  },
  "required": [
    "story_id"
  ],
  "type": "object"
}
```

## Example Call

### JSON-RPC

```json
{
  "id": 1,
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "arguments": {
      "story_id": "sample"
    },
    "name": "dendrite_story_abort"
  }
}
```

### CLI

```bash
dendrite mcp call dendrite_story_abort '{"story_id":"sample"}'
```
