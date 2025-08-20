#![cfg(feature = "native-db")]
use crate::db::connection;
use chrono::{NaiveDate, NaiveDateTime};
use rusqlite::{params, Result, Row};
use serde::{Deserialize, Serialize};
use serde_json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publisher {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub gender: String,
    pub is_shift_manager: bool,
    pub priority: i64,
}

impl Publisher {
    fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            first_name: row.get(1)?,
            last_name: row.get(2)?,
            gender: row.get(3)?,
            is_shift_manager: row.get::<_, i64>(4)? != 0,
            priority: row.get(5)?,
        })
    }
}

pub fn list_publishers() -> Result<Vec<Publisher>> {
    let conn = connection();
    let mut stmt = conn.prepare("SELECT id, first_name, last_name, gender, is_shift_manager, priority FROM Publishers ORDER BY first_name, last_name")?;
    let rows = stmt.query_map([], |r| Publisher::from_row(r))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create_publisher(first: &str, last: &str, gender: &str, is_shift_manager: bool, priority: i64) -> Result<i64> {
    let conn = connection();
    conn.execute(
        "INSERT INTO Publishers (first_name, last_name, gender, is_shift_manager, priority) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![first, last, gender, if is_shift_manager {1} else {0}, priority],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_publisher(id: i64, first: &str, last: &str, gender: &str, is_shift_manager: bool, priority: i64) -> Result<()> {
    let conn = connection();
    conn.execute(
        "UPDATE Publishers SET first_name=?1, last_name=?2, gender=?3, is_shift_manager=?4, priority=?5 WHERE id=?6",
        params![first, last, gender, if is_shift_manager {1} else {0}, priority, id],
    )?;
    Ok(())
}

pub fn delete_publisher(id: i64) -> Result<()> {
    let conn = connection();
    conn.execute("DELETE FROM Publishers WHERE id=?1", params![id])?;
    Ok(())
}

pub fn cleanup_expired_absences(today: NaiveDate) -> Result<usize> {
    let conn = connection();
    let count = conn.execute("DELETE FROM Absences WHERE end_date < ?1", [today.to_string()])?;
    Ok(count)
}

// ================= Configuration =================
#[derive(Debug, Clone)]
pub struct Configuration {
    pub congregation_name: String,
    pub theme: String,
    pub name_order: String, // 'first_last' or 'last_first'
    pub week_start: String, // 'monday' or 'sunday'
    pub language: String,   // 'system' | 'en' | 'es' | 'fr' | 'de'
    pub date_format: String, // 'YYYY-MM-DD' | 'DD/MM/YYYY' | 'MM/DD/YYYY' | 'DD MMM YYYY'
}

pub fn get_configuration() -> Result<Configuration> {
    let conn = connection();
    conn.query_row(
    "SELECT congregation_name, theme, name_order, week_start, language, date_format FROM Configuration WHERE id = 1",
        [],
        |r| {
            Ok(Configuration {
                congregation_name: r.get(0)?,
                theme: r.get(1)?,
                name_order: r.get(2).unwrap_or_else(|_| "first_last".to_string()),
                week_start: r.get(3).unwrap_or_else(|_| "monday".to_string()),
                language: r.get(4).unwrap_or_else(|_| "system".to_string()),
                date_format: r.get(5).unwrap_or_else(|_| "YYYY-MM-DD".to_string()),
            })
        },
    )
}

pub fn update_configuration(name: &str, theme: &str, name_order: &str, week_start: &str, language: &str, date_format: &str) -> Result<()> {
    let conn = connection();
    conn.execute(
        "UPDATE Configuration SET congregation_name=?1, theme=?2, name_order=?3, week_start=?4, language=?5, date_format=?6 WHERE id=1",
        params![name, theme, name_order, week_start, language, date_format],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn configuration_is_set() -> bool {
    if let Ok(cfg) = get_configuration() {
        let name_ok = !cfg.congregation_name.trim().is_empty() && cfg.congregation_name != "Congregation";
        let theme_ok = !cfg.theme.trim().is_empty() && cfg.theme != "System";
        name_ok && theme_ok
    } else { false }
}

// ================= Schedules =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schedule {
    pub id: i64,
    pub location: String,
    pub start_hour: String,
    pub end_hour: String,
    pub weekday: String,
    pub description: Option<String>,
    pub num_publishers: i64,
    pub num_shift_managers: i64,
    pub num_brothers: i64,
    pub num_sisters: i64,
}

impl Schedule {
    fn from_row(r: &Row) -> Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            location: r.get(1)?,
            start_hour: r.get(2)?,
            end_hour: r.get(3)?,
            weekday: r.get(4)?,
            description: r.get(5)?,
            num_publishers: r.get(6)?,
            num_shift_managers: r.get(7)?,
            num_brothers: r.get(8)?,
            num_sisters: r.get(9)?,
        })
    }
}

