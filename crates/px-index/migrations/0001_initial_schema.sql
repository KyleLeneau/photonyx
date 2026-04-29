CREATE TABLE IF NOT EXISTS calibration_set (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL CHECK(kind IN ('bias', 'dark', 'flat')),
    source_path TEXT NOT NULL,
    master_path TEXT NOT NULL UNIQUE,
    date        TEXT NOT NULL,
    frame_count INTEGER,
    temperature REAL,
    gain        INTEGER,
    offset      INTEGER,
    binning     TEXT,
    exposure    REAL,
    filter      TEXT,
    created_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS observation_set (
    id              TEXT PRIMARY KEY,
    target_name     TEXT NOT NULL,
    target_ra       REAL,
    target_dec      REAL,
    date            TEXT NOT NULL,
    filter          TEXT NOT NULL,
    exposure        REAL NOT NULL,
    frame_count     INTEGER,
    temperature     REAL,
    gain            INTEGER,
    offset          INTEGER,
    binning         TEXT,
    raw_path        TEXT NOT NULL UNIQUE,
    calibrated_path TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS calibration_link (
    observation_id  TEXT NOT NULL REFERENCES observation_set(id),
    master_id       TEXT NOT NULL REFERENCES calibration_set(id),
    kind            TEXT NOT NULL CHECK(kind IN ('bias', 'dark', 'flat')),
    PRIMARY KEY (observation_id, kind)
);
