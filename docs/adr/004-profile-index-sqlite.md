# 004 — Profile Index: SQLite via `.px/` Hidden Directory

## Context

Photonyx needs a way to store and query metadata about observations and calibration frames
(masters) within a profile. Specifically:

- Each observation needs to be linkable to a master dark and one or more master flats (by filter)
  so that the calibration pipeline knows which frames to use without requiring the user to specify
  them explicitly on every command.
- Master frames carry metadata (temperature, gain, offset, binning, filter, exposure) that should
  be queryable for best-match selection and statistics.
- Observations carry metadata (target, date, filter, exposure, temperature, gain) that should
  support cross-session queries such as total integration time per filter.
- File paths for both observations and masters may eventually point to remote locations (e.g.
  `s3://` URIs) rather than local disk paths, so the index must not assume local presence.

Two alternatives were considered:

- **Per-session YAML** (`px_session.yaml` next to each observation): human-editable and consistent
  with the existing `px_profile.yaml` / `px_project.yaml` convention, but cannot be queried
  across sessions, cannot support remote URIs cleanly, and becomes verbose for multi-filter setups.
- **SQLite database at profile root**: enables cross-session queries, statistics, schema
  migrations, and remote URI storage. Follows the same hidden-directory pattern as `.git/`.

---

## Decision

Each profile root contains a `.px/` hidden directory. This directory is managed exclusively by
`px` — users do not edit it directly.

```
PX_Radian75/
  .px/
    index.db        ← SQLite profile index (managed by px)
  px_profile.yaml   ← human-editable profile settings (unchanged)
  BIAS/
  DARK/
  FLAT/
  LIGHT/
  PROJECTS/
```

### Library: `sqlx`

Use `sqlx` (async, compile-time query verification) rather than `rusqlite`. Rationale:

- Compile-time query checking catches schema/query mismatches at build time.
- Async support aligns with the existing `tokio`-based pipeline and future TUI/server work.
- Built-in migration support via `sqlx::migrate!()` removes the need for a hand-rolled migration
  runner as the schema evolves.

### Crate placement: `px-index` (new crate)

`ProfileIndex` and the SQLite layer live in a new `px-index` crate rather than `px-configuration`.
Rationale:

- `sqlx` is a heavy dependency with compile-time query checking and an embedded `migrations/`
  directory. Isolating it keeps `px-configuration` a lightweight YAML-only crate.
- The separation reflects a real boundary: `px-configuration` owns human-editable state,
  `px-index` owns machine-managed state.
- Anything that needs only profile settings (filters, description) does not pull in `sqlx`.

The crate dependency graph remains a clean DAG:

```
px-conventions      (ProfilePath — no config or DB deps)
px-configuration    (ProfileConfig YAML — depends on px-conventions)
px-index            (ProfileIndex + SQLite — depends on px-conventions + px-configuration)
px                  (CLI — depends on px-index)
```

`px-index` owns the `migrations/` directory and the `ProfileIndex` struct. All other crates
that previously depended on `px-configuration` for profile access now depend on `px-index`
instead when they need the full handle.

### `ProfileIndex`: the primary profile handle

`ProfileIndex` is the struct that owns both the `ProfilePath` and the open `SqlitePool`. It
replaces `ProfilePath` as the value passed to commands that require a profile.

```rust
pub struct ProfileIndex {
    pub profile: ProfilePath,   // path conventions still accessible
    pub config: ProfileConfig,  // loaded eagerly; replaces manual load_config() calls
    pool: SqlitePool,           // private; accessed via methods
}

impl ProfileIndex {
    pub async fn open(profile: ProfilePath) -> Result<Self, ProfileIndexError>;
    pub async fn save_config(&self) -> Result<(), ProfileIndexError>;
}
```

`ProfileIndex::open` is the single entry point: it takes ownership of `ProfilePath`, loads
`ProfileConfig` from the YAML, creates `.px/` if absent, opens the `SqlitePool`, and runs all
pending migrations before returning. Migrations run on every `open` call — `sqlx::migrate!()` is
idempotent and fast when the schema is current, so there is no need to guard the call.

