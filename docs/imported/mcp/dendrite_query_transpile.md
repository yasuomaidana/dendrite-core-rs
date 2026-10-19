<!-- Imported from Dendrite docs/mcp/dendrite_query_transpile.md @ 9cbfbeb on 2026-10-18. Source of truth for this repo's scope; keep in sync with the Dendrite copy until the extraction umbrella lands. -->

---
id: dendrite_query_transpile
title: "dendrite_query_transpile"
sidebar_label: "dendrite_query_transpile"
---

# dendrite_query_transpile

Translates legacy binaries or repos to safe, idiomatic Rust with Newtypes, zero unsafe blocks, and differential tests

## Input Schema

```json
{
  "properties": {
    "audit_library_reuse": {
      "description": "Enforce 3-tier library reuse hierarchy",
      "type": "boolean"
    },
    "no_std": {
      "description": "Generate no_std compatible embedded crate",
      "type": "boolean"
    },
    "output_dir": {
      "description": "Destination directory for synthesized crate",
      "type": "string"
    },
    "prompt": {
      "description": "Natural language architectural guidance",
      "type": "string"
    },
    "sources_artifact": {
      "description": "Target compiled binary or repository path",
      "type": "string"
    },
    "target_language": {
      "description": "Target language (default: rust)",
      "type": "string"
    }
  },
  "required": [
    "sources_artifact"
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
      "audit_library_reuse": false,
      "no_std": false,
      "output_dir": "sample",
      "prompt": "sample",
      "sources_artifact": "sample",
      "target_language": "rust"
    },
    "name": "dendrite_query_transpile"
  }
}
```

### CLI

```bash
dendrite mcp call dendrite_query_transpile '{"audit_library_reuse":false,"no_std":false,"output_dir":"sample","prompt":"sample","sources_artifact":"sample","target_language":"rust"}'
```
