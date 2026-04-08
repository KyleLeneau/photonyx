# 003 — Core Data Model Terminology

## Context

Photonyx needs a set of first-class CLI concepts that map cleanly onto how astrophotographers
actually work. The primary challenge is naming: the domain has overlapping vocabulary (target,
session, sequence, project) and different tools use the same words to mean different things.

Several terms were considered and rejected:

- **target** — too broad; a target (e.g. M42) can span many nights and many projects.
- **session** — commonly means an entire night of imaging in tools like NINA and APT, which may
  cover multiple targets. Ambiguous scope makes it a poor fit for a target+night concept.
- **sequence** — already carries meaning in NINA/SGP (an automation sequence); risks confusion
  with Siril's own sequencing concepts.
- **image-set** — descriptive but generic and not astronomy-native.

---

## Decision

### `observation` (alias: `obs`)

An **observation** is the atomic unit of captured data: a set of light frames taken on a
specific target during a specific night, with a specific hardware configuration.

Key properties:
- Scoped to **one target + one night** — the closest practical unit to "when the data was
  captured."
- Carries a **hardware profile** (camera, telescope, gain, sensor temperature, filter, etc.)
  which drives calibration frame matching and pipeline decisions downstream.
- For mosaic projects, carries an optional **panel identifier** (e.g. `1-1`, `1-2`) consistent
  with common folder naming conventions like `M2_4P/RAW_L_1-1`.
- Has a **contributor** field, enabling multi-user projects without changing the data shape.

The alias `obs` is the short form used in CLI commands (e.g. `px obs list`, `px obs add`).

The term "observation" is used in professional astronomy to mean exactly this: a defined set of
exposures on a target within a time window. It is unambiguous, astronomy-native, and composes
naturally with the rest of the model.

---

### `project`

A **project** is the assembly of one or more observations and processing settings used to
produce a final linear (and eventually processed) image of a target.

Key properties:
- Has a **type**: `single` (one pointing) or `mosaic` (multiple panels). The type is explicit
  rather than derived so that it can drive workflow selection and validate observations being
  added.
- Aggregates observations from any number of contributors — the model is identical for solo and
  multi-user use; only the access layer (see `group` below) differs.
- Observations attach to a project directly. For mosaics, the panel identifier on each
  observation provides the grouping — no intermediate `panel` entity is needed unless mosaic
  tooling grows complex enough to warrant it.

---

### `group` (future)

A **group** is an access and membership concept only. It answers "who can see and contribute to
this project." The data model (`observation` → `project`) does not change between solo and group
use.

Key properties:
- Has named members.
- Projects optionally belong to a group; a project without a group is private to its owner.
- Members of a group can add observations to shared projects.

This keeps two concerns cleanly separated:

| Concern | Entity |
|---|---|
| Data shape and pipeline | `observation`, `project` |
| Access and collaboration | `group`, members, permissions |

The group layer can be built independently and bolted on without restructuring the core model.

---

## Entity summary

| Entity | Key fields |
|---|---|
| `observation` / `obs` | id, project\_id, target, date, panel?, hardware\_profile, contributor, filter, frames |
| `project` | id, name, target, type (single \| mosaic), owner, group? |
| `group` | id, name, members |

---

## Pipeline implication

Because the hardware profile lives on the `observation`, the processing pipeline can
automatically determine what intermediate assets to produce. For a group project with
observations from contributors using different hardware, the pipeline forks per distinct
`(contributor, hardware, filter, panel?)` combination to produce per-group calibrated stacks,
then merges them at the linear stage. No special-casing is required — this falls out of the
data model naturally.