pub fn list_schedules() -> Result<Vec<Schedule>> {
    let conn = connection();
    let mut stmt = conn.prepare("SELECT id, location, start_hour, end_hour, weekday, description, num_publishers, num_shift_managers, num_brothers, num_sisters FROM Schedules ORDER BY weekday, start_hour")?;
    let rows = stmt.query_map([], |r| Schedule::from_row(r))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create_schedule(s: &Schedule) -> Result<i64> {
    validate_schedule_counts(s)?;
    let conn = connection();
    conn.execute("INSERT INTO Schedules (location, start_hour, end_hour, weekday, description, num_publishers, num_shift_managers, num_brothers, num_sisters) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![s.location, s.start_hour, s.end_hour, s.weekday, s.description, s.num_publishers, s.num_shift_managers, s.num_brothers, s.num_sisters])?;
    Ok(conn.last_insert_rowid())
}

pub fn update_schedule(s: &Schedule) -> Result<()> {
    validate_schedule_counts(s)?;
    let conn = connection();
    conn.execute("UPDATE Schedules SET location=?1, start_hour=?2, end_hour=?3, weekday=?4, description=?5, num_publishers=?6, num_shift_managers=?7, num_brothers=?8, num_sisters=?9 WHERE id=?10",
        params![s.location, s.start_hour, s.end_hour, s.weekday, s.description, s.num_publishers, s.num_shift_managers, s.num_brothers, s.num_sisters, s.id])?;
    Ok(())
}

pub fn delete_schedule(id: i64) -> Result<()> {
    let conn = connection();
    conn.execute("DELETE FROM Schedules WHERE id=?1", params![id])?;
    Ok(())
}

fn validate_schedule_counts(s: &Schedule) -> Result<()> {
    if s.num_shift_managers + s.num_brothers + s.num_sisters > s.num_publishers { 
        return Err(rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Integer, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "slot counts exceed total publishers"))));
    }
    Ok(())
}

// ================= Absences =================
#[derive(Debug, Clone)]
pub struct Absence { pub id: i64, pub publisher_id: i64, pub start_date: NaiveDate, pub end_date: NaiveDate, pub description: Option<String> }

impl Absence { fn from_row(r: &Row) -> Result<Self> { Ok(Self { id: r.get(0)?, publisher_id: r.get(1)?, start_date: NaiveDate::parse_from_str(&r.get::<_, String>(2)?, "%Y-%m-%d").unwrap(), end_date: NaiveDate::parse_from_str(&r.get::<_, String>(3)?, "%Y-%m-%d").unwrap(), description: r.get(4)? }) } }

pub fn list_future_absences(today: NaiveDate) -> Result<Vec<Absence>> {
    let conn = connection();
    let mut stmt = conn.prepare("SELECT id, publisher_id, start_date, end_date, description FROM Absences WHERE end_date >= ?1 ORDER BY start_date")?;
    let rows = stmt.query_map([today.to_string()], |r| Absence::from_row(r))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create_absence(publisher_id: i64, start: NaiveDate, end: NaiveDate, desc: Option<&str>) -> Result<i64> {
    let conn = connection();
    conn.execute("INSERT INTO Absences (publisher_id, start_date, end_date, description) VALUES (?1, ?2, ?3, ?4)", params![publisher_id, start.to_string(), end.to_string(), desc])?;
    Ok(conn.last_insert_rowid())
}

pub fn update_absence(id: i64, publisher_id: i64, start: NaiveDate, end: NaiveDate, desc: Option<&str>) -> Result<()> {
    let conn = connection();
    conn.execute("UPDATE Absences SET publisher_id=?1, start_date=?2, end_date=?3, description=?4 WHERE id=?5", params![publisher_id, start.to_string(), end.to_string(), desc, id])?;
    Ok(())
}

pub fn delete_absence(id: i64) -> Result<()> { let conn = connection(); conn.execute("DELETE FROM Absences WHERE id=?1", params![id])?; Ok(()) }

pub fn is_absent_on(publisher_id: i64, day: NaiveDate) -> Result<bool> {
    let conn = connection();
    let mut stmt = conn.prepare("SELECT COUNT(1) FROM Absences WHERE publisher_id=?1 AND start_date <= ?2 AND end_date >= ?2")?;
    let count: i64 = stmt.query_row(params![publisher_id, day.to_string()], |r| r.get(0))?;
    Ok(count > 0)
}

// ================= Shifts =================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shift {
    pub id: i64,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub location: String,
    pub publishers: Vec<i64>,
    pub warning: Option<String>,
}