Commands that need path conventions access them via `index.profile.flat`, `index.profile.light`,
etc. Commands that need profile settings use `index.config.filters`, etc. Commands that need the
DB call methods on `index` directly. No command ever holds a raw `ProfilePath` or calls
`load_config()` directly — `ProfileIndex` is the single profile handle.

**Call site in `run()`:**

```rust
// ProfilePath resolved first (unchanged); then wrapped into ProfileIndex
let profile_index = ProfileIndex::open(ProfilePath::find(cli.top_level.global_args.profile)?).await?;

// Passed to commands (replaces profile_path parameter)
commands::create_master_flat(args, printer, profile_index).await
```

Commands that do not require a profile (e.g. `px self version`) are unaffected — `ProfileIndex`
is only opened after the `requires_profile()` guard passes.

### `ProfileIndex` API

Methods on `ProfileIndex` cover the four interaction areas:

```rust
impl ProfileIndex {
    // Masters
    pub async fn register_master(&self, master: impl Into<MasterRecord>) -> Result<Uuid>;
    pub async fn list_masters(&self, kind: Option<MasterKind>) -> Result<Vec<MasterRecord>>;
    pub async fn find_best_dark(&self, criteria: &MatchCriteria) -> Result<Option<MasterRecord>>;
    pub async fn find_best_flat(&self, filter: &str, criteria: &MatchCriteria) -> Result<Option<MasterRecord>>;

    // Observations
    pub async fn upsert_observation(&self, obs: &ObservationRecord) -> Result<Uuid>;
    pub async fn list_observations(&self, filter: Option<&str>) -> Result<Vec<ObservationRecord>>;

    // Calibration links
    pub async fn link_calibration(&self, obs_id: Uuid, master_id: Uuid, kind: MasterKind) -> Result<()>;
    pub async fn get_links(&self, obs_id: Uuid) -> Result<CalibrationLinks>;
}
```

`register_master` performs the upsert by `path`: if a row with the same path already exists,
its metadata is updated in place and the existing `id` is returned. This prevents duplicates
when a master command is re-run on the same output file.

### Migration infrastructure

Migrations live in `crates/px-configuration/migrations/` as numbered SQL files following the
`sqlx` convention:

```
crates/px-configuration/
  migrations/
    0001_initial_schema.sql
    0002_add_frame_count_index.sql   ← future example
  src/
    index.rs      ← ProfileIndex impl
    profile.rs
    project.rs
```

The `sqlx::migrate!("migrations/")` macro embeds migration files at compile time. Each migration
file is a plain SQL `CREATE TABLE` / `ALTER TABLE` statement. The `_sqlx_migrations` table
(managed by sqlx) tracks which migrations have been applied, making `open` safe to call on both
new and existing databases.

Adding a new migration requires only a new numbered `.sql` file — no Rust changes to the
migration runner.

### Schema

