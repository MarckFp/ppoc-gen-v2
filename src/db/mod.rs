// Native (sqlite) path
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
pub mod native {
    use once_cell::sync::Lazy;
    use rusqlite::{Connection, Result};
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};
    #[cfg(feature = "encryption")] use rand::RngCore;
    #[cfg(feature = "encryption")] use zeroize::Zeroize;
    #[path = "dao.rs"]
    pub mod dao;

    pub static DB: Lazy<Mutex<Connection>> = Lazy::new(|| {
        let path = db_file_path();
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        let conn = Connection::open(path).expect("open sqlite db");
        let _ = conn.execute("PRAGMA foreign_keys = ON;", []);
        #[cfg(feature = "encryption")] {
            let mut tmp = conn;
            apply_key(&mut tmp).expect("apply encryption key");
            let conn = tmp;
        }
        apply_migrations(&conn).expect("apply migrations");
        let today = chrono::Local::now().date_naive();
        let _ = conn.execute("DELETE FROM Absences WHERE end_date < ?1", [today.to_string()]);
        Mutex::new(conn)
    });

    pub fn connection() -> MutexGuard<'static, Connection> { DB.lock().unwrap() }

    fn db_file_path() -> PathBuf {
        let mut base = dirs_next::data_local_dir().unwrap_or(std::env::current_dir().unwrap());
        base.push("dx_app");
        base.push("data.db");
        base
    }

    fn apply_migrations(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);",
            [],
        )?;
        let migrations: &[(&str, &str)] = &[("0001_init", super::MIGRATION_0001_INIT)];
        for (name, sql) in migrations {
            let already: Option<i64> = conn
                .query_row("SELECT id FROM _migrations WHERE name = ?1", [name], |row| row.get(0))
                .optional()?;
            if already.is_none() {
                conn.execute_batch(sql)?;
                conn.execute("INSERT INTO _migrations (name) VALUES (?1)", [name])?;
            }
        }
        // Idempotent patch: ensure Configuration has name_order column
        let mut stmt = conn.prepare("PRAGMA table_info(Configuration)")?;
        let mut has_name_order = false;
        let mut has_week_start = false;
        let mut has_language = false;
        let mut has_date_format = false;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let col_name: String = row.get(1)?; // 1 = name
            if col_name == "name_order" { has_name_order = true; }
            if col_name == "week_start" { has_week_start = true; }
            if col_name == "language" { has_language = true; }
            if col_name == "date_format" { has_date_format = true; }
        }
        if !has_name_order {
            let _ = conn.execute("ALTER TABLE Configuration ADD COLUMN name_order TEXT NOT NULL DEFAULT 'first_last'", []);
        }
        if !has_week_start {
            let _ = conn.execute("ALTER TABLE Configuration ADD COLUMN week_start TEXT NOT NULL DEFAULT 'monday'", []);
        }
        if !has_language {
            let _ = conn.execute("ALTER TABLE Configuration ADD COLUMN language TEXT NOT NULL DEFAULT 'system'", []);
        }
        if !has_date_format {
            let _ = conn.execute("ALTER TABLE Configuration ADD COLUMN date_format TEXT NOT NULL DEFAULT 'YYYY-MM-DD'", []);
        }
        // Ensure Relationships.kind column exists
        {
            let mut stmt = conn.prepare("PRAGMA table_info(Relationships)")?;
            let mut has_kind = false;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let col_name: String = row.get(1)?; // 1 = name
                if col_name == "kind" { has_kind = true; }
            }
            if !has_kind {
                let _ = conn.execute("ALTER TABLE Relationships ADD COLUMN kind TEXT NOT NULL DEFAULT 'recommended'", []);
            }
        }
        Ok(())
    }

    #[cfg(feature = "encryption")]
    fn apply_key(conn: &mut Connection) -> Result<()> {
        let key_path = key_file_path();
        let key_bytes = if key_path.exists() {
            std::fs::read(&key_path).expect("read key file")
        } else {
            let mut rng = rand::rngs::OsRng;
            let mut secret = [0u8; 32];
            rng.fill_bytes(&mut secret);
            std::fs::write(&key_path, &secret).expect("store key file");
            secret.to_vec()
        };
        let hex_key = hex::encode(&key_bytes);
        let _ = conn.execute(&format!("PRAGMA key = '{}';", hex_key), []);
        let _ = conn.execute("PRAGMA cipher_memory_security = ON;", []);
        Ok(())
    }
    #[cfg(feature = "encryption")]
    fn key_file_path() -> PathBuf {
        let mut base = dirs_next::data_local_dir().unwrap_or(std::env::current_dir().unwrap());
        base.push("dx_app");
        base.push("key.bin");
        base
    }

    // Helper trait
    trait OptionalRow { type Output; fn optional(self) -> Result<Option<Self::Output>>; }
    impl<T> OptionalRow for rusqlite::Result<T> { type Output = T; fn optional(self) -> Result<Option<T>> { match self { Ok(v)=>Ok(Some(v)), Err(rusqlite::Error::QueryReturnedNoRows)=>Ok(None), Err(e)=>Err(e) } } }
}

