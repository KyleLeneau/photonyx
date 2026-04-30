use std::path::PathBuf;

use chrono::Utc;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use thiserror::Error;
use uuid::Uuid;

use px_configuration::{ProfileConfig, ProfileConfigError};
use px_conventions::profile::ProfilePath;
use px_fits::{CalibratedLight, MasterBias, MasterDark, MasterFlat};

#[derive(Debug, Error)]
pub enum ProfileIndexError {
    #[error("profile config error: {0}")]
    Config(#[from] ProfileConfigError),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum MasterKind {
    Bias,
    Dark,
    Flat,
}

impl MasterKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MasterKind::Bias => "bias",
            MasterKind::Dark => "dark",
            MasterKind::Flat => "flat",
        }
    }
}

/// A record ready to be inserted into `observation_set`.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct ObservationRecord {
    pub id: String,
    pub target_name: String,
    pub target_ra: Option<f64>,
    pub target_dec: Option<f64>,
    pub date: String,
    pub filter: String,
    pub exposure: f64,
    pub frame_count: Option<i64>,
    pub temperature: Option<f64>,
    pub gain: Option<i64>,
    pub offset: Option<i64>,
    pub binning: Option<String>,
    pub raw_path: String,
    pub calibrated_path: Option<String>,
    pub site_lat: Option<f64>,
    pub site_long: Option<f64>,
}

impl From<CalibratedLight> for ObservationRecord {
    fn from(l: CalibratedLight) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            target_name: l.target_name,
            target_ra: l.target_ra,
            target_dec: l.target_dec,
            date: l.capture_date.format("%Y-%m-%d").to_string(),
            filter: l.filter,
            exposure: l.exposure,
            frame_count: Some(l.frame_count as i64),
            temperature: Some(l.temperature),
            gain: Some(l.gain),
            offset: Some(l.offset),
            binning: Some(l.binning.to_string()),
            raw_path: l.source.to_string_lossy().into_owned(),
            calibrated_path: Some(l.path.to_string_lossy().into_owned()),
            site_lat: l.site_lat,
            site_long: l.site_long,
        }
    }
}

/// A record for `calibration_set` — used for both inserts and query results.
#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct CalibrationRecord {
    pub id: String,
    pub kind: MasterKind,
    pub source_path: String,
    pub master_path: String,
    pub date: String,
    pub frame_count: Option<i64>,
    pub temperature: Option<f64>,
    pub gain: Option<i64>,
    pub offset: Option<i64>,
    pub binning: Option<String>,
    pub exposure: Option<f64>,
    pub filter: Option<String>,
}

impl From<MasterBias> for CalibrationRecord {
    fn from(m: MasterBias) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind: MasterKind::Bias,
            date: m.capture_date.format("%Y-%m-%d").to_string(),
            source_path: m.source.to_string_lossy().into_owned(),
            master_path: m.path.to_string_lossy().into_owned(),
            frame_count: Some(m.frame_count as i64),
            temperature: Some(m.temperature),
            gain: Some(m.gain),
            offset: Some(m.offset),
            binning: Some(m.binning.to_string()),
            exposure: None,
            filter: None,
        }
    }
}

impl From<MasterDark> for CalibrationRecord {
    fn from(m: MasterDark) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind: MasterKind::Dark,
            date: m.capture_date.format("%Y-%m-%d").to_string(),
            source_path: m.source.to_string_lossy().into_owned(),
            master_path: m.path.to_string_lossy().into_owned(),
            frame_count: Some(m.frame_count as i64),
            temperature: Some(m.temperature),
            gain: Some(m.gain),
            offset: Some(m.offset),
            binning: Some(m.binning.to_string()),
            exposure: Some(m.exposure),
            filter: None,
        }
    }
}

impl From<MasterFlat> for CalibrationRecord {
    fn from(m: MasterFlat) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind: MasterKind::Flat,
            date: m.capture_date.format("%Y-%m-%d").to_string(),
            source_path: m.source.to_string_lossy().into_owned(),
            master_path: m.path.to_string_lossy().into_owned(),
            frame_count: Some(m.frame_count as i64),
            temperature: Some(m.temperature),
            gain: Some(m.gain),
            offset: Some(m.offset),
            binning: Some(m.binning.to_string()),
            exposure: None,
            filter: Some(m.filter),
        }
    }
}

/// Criteria for best-match lookup against `calibration_set`.
#[derive(Debug, Default)]
pub struct MatchCriteria {
    pub temperature: Option<f64>,
    /// Allowed deviation in degrees C when matching temperature.
    pub temperature_tolerance: Option<f64>,
    pub gain: Option<i64>,
    pub offset: Option<i64>,
    pub binning: Option<String>,
}