impl Shift {
    fn from_row(r: &Row) -> Result<Self> {
        let publishers_json: String = r.get(4)?;
        let publishers: Vec<i64> = serde_json::from_str(&publishers_json).unwrap_or_default();
        Ok(Self { id: r.get(0)?, start: NaiveDateTime::parse_from_str(&r.get::<_, String>(1)?, "%Y-%m-%d %H:%M:%S").unwrap(), end: NaiveDateTime::parse_from_str(&r.get::<_, String>(2)?, "%Y-%m-%d %H:%M:%S").unwrap(), location: r.get(3)?, publishers, warning: r.get(5)? })
    }
}

pub fn list_shifts_between(start: NaiveDateTime, end: NaiveDateTime) -> Result<Vec<Shift>> {
    let conn = connection();
    let mut stmt = conn.prepare("SELECT id, start_datetime, end_datetime, location, publishers, warning FROM Shifts WHERE start_datetime >= ?1 AND end_datetime <= ?2 ORDER BY start_datetime")?;
    let rows = stmt.query_map(params![start.format("%Y-%m-%d %H:%M:%S").to_string(), end.format("%Y-%m-%d %H:%M:%S").to_string()], |r| Shift::from_row(r))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn create_shift(start: NaiveDateTime, end: NaiveDateTime, location: &str, publishers: &[i64], warning: Option<&str>) -> Result<i64> {
    let conn = connection();
    let pubs_json = serde_json::to_string(publishers).unwrap();
    conn.execute("INSERT INTO Shifts (start_datetime, end_datetime, location, publishers, warning) VALUES (?1, ?2, ?3, ?4, ?5)", params![start.format("%Y-%m-%d %H:%M:%S").to_string(), end.format("%Y-%m-%d %H:%M:%S").to_string(), location, pubs_json, warning])?;
    Ok(conn.last_insert_rowid())
}

pub fn update_shift_publishers(id: i64, publishers: &[i64], warning: Option<&str>) -> Result<()> {
    let conn = connection();
    let pubs_json = serde_json::to_string(publishers).unwrap();
    conn.execute("UPDATE Shifts SET publishers=?1, warning=?2 WHERE id=?3", params![pubs_json, warning, id])?;
    Ok(())
}

#[allow(dead_code)]
pub fn update_shift_datetime(id: i64, start: NaiveDateTime, end: NaiveDateTime, warning: Option<&str>) -> Result<()> {
    let conn = connection();
    conn.execute(
        "UPDATE Shifts SET start_datetime=?1, end_datetime=?2, warning=?3 WHERE id=?4",
        params![
            start.format("%Y-%m-%d %H:%M:%S").to_string(),
            end.format("%Y-%m-%d %H:%M:%S").to_string(),
            warning,
            id
        ],
    )?;
    Ok(())
}

pub fn update_shift_datetime_location(id: i64, start: NaiveDateTime, end: NaiveDateTime, location: &str, warning: Option<&str>) -> Result<()> {
    let conn = connection();
    conn.execute(
        "UPDATE Shifts SET start_datetime=?1, end_datetime=?2, location=?3, warning=?4 WHERE id=?5",
        params![
            start.format("%Y-%m-%d %H:%M:%S").to_string(),
            end.format("%Y-%m-%d %H:%M:%S").to_string(),
            location,
            warning,
            id
        ],
    )?;
    Ok(())
}

pub fn delete_shift(id: i64) -> Result<()> { let conn = connection(); conn.execute("DELETE FROM Shifts WHERE id=?1", params![id])?; Ok(()) }

#[allow(dead_code)]
pub fn delete_shifts_in_range(start: NaiveDateTime, end: NaiveDateTime) -> Result<usize> { let conn = connection(); let n = conn.execute("DELETE FROM Shifts WHERE start_datetime >= ?1 AND end_datetime <= ?2", params![start.format("%Y-%m-%d %H:%M:%S").to_string(), end.format("%Y-%m-%d %H:%M:%S").to_string()])?; Ok(n) }

// ================= Availability =================
pub fn set_publisher_availability(publisher_id: i64, schedule_ids: &[i64]) -> Result<()> {
    let conn = connection();
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM Availability WHERE publisher_id=?1", params![publisher_id])?;
    {
        let mut stmt = tx.prepare("INSERT INTO Availability (publisher_id, schedule_id) VALUES (?1, ?2)")?;
        for sid in schedule_ids { stmt.execute(params![publisher_id, sid])?; }
    }
    tx.commit()?;
    Ok(())
}

pub fn list_availability_for_publisher(publisher_id: i64) -> Result<Vec<i64>> {
    let conn = connection();
    let mut stmt = conn.prepare("SELECT schedule_id FROM Availability WHERE publisher_id=?1")?;
    let rows = stmt.query_map(params![publisher_id], |r| r.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

pub fn list_publishers_for_schedule(schedule_id: i64) -> Result<Vec<i64>> {
    let conn = connection();
    let mut stmt = conn.prepare("SELECT publisher_id FROM Availability WHERE schedule_id=?1")?;
    let rows = stmt.query_map(params![schedule_id], |r| r.get(0))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ================= Relationships =================
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationshipKind { Mandatory, Recommended }

impl RelationshipKind {
    fn as_str(&self) -> &'static str { match self { RelationshipKind::Mandatory => "mandatory", RelationshipKind::Recommended => "recommended" } }
    fn from_db(s: String) -> Self { match s.as_str() { "mandatory" => RelationshipKind::Mandatory, _ => RelationshipKind::Recommended } }
}

pub fn add_relationship(a: i64, b: i64, kind: RelationshipKind) -> Result<()> {
    if a==b { return Ok(()); }
    let (x,y) = if a<b {(a,b)} else {(b,a)};
    let conn = connection();
    conn.execute(
        "INSERT INTO Relationships (publisher_a_id, publisher_b_id, kind) VALUES (?1, ?2, ?3) ON CONFLICT(publisher_a_id, publisher_b_id) DO UPDATE SET kind=excluded.kind",
        params![x,y, kind.as_str()],
    )?;
    Ok(())
}
pub fn remove_relationship(a: i64, b: i64) -> Result<()> { if a==b { return Ok(()); } let (x,y) = if a<b {(a,b)} else {(b,a)}; let conn = connection(); conn.execute("DELETE FROM Relationships WHERE publisher_a_id=?1 AND publisher_b_id=?2", params![x,y])?; Ok(()) }
pub fn list_relationships_for_publisher(p: i64) -> Result<Vec<(i64, RelationshipKind)>> {
    let conn = connection();
    let mut stmt = conn.prepare("SELECT CASE WHEN publisher_a_id = ?1 THEN publisher_b_id ELSE publisher_a_id END AS other, kind FROM Relationships WHERE publisher_a_id = ?1 OR publisher_b_id = ?1")?;
    let rows = stmt.query_map(params![p], |r| Ok((r.get(0)?, RelationshipKind::from_db(r.get::<_, String>(1)?))))?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

// ================= Export/Import (excluding Configuration) =================
#[derive(Serialize, Deserialize)]
pub struct AbsenceExport { pub id: i64, pub publisher_id: i64, pub start_date: String, pub end_date: String, pub description: Option<String> }

#[derive(Serialize, Deserialize)]
pub struct AvailabilityExport(pub i64, pub i64);

#[derive(Serialize, Deserialize)]
pub struct RelationshipExport(pub i64, pub i64, pub String);

#[derive(Serialize, Deserialize)]
pub struct ExportPayload {
    pub publishers: Vec<Publisher>,
    pub schedules: Vec<Schedule>,
    pub absences: Vec<AbsenceExport>,
    pub shifts: Vec<Shift>,
    pub availability: Vec<AvailabilityExport>,
    pub relationships: Vec<RelationshipExport>,
}

pub fn export_data() -> Result<String> {
    let conn = connection();
    // publishers
    let publishers = {
        let mut stmt = conn.prepare("SELECT id, first_name, last_name, gender, is_shift_manager, priority FROM Publishers ORDER BY id")?;
        let rows = stmt.query_map([], |r| Publisher::from_row(r))?;
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
    };
    // schedules
    let schedules = list_schedules()?;
    // absences (all)
    let absences = {
        let mut stmt = conn.prepare("SELECT id, publisher_id, start_date, end_date, description FROM Absences ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok(AbsenceExport { id: r.get(0)?, publisher_id: r.get(1)?, start_date: r.get::<_, String>(2)?, end_date: r.get::<_, String>(3)?, description: r.get(4)? }))?;
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
    };
    // shifts (all)
    let shifts = {
        let mut stmt = conn.prepare("SELECT id, start_datetime, end_datetime, location, publishers, warning FROM Shifts ORDER BY id")?;
        let rows = stmt.query_map([], |r| Shift::from_row(r))?;
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
    };
    // availability
    let availability = {
        let mut stmt = conn.prepare("SELECT publisher_id, schedule_id FROM Availability ORDER BY publisher_id, schedule_id")?;
        let rows = stmt.query_map([], |r| Ok(AvailabilityExport(r.get(0)?, r.get(1)?)))?;
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
    };
    // relationships
    let relationships = {
        let mut stmt = conn.prepare("SELECT publisher_a_id, publisher_b_id, kind FROM Relationships ORDER BY publisher_a_id, publisher_b_id")?;
        let rows = stmt.query_map([], |r| Ok(RelationshipExport(r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
    };

    let payload = ExportPayload { publishers, schedules, absences, shifts, availability, relationships };
    Ok(serde_json::to_string_pretty(&payload).unwrap())
}

pub fn import_data(json: &str) -> Result<()> {
    let payload: ExportPayload = serde_json::from_str(json).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    let conn = connection();
    let tx = conn.unchecked_transaction()?;
    // Clear all (respect FK constraints)
    tx.execute("DELETE FROM Availability", [])?;
    tx.execute("DELETE FROM Relationships", [])?;
    tx.execute("DELETE FROM Shifts", [])?;
    tx.execute("DELETE FROM Absences", [])?;
    tx.execute("DELETE FROM Schedules", [])?;
    tx.execute("DELETE FROM Publishers", [])?;
    // Publishers
    {
        let mut stmt = tx.prepare("INSERT INTO Publishers (id, first_name, last_name, gender, is_shift_manager, priority) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
        for p in &payload.publishers {
            stmt.execute(params![p.id, p.first_name, p.last_name, p.gender, if p.is_shift_manager {1} else {0}, p.priority])?;
        }
    }
    // Schedules
    {
        let mut stmt = tx.prepare("INSERT INTO Schedules (id, location, start_hour, end_hour, weekday, description, num_publishers, num_shift_managers, num_brothers, num_sisters) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)")?;
        for s in &payload.schedules {
            stmt.execute(params![s.id, s.location, s.start_hour, s.end_hour, s.weekday, s.description, s.num_publishers, s.num_shift_managers, s.num_brothers, s.num_sisters])?;
        }
    }
    // Absences
    {
        let mut stmt = tx.prepare("INSERT INTO Absences (id, publisher_id, start_date, end_date, description) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for a in &payload.absences {
            stmt.execute(params![a.id, a.publisher_id, a.start_date, a.end_date, a.description])?;
        }
    }
    // Shifts
    {
        let mut stmt = tx.prepare("INSERT INTO Shifts (id, start_datetime, end_datetime, location, publishers, warning) VALUES (?1, ?2, ?3, ?4, ?5, ?6)")?;
        for sh in &payload.shifts {
            let pubs_json = serde_json::to_string(&sh.publishers).unwrap_or_else(|_| "[]".to_string());
            stmt.execute(params![sh.id, sh.start.format("%Y-%m-%d %H:%M:%S").to_string(), sh.end.format("%Y-%m-%d %H:%M:%S").to_string(), sh.location, pubs_json, sh.warning])?;
        }
    }
    // Availability
    {
        let mut stmt = tx.prepare("INSERT INTO Availability (publisher_id, schedule_id) VALUES (?1, ?2)")?;
        for AvailabilityExport(p, s) in &payload.availability { stmt.execute(params![p, s])?; }
    }
    // Relationships
    {
        let mut stmt = tx.prepare("INSERT INTO Relationships (publisher_a_id, publisher_b_id, kind) VALUES (?1, ?2, ?3)")?;
        for RelationshipExport(a, b, k) in &payload.relationships { stmt.execute(params![a, b, k])?; }
    }

    tx.commit()?;
    Ok(())
}

// Destructive: remove all data from database (keeps Configuration row)
pub fn reset_data() -> Result<()> {
    let conn = connection();
    let tx = conn.unchecked_transaction()?;
    tx.execute("DELETE FROM Availability", [])?;
    tx.execute("DELETE FROM Relationships", [])?;
    tx.execute("DELETE FROM Shifts", [])?;
    tx.execute("DELETE FROM Absences", [])?;
    tx.execute("DELETE FROM Schedules", [])?;
    tx.execute("DELETE FROM Publishers", [])?;
    // Reset configuration to defaults/unset so landing page shows
    tx.execute("UPDATE Configuration SET congregation_name='Congregation', theme='System', name_order='first_last', week_start='monday', language='system', date_format='YYYY-MM-DD' WHERE id=1", [])?;
    tx.commit()?;
    Ok(())
}