// Wasm path re-export minimal store API
#[cfg(target_arch = "wasm32")] pub mod wasm_store;


#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
const MIGRATION_0001_INIT: &str = r#"
-- Core tables
CREATE TABLE IF NOT EXISTS Configuration (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    congregation_name TEXT NOT NULL,
    theme TEXT NOT NULL DEFAULT 'System',
    name_order TEXT NOT NULL DEFAULT 'first_last',
    week_start TEXT NOT NULL DEFAULT 'monday',
    language TEXT NOT NULL DEFAULT 'system',
    date_format TEXT NOT NULL DEFAULT 'YYYY-MM-DD'
);
INSERT OR IGNORE INTO Configuration (id, congregation_name, theme, name_order, week_start, language, date_format) VALUES (1, 'Congregation', 'System', 'first_last', 'monday', 'system', 'YYYY-MM-DD');

CREATE TABLE IF NOT EXISTS Publishers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    gender TEXT NOT NULL CHECK (gender IN ('Male','Female')),
    is_shift_manager INTEGER NOT NULL DEFAULT 0,
    priority INTEGER NOT NULL DEFAULT 5
);

CREATE TABLE IF NOT EXISTS Schedules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    location TEXT NOT NULL,
    start_hour TEXT NOT NULL,
    end_hour TEXT NOT NULL,
    weekday TEXT NOT NULL,
    description TEXT,
    num_publishers INTEGER NOT NULL,
    num_shift_managers INTEGER NOT NULL,
    num_brothers INTEGER NOT NULL,
    num_sisters INTEGER NOT NULL,
    CHECK (num_shift_managers + num_brothers + num_sisters <= num_publishers)
);

CREATE TABLE IF NOT EXISTS Absences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    publisher_id INTEGER NOT NULL REFERENCES Publishers(id) ON DELETE CASCADE,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    description TEXT
);

CREATE TABLE IF NOT EXISTS Shifts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    start_datetime TEXT NOT NULL,
    end_datetime TEXT NOT NULL,
    location TEXT NOT NULL,
    publishers TEXT NOT NULL DEFAULT '[]',
    warning TEXT
);

-- Join tables
CREATE TABLE IF NOT EXISTS Availability (
    publisher_id INTEGER NOT NULL REFERENCES Publishers(id) ON DELETE CASCADE,
    schedule_id INTEGER NOT NULL REFERENCES Schedules(id) ON DELETE CASCADE,
    PRIMARY KEY (publisher_id, schedule_id)
);

CREATE TABLE IF NOT EXISTS Relationships (
    publisher_a_id INTEGER NOT NULL REFERENCES Publishers(id) ON DELETE CASCADE,
    publisher_b_id INTEGER NOT NULL REFERENCES Publishers(id) ON DELETE CASCADE,
    kind TEXT NOT NULL DEFAULT 'recommended',
    PRIMARY KEY (publisher_a_id, publisher_b_id),
    CHECK (publisher_a_id != publisher_b_id)
);

-- Indexes for performance
CREATE INDEX IF NOT EXISTS idx_absences_publisher ON Absences(publisher_id);
CREATE INDEX IF NOT EXISTS idx_shifts_start ON Shifts(start_datetime);
CREATE INDEX IF NOT EXISTS idx_availability_schedule ON Availability(schedule_id);

"#;

// Native connection re-export for external code
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
pub use native::connection;
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
pub use native::dao;