/// The primary profile handle: paths, config, and the SQLite index.
pub struct ProfileIndex {
    pub profile: ProfilePath,
    pub config: ProfileConfig,
    pool: SqlitePool,
}

impl ProfileIndex {
    /// Open (or create) the profile index at `profile.root/.px/index.db`.
    /// Runs all pending migrations before returning.
    pub async fn open(profile: ProfilePath) -> Result<Self, ProfileIndexError> {
        let config = profile.load_config()?;

        let px_dir = profile.root.join(".px");
        std::fs::create_dir_all(&px_dir)?;

        let db_path = px_dir.join("index.db");
        let options = SqliteConnectOptions::new()
            .filename(&db_path)
            .create_if_missing(true);

        let pool = SqlitePool::connect_with(options).await?;

        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self {
            profile,
            config,
            pool,
        })
    }

    /// Convenience: find the profile from an optional directory then open the index.
    pub async fn find_and_open(directory: Option<PathBuf>) -> Result<Self, ProfileIndexError> {
        let profile = ProfilePath::find(directory)?;
        Self::open(profile).await
    }

    /// Persist any changes made to `self.config` back to `px_profile.yaml`.
    pub fn save_config(&self) -> Result<(), ProfileIndexError> {
        self.profile.save_config(&self.config)?;
        Ok(())
    }

    // ── calibration_set ───────────────────────────────────────────────────────

    /// Insert or update a calibration master in the index. Returns the row id.
    /// Upserts by `master_path`: if the path already exists the metadata is
    /// refreshed in place and the existing id is returned.
    pub async fn register_master(
        &self,
        record: impl Into<CalibrationRecord>,
    ) -> Result<String, ProfileIndexError> {
        let r = record.into();

        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM calibration_set WHERE master_path = ?")
                .bind(&r.master_path)
                .fetch_optional(&self.pool)
                .await?;

        if let Some((id,)) = existing {
            sqlx::query(
                "UPDATE calibration_set
                 SET kind=?, source_path=?, date=?, frame_count=?, temperature=?,
                     gain=?, offset=?, binning=?, exposure=?, filter=?
                 WHERE id=?",
            )
            .bind(r.kind.as_str())
            .bind(&r.source_path)
            .bind(&r.date)
            .bind(r.frame_count)
            .bind(r.temperature)
            .bind(r.gain)
            .bind(r.offset)
            .bind(&r.binning)
            .bind(r.exposure)
            .bind(&r.filter)
            .bind(&id)
            .execute(&self.pool)
            .await?;

            return Ok(id);
        }

        sqlx::query(
            "INSERT INTO calibration_set
             (id, kind, source_path, master_path, date, frame_count, temperature,
              gain, offset, binning, exposure, filter, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&r.id)
        .bind(r.kind.as_str())
        .bind(&r.source_path)
        .bind(&r.master_path)
        .bind(&r.date)
        .bind(r.frame_count)
        .bind(r.temperature)
        .bind(r.gain)
        .bind(r.offset)
        .bind(&r.binning)
        .bind(r.exposure)
        .bind(&r.filter)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(r.id)
    }

    /// Insert or update a calibrated observation in the index. Returns the row id.
    /// Upserts by `raw_path`: if the path already exists the metadata is refreshed in place.
    pub async fn register_observation(
        &self,
        record: impl Into<ObservationRecord>,
    ) -> Result<String, ProfileIndexError> {
        let r = record.into();

        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM observation_set WHERE raw_path = ?")
                .bind(&r.raw_path)
                .fetch_optional(&self.pool)
                .await?;

        if let Some((id,)) = existing {
            sqlx::query(
                "UPDATE observation_set
                 SET target_name=?, target_ra=?, target_dec=?, date=?, filter=?, exposure=?,
                     frame_count=?, temperature=?, gain=?, offset=?, binning=?, calibrated_path=?,
                     site_lat=?, site_long=?
                 WHERE id=?",
            )
            .bind(&r.target_name)
            .bind(r.target_ra)
            .bind(r.target_dec)
            .bind(&r.date)
            .bind(&r.filter)
            .bind(r.exposure)
            .bind(r.frame_count)
            .bind(r.temperature)
            .bind(r.gain)
            .bind(r.offset)
            .bind(&r.binning)
            .bind(&r.calibrated_path)
            .bind(r.site_lat)
            .bind(r.site_long)
            .bind(&id)
            .execute(&self.pool)
            .await?;

            return Ok(id);
        }

        sqlx::query(
            "INSERT INTO observation_set
             (id, target_name, target_ra, target_dec, date, filter, exposure,
              frame_count, temperature, gain, offset, binning, raw_path, calibrated_path,
              site_lat, site_long, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&r.id)
        .bind(&r.target_name)
        .bind(r.target_ra)
        .bind(r.target_dec)
        .bind(&r.date)
        .bind(&r.filter)
        .bind(r.exposure)
        .bind(r.frame_count)
        .bind(r.temperature)
        .bind(r.gain)
        .bind(r.offset)
        .bind(&r.binning)
        .bind(&r.raw_path)
        .bind(&r.calibrated_path)
        .bind(r.site_lat)
        .bind(r.site_long)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(r.id)
    }

    /// List all calibration masters, optionally filtered by kind.
    pub async fn list_masters(
        &self,
        kind: Option<MasterKind>,
    ) -> Result<Vec<CalibrationRecord>, ProfileIndexError> {
        let rows: Vec<CalibrationRecord> = match kind {
            Some(k) => {
                sqlx::query_as::<_, CalibrationRecord>(
                    "SELECT id, kind, source_path, master_path, date, frame_count,
                            temperature, gain, offset, binning, exposure, filter
                     FROM calibration_set WHERE kind = ?",
                )
                .bind(k.as_str())
                .fetch_all(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, CalibrationRecord>(
                    "SELECT id, kind, source_path, master_path, date, frame_count,
                            temperature, gain, offset, binning, exposure, filter
                     FROM calibration_set ORDER BY date DESC",
                )
                .fetch_all(&self.pool)
                .await?
            }
        };
        Ok(rows)
    }

    /// Find the best-matching dark master for the given criteria.
    /// Matches on gain, offset, binning exactly (when provided), then picks
    /// the closest temperature within the optional tolerance.
    pub async fn find_best_dark(
        &self,
        exposure: f64,
        criteria: &MatchCriteria,
    ) -> Result<Option<CalibrationRecord>, ProfileIndexError> {
        let tolerance = criteria.temperature_tolerance.unwrap_or(2.0);

        let rows: Vec<CalibrationRecord> = sqlx::query_as::<_, CalibrationRecord>(
            "SELECT id, kind, source_path, master_path, date, frame_count,
                    temperature, gain, offset, binning, exposure, filter
             FROM calibration_set
             WHERE kind = 'dark'
               AND (? IS NULL OR gain = ?)
               AND (? IS NULL OR offset = ?)
               AND (? IS NULL OR binning = ?)
               AND ABS(COALESCE(exposure, 0) - ?) <= 0.5
             ORDER BY ABS(COALESCE(temperature, 0) - COALESCE(?, 0))",
        )
        .bind(criteria.gain)
        .bind(criteria.gain)
        .bind(criteria.offset)
        .bind(criteria.offset)
        .bind(&criteria.binning)
        .bind(&criteria.binning)
        .bind(exposure)
        .bind(criteria.temperature)
        .bind(criteria.temperature)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .find(|r| match (r.temperature, criteria.temperature) {
                (Some(t), Some(target)) => (t - target).abs() <= tolerance,
                _ => true,
            }))
    }

    /// Find the best-matching flat master for the given filter and criteria.
    pub async fn find_best_flat(
        &self,
        filter: &str,
        criteria: &MatchCriteria,
    ) -> Result<Option<CalibrationRecord>, ProfileIndexError> {
        let tolerance = criteria.temperature_tolerance.unwrap_or(2.0);

        let rows: Vec<CalibrationRecord> = sqlx::query_as::<_, CalibrationRecord>(
            "SELECT id, kind, source_path, master_path, date, frame_count,
                    temperature, gain, offset, binning, exposure, filter
             FROM calibration_set
             WHERE kind = 'flat'
               AND filter = ?
               AND (? IS NULL OR gain = ?)
               AND (? IS NULL OR offset = ?)
               AND (? IS NULL OR binning = ?)
             ORDER BY ABS(COALESCE(temperature, 0) - COALESCE(?, 0))",
        )
        .bind(filter)
        .bind(criteria.gain)
        .bind(criteria.gain)
        .bind(criteria.offset)
        .bind(criteria.offset)
        .bind(&criteria.binning)
        .bind(&criteria.binning)
        .bind(criteria.temperature)
        .bind(criteria.temperature)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .find(|r| match (r.temperature, criteria.temperature) {
                (Some(t), Some(target)) => (t - target).abs() <= tolerance,
                _ => true,
            }))
    }
}
