# Crate Dependency Graph

This document shows how the `px-*` crates (and `siril-sys`) depend on each other within the workspace.

## Graph

```mermaid
graph TD
    px --> px-cli
    px --> px-configuration
    px --> px-conventions
    px --> px-fits
    px --> px-index
    px --> px-fs
    px --> px-nativeui
    px --> px-pipeline
    px --> siril-sys

    px-cli --> px-static

    px-conventions --> px-configuration
    px-conventions --> px-fs

    px-fits --> px-fs

    px-index --> px-configuration
    px-index --> px-conventions
    px-index --> px-fits

    px-nativeui --> px-fits

    px-pipeline --> px-fits
    px-pipeline --> px-fs
    px-pipeline --> siril-sys
```

## Dependency Table

| Crate | Depends On |
|---|---|
| `px` | `px-cli`, `px-configuration`, `px-conventions`, `px-fits`, `px-fs`, `px-index`, `px-nativeui`, `px-pipeline`, `siril-sys` |
| `px-cli` | `px-static` |
| `px-conventions` | `px-configuration`, `px-fs` |
| `px-fits` | `px-fs` |
| `px-index` | `px-configuration`, `px-conventions`, `px-fits` |
| `px-nativeui` | `px-fits` |
| `px-pipeline` | `px-fits`, `px-fs`, `siril-sys` |
| `px-configuration` | *(none)* |
| `px-fs` | *(none)* |
| `px-static` | *(none)* |
| `siril-sys` | *(none)* |

## Leaf Crates (no internal dependencies)

- `px-configuration` — project/workspace config types
- `px-fs` — filesystem utilities
- `px-static` — static assets / embedded resources
- `siril-sys` — FFI bindings to Siril