```sql
CREATE TABLE calibration_set (
    id          TEXT PRIMARY KEY,   -- UUID v4
    kind        TEXT NOT NULL,      -- 'bias' | 'dark' | 'flat'
    source_path TEXT NOT NULL,      -- path to the raw input frames (local or remote URI)
    master_path TEXT NOT NULL,      -- path to the stacked master output (local or remote URI)
    date        TEXT NOT NULL,      -- ISO 8601 date (YYYY-MM-DD)
    frame_count INTEGER,
    temperature REAL,
    gain        INTEGER,
    offset      INTEGER,
    binning     TEXT,               -- e.g. "1x1", "2x2"
    exposure    REAL,               -- seconds; NULL for bias/flat
    filter      TEXT,               -- NULL for bias/dark
    created_at  TEXT NOT NULL       -- ISO 8601 datetime
);

CREATE TABLE observation_set (
    id              TEXT PRIMARY KEY,   -- UUID v4
    target_name     TEXT NOT NULL,
    date            TEXT NOT NULL,      -- ISO 8601 date (YYYY-MM-DD)
    filter          TEXT NOT NULL,
    exposure        REAL NOT NULL,      -- seconds per frame
    frame_count     INTEGER,
    temperature     REAL,
    gain            INTEGER,
    offset          INTEGER,
    binning         TEXT,
    target_ra       REAL,               -- right ascension in decimal degrees; NULL if unknown
    target_dec      REAL,               -- declination in decimal degrees; NULL if unknown
    raw_path        TEXT NOT NULL,      -- path to raw light frames (local or remote URI)
    calibrated_path TEXT,               -- path to calibrated frames; NULL until calibrated
    created_at      TEXT NOT NULL
);

CREATE TABLE calibration_link (
    observation_id  TEXT NOT NULL REFERENCES observation_set(id),
    master_id       TEXT NOT NULL REFERENCES calibration_set(id),
    kind            TEXT NOT NULL,  -- 'dark' | 'flat' | 'bias'
    PRIMARY KEY (observation_id, kind)
);
```

### Auto-population

Master commands (`px masters create-bias`, `create-dark`, `create-flat`) automatically insert
a row into `masters` upon successful completion. The pipeline already returns `MasterBias`,
`MasterDark`, and `MasterFlat` structs containing all required metadata — registration is a
post-pipeline step in those commands.

Duplicate prevention: before inserting, query for an existing row with the same `path`. If
found, update metadata in place rather than inserting a new row.

Observation scanning is done via an explicit `px obs scan` command (and optionally `px obs add`)
that walks the `LIGHT/` tree, parses `ObservationPath` entries, reads FITS header metadata, and
upserts into `observations`. This is intentionally separate from master registration — scanning
a LIGHT tree is a heavier operation and should be user-initiated.

### Calibration linking

Links are set explicitly via `px obs link` or automatically via `px obs auto-link`:

```
px obs link --dark <master-id> <obs-path>
px obs link --flat Ha=<master-id>,OIII=<master-id> <obs-path>
px obs auto-link          # match by temperature ± tolerance, gain, filter, binning
```

`auto-link` uses the metadata on both sides to score candidates and pick the best match per
observation. It does not overwrite existing explicit links unless `--force` is passed.

### Path to global index

The per-profile scope is intentional for isolation and portability. A future global index at
`~/.px/index.db` can aggregate across profiles via SQLite's `ATTACH DATABASE` mechanism or a
periodic sync command (`px index sync`) without requiring any schema changes to the per-profile
database.

---

## Consequences

- A new `px-index` crate is added with `sqlx` (`sqlite` + `runtime-tokio` features) as its
  primary dependency.
- A `migrations/` directory lives under `crates/px-index/` for numbered `.sql` files.
- `px-configuration` is unchanged — it remains a YAML-only crate with no `sqlx` dependency.
- `ProfileIndex::open` creates `.px/`, runs all pending migrations, and returns the handle.
  This replaces any need for a separate migration step in `px profile init`.
- `ProfilePath` is no longer passed directly to commands that require a profile — `ProfileIndex`
  is passed instead. Path conventions remain accessible via `index.profile`.
- The `calibration_master: Vec<CalibrationMaster>` field is removed from `ProfileConfig` and
  `px_profile.yaml`. The `masters` table in the index is the sole record of calibration frames.
  Existing YAML files with that field will have it silently ignored on load (serde default).
- `px masters create-*` commands gain a post-pipeline `index.register_master(...)` call.
- `px_session.yaml` is not used; the index is the single source of truth for calibration links.

---

## Entity summary

| Table | Key fields |
|---|---|
| `calibration_set` | id, kind, source\_path, master\_path, date, frame\_count, temp, gain, offset, binning, exposure, filter |
| `observation_set` | id, target\_name, target\_ra, target\_dec, date, filter, exposure, frame\_count, temp, gain, raw\_path, calibrated\_path |
| `calibration_link` | observation\_id, master\_id, kind |
