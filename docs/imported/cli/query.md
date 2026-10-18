<!-- Imported from Dendrite docs/cli/query.md @ 69ea048 on 2026-10-18. Source of truth for this repo's scope; keep in sync with the Dendrite copy until the extraction umbrella lands. -->

---
id: query
title: "dendrite query"
sidebar_label: "query"
---

# dendrite query

Intent-driven CFG/AST reachability slicing, debloater, transpile & fuse

## Category

Dendritic Arbor

## Aliases

- `dendritic-arbor`
- `arbor`
- `dend`

## Subcommands

| Subcommand | Description |
| :--- | :--- |
| `slice` | Universal intent-driven CFG/AST reachability slicing and debloater |
| `swap` | AST-aware pattern and framework replacement engine |
| `transpile` | Polyglot cross-language code translation with dual test parity |
| `fuse` | Multi-source AST blending and syntax tree synthesis |
| `reuse-audit` | AST token reuse audit and anti-duplication scanner |

## Flags

None

## Examples

```bash
dendrite query slice "verify docker container mounts"
dendrite query reuse-audit
```
