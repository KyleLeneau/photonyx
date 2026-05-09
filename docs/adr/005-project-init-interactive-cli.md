# 005 — `px project init`: Interactive CLI with Framing-Aware Scaffolding

## Context

The original `px project init` command accepted a required positional `path` argument plus
optional `--name` and `--description` flags. It always produced a minimal `px_project.yaml`
with an empty `Single` framing and no `master_lights` entries — leaving the user to manually
construct the config file from scratch with no structural guidance.

Two problems motivated a rework:

1. **Discovery gap.** New users had no way to learn the shape of each framing type without
   reading source code or documentation. The generated file was too sparse to serve as an
   example.
2. **Friction for common cases.** Creating a project required knowing the exact YAML structure
   for `Single`, `SpiralMosiac`, or `GridMosiac` framing upfront. There was no guided path.

---

## Decision

### CLI shape

`px project init` now accepts all inputs as optional flags; nothing is required at the CLI level.

```
px project init [OPTIONS]

Options:
  --path <PATH>            Project directory (default: <profile_root>/PROJECTS/<name_slug>)
  --name <NAME>            Project name (prompted if omitted)
  --description <DESC>     Short description (prompted; skippable)
  --framing <TYPE>         single | spiral-mosiac | grid-mosiac (prompted if omitted)
  --stack-name <NAME>      First stack/mosaic name (prompted if omitted)
  --filter <FILTER>        Filter label, e.g. Ha, LRGB, OSC (prompted if omitted)
  --feather-pixels <F32>   Edge feather for spiral-mosiac framing (default: 0.0)
  -y, --no-interactive     Skip all prompts; use flag values and derived defaults
```

The positional `path` argument is removed. Path is now a `--path` flag; when omitted the
directory is derived as `<profile_root>/PROJECTS/<name with spaces replaced by _>`. The name
casing from user input is preserved — no forced lowercase.

### Interactive mode (default)

When `--no-interactive` is not set, `inquire` prompts guide the user through:

1. **Project name** — free text; defaults to the directory name of `--path` if provided,
   otherwise `my_project`.
2. **Description** — free text; pressing enter with an empty value stores `None`.
3. **Framing type** — select from `single`, `spiral-mosiac`, `grid-mosiac`; defaults to
   `single`.
4. **Framing-specific questions** — only the questions relevant to the chosen framing type
   are asked (see below).

For `Single` framing the user is also asked:

- **How many master light layers?** — integer, default 1. Each layer represents one
  `ProjectLinearStack` entry (e.g. one per filter channel or per session group).
- **Layer N name** — free text per layer; defaults to `light_stack` (or `light_stack_N` for
  multiple layers). If `--stack-name` was passed on the CLI it seeds the default.
- **Layer N filter** — free text per layer; blank input is accepted and stored as `None`.

For `SpiralMosiac` framing:

- **Mosaic name** — free text; defaults to `my_mosaic`.
- **Filter** — free text; blank input is accepted.

For `GridMosiac` framing:

- **Panel name** — free text; defaults to `panel_01`.
- **Filter** — free text; blank input is accepted.

### Non-interactive mode (`-y`)

All prompts are skipped. Defaults apply:

- Name derived from `--path` directory name; fails with an error if neither `--name` nor
  `--path` is provided.
- Framing defaults to `Single`.
- Stack name defaults to `light_stack`; filter defaults to `None`.
- One `master_lights` entry is generated.

This mode is intended for scripting and CI use cases.

### Scaffolded output

Rather than an empty config, `init` generates a populated example with fictitious but
structurally valid placeholder data. The purpose is to show the user the full shape of the
chosen framing type so they can edit paths and add entries rather than author YAML from scratch.

Example placeholder values used:

| Field | Placeholder |
|---|---|
| `profile` | `<profile_root>/hardware_profiles/my_camera.yaml` |
| `observations[].path` | `observations/session_2025_01_01` |
| `comments` | Short editorial note |

These paths will not exist on disk — the intent is documentation-by-example within the file
itself.

### `requires_profile` change

`px project init` previously bypassed the profile requirement (it was excluded from
`requires_profile`). It now requires a profile for two reasons:

- The default project path is derived from `profile_index.profile.root`, so a profile must
  be resolved before path defaulting can occur.
- Future phases (see below) will query the profile index to list available observations and
  hardware profiles during the interactive flow.

`px project list` remains excluded from `requires_profile` (it is not yet implemented).

---

## Consequences

- `InitProjectArgs` in `px-cli` gains six new fields (`path` changed from required positional
  to optional flag, plus `framing`, `stack_name`, `filter`, `feather_pixels`, `no_interactive`).
- `init_project` in `px` accepts a `ProfileIndex` third argument and uses
  `profile_index.profile.root` for path defaulting.
- The dispatch in `px/src/lib.rs` passes `profile_index.unwrap()` to `init_project` and
  removes `Commands::Project(_)` from the `requires_profile` exclusion — only
  `ProjectCommand::List` remains excluded.
- `inquire` (`CustomType`, `Select`, `Text`) is used for all interactive prompts.
  `inquire` was already a workspace dependency.
- Generated configs contain example paths that the user must edit before running
  `px project stack`. No validation of placeholder paths is performed at init time.

---

## Future expansion

### Phase 2 — Profile-index-aware prompts

Once `px project init` has the `ProfileIndex` in hand, the interactive flow can be extended
to offer real choices from the profile's data rather than free-text fields:

- **Hardware profile selection** — list available `px_profile.yaml` files discovered under
  `<profile_root>/hardware_profiles/` and present them as a `Select` prompt for the `profile`
  field on each `ProjectLinearStack`.
- **Observation selection** — query `ProfileIndex` for registered observations and present a
  multi-select so users can attach real observation paths at init time rather than editing the
  YAML afterwards. This replaces the `observations/session_2025_01_01` placeholder with actual
  entries.
- **Filter inference** — when observations are selected, infer available filters from their
  metadata and pre-populate the filter field accordingly.

### Phase 3 — Multi-profile projects

The current design anchors a project to the profile root it was initialized under
(`PROJECTS/` lives inside the profile directory). A future version may decouple project
storage from the profile root so that a single project can span observations from multiple
profiles (e.g. data collected with different rigs or at different sites). This was considered
during the initial design but deferred as too complex for the current phase.

Markers for this work:
- `ProjectPath::find` currently walks up from CWD looking for `px_project.yaml` with no
  profile awareness — this boundary will need to be revisited.
- The `--path` flag accepting an absolute path already provides an escape hatch for placing
  projects outside the profile root.

### Phase 4 — Grid mosaic panel builder

`GridMosiacFraming` currently scaffolds a single `GridMosiacPanel`. The schema is still
evolving; the interactive flow intentionally produces a minimal example with an editorial
comment rather than prompting for full panel configuration. When the grid mosaic schema
stabilises, the `init` flow should be extended to prompt for panel count, per-panel names,
and per-panel filter/stack assignments — following the same per-layer loop pattern introduced
for `Single` framing in this ADR.
