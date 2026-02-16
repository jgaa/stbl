use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::CacheError;

#[derive(Debug, Clone)]
pub struct CachedTask {
    pub task_id: String,
    pub inputs_fingerprint: [u8; 32],
    pub outputs: Vec<String>,
}

pub trait CacheStore {
    fn get(&mut self, task_id: &str) -> Result<Option<CachedTask>, CacheError>;
    fn put(
        &mut self,
        task_id: &str,
        inputs_fingerprint: [u8; 32],
        outputs: &[String],
    ) -> Result<(), CacheError>;
}

pub struct SqliteCacheStore {
    conn: Connection,
}

const SCHEMA_VERSION: i64 = 1;

impl SqliteCacheStore {
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta(\
                key TEXT PRIMARY KEY,\
                value INTEGER NOT NULL\
            );",
        )?;
        let version: Option<i64> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(version) = version {
            if version != SCHEMA_VERSION {
                eprintln!(
                    "warning: cache schema version mismatch (found {}, expected {}), recreating cache",
                    version, SCHEMA_VERSION
                );
                conn.execute_batch(
                    "DROP TABLE IF EXISTS outputs;\
                    DROP TABLE IF EXISTS tasks;\
                    DROP TABLE IF EXISTS meta;",
                )?;
            }
        }
        create_schema(&conn)?;
        Ok(Self { conn })
    }
}

fn create_schema(conn: &Connection) -> Result<(), CacheError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta(\
            key TEXT PRIMARY KEY,\
            value INTEGER NOT NULL\
        );\
        CREATE TABLE IF NOT EXISTS tasks(\
            task_id TEXT PRIMARY KEY,\
            inputs_hash BLOB NOT NULL,\
            updated_utc INTEGER NULL\
        );\
        CREATE TABLE IF NOT EXISTS outputs(\
            task_id TEXT NOT NULL,\
            path TEXT NOT NULL,\
            PRIMARY KEY(task_id, path),\
            FOREIGN KEY(task_id) REFERENCES tasks(task_id) ON DELETE CASCADE\
        );",
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        params![SCHEMA_VERSION],
    )?;
    Ok(())
}

impl CacheStore for SqliteCacheStore {
    fn get(&mut self, task_id: &str) -> Result<Option<CachedTask>, CacheError> {
        let mut stmt = self
            .conn
            .prepare("SELECT inputs_hash FROM tasks WHERE task_id = ?1")?;
        let row: Option<Vec<u8>> = stmt
            .query_row(params![task_id], |row| row.get(0))
            .optional()?;
        let Some(blob) = row else {
            return Ok(None);
        };
        if blob.len() != 32 {
            return Err(CacheError::InvalidFingerprintLength(blob.len()));
        }
        let mut inputs = [0u8; 32];
        inputs.copy_from_slice(&blob);
        let mut outputs_stmt = self
            .conn
            .prepare("SELECT path FROM outputs WHERE task_id = ?1 ORDER BY path")?;
        let outputs_iter = outputs_stmt.query_map(params![task_id], |row| row.get(0))?;
        let mut outputs = Vec::new();
        for output in outputs_iter {
            outputs.push(output?);
        }
        Ok(Some(CachedTask {
            task_id: task_id.to_string(),
            inputs_fingerprint: inputs,
            outputs,
        }))
    }

    fn put(
        &mut self,
        task_id: &str,
        inputs_fingerprint: [u8; 32],
        outputs: &[String],
    ) -> Result<(), CacheError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO tasks (task_id, inputs_hash, updated_utc)\n\
                VALUES (?1, ?2, ?3)\n\
                ON CONFLICT(task_id) DO UPDATE SET\n\
                    inputs_hash = excluded.inputs_hash,\n\
                    updated_utc = excluded.updated_utc",
            params![task_id, inputs_fingerprint.to_vec(), now],
        )?;
        tx.execute("DELETE FROM outputs WHERE task_id = ?1", params![task_id])?;
        if !outputs.is_empty() {
            let mut stmt = tx.prepare("INSERT INTO outputs (task_id, path) VALUES (?1, ?2)")?;
            for output in outputs {
                stmt.execute(params![task_id, output])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
