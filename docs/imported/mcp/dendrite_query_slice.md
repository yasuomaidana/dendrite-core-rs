<!-- Imported from Dendrite docs/mcp/dendrite_query_slice.md @ 9cbfbeb on 2026-10-18. Source of truth for this repo's scope; keep in sync with the Dendrite copy until the extraction umbrella lands. -->

---
id: dendrite_query_slice
title: "dendrite_query_slice"
sidebar_label: "dendrite_query_slice"
---

# dendrite_query_slice

Universal intent-driven CFG/AST reachability slicer pruning dead features and generating golden I/O vectors

## Input Schema

```json
{
  "properties": {
    "direction": {
      "description": "Traversal direction",
      "enum": [
        "forward",
        "backward",
        "bidirectional"
      ],
      "type": "string"
    },
    "entrypoint_symbol": {
      "description": "Root function or method symbol for reachability traversal",
      "type": "string"
    },
    "intent": {
      "description": "Natural language architectural intent for extraction",
      "type": "string"
    },
    "output_dir": {
      "description": "Optional output path to emit debloated project \u0026 golden tests",
      "type": "string"
    },
    "sources_path": {
      "description": "Target source directory or binary path",
      "type": "string"
    }
  },
  "required": [
    "sources_path",
    "entrypoint_symbol"
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
      "direction": "sample",
      "entrypoint_symbol": "sample",
      "intent": "sample",
      "output_dir": "sample",
      "sources_path": "DEV_STATUS.md"
    },
    "name": "dendrite_query_slice"
  }
}
```

### CLI

```bash
dendrite mcp call dendrite_query_slice '{"direction":"sample","entrypoint_symbol":"sample","intent":"sample","output_dir":"sample","sources_path":"DEV_STATUS.md"}'
```
