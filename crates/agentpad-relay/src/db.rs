use anyhow::Result;
use rusqlite::{params, Connection, Error as SqlError};
use std::sync::Mutex;

use crate::models::{Op, Snapshot};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS ops (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                workspace_id TEXT NOT NULL,
                op_id TEXT NOT NULL,
                op_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                client_id TEXT,
                UNIQUE(workspace_id, op_id)
            );

            CREATE INDEX IF NOT EXISTS idx_ops_workspace ON ops(workspace_id, id);

            CREATE TABLE IF NOT EXISTS snapshots (
                workspace_id TEXT PRIMARY KEY,
                data TEXT NOT NULL,
                last_op_id TEXT,
                updated_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn push_op(&self, workspace_id: &str, op: &Op) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR IGNORE INTO ops (workspace_id, op_id, op_type, payload, timestamp, client_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                workspace_id,
                op.id,
                op.op_type,
                op.payload,
                op.timestamp,
                op.client_id,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_ops(&self, workspace_id: &str, after_id: Option<i64>) -> Result<Vec<Op>> {
        let conn = self.conn.lock().unwrap();
        let after_id = after_id.unwrap_or(0);

        let mut stmt = conn.prepare(
            r#"
            SELECT id, op_id, op_type, payload, timestamp, client_id
            FROM ops
            WHERE workspace_id = ?1 AND id > ?2
            ORDER BY id ASC
            "#,
        )?;

        let ops = stmt
            .query_map(params![workspace_id, after_id], |row| {
                Ok(Op {
                    db_id: Some(row.get(0)?),
                    id: row.get(1)?,
                    op_type: row.get(2)?,
                    payload: row.get(3)?,
                    timestamp: row.get(4)?,
                    client_id: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ops)
    }

    pub fn get_snapshot(&self, workspace_id: &str) -> Result<Option<Snapshot>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT data, last_op_id, updated_at
            FROM snapshots
            WHERE workspace_id = ?1
            "#,
        )?;

        match stmt.query_row(params![workspace_id], |row| {
            Ok(Snapshot {
                workspace_id: workspace_id.to_string(),
                data: row.get(0)?,
                last_op_id: row.get(1)?,
                updated_at: row.get(2)?,
            })
        }) {
            Ok(snapshot) => Ok(Some(snapshot)),
            Err(SqlError::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save_snapshot(&self, snapshot: &Snapshot) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT OR REPLACE INTO snapshots (workspace_id, data, last_op_id, updated_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                snapshot.workspace_id,
                snapshot.data,
                snapshot.last_op_id,
                snapshot.updated_at,
            ],
        )?;
        Ok(())
    }
}
