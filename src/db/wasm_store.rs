use serde::{Serialize, Deserialize};
use serde_json;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use web_sys::{window, Storage};

const KEY_PUBLISHERS: &str = "dx_app_publishers";
const KEY_CONFIGURATION: &str = "dx_app_configuration";

fn storage() -> Storage { window().and_then(|w| w.local_storage().ok().flatten()).expect("localStorage") }

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Publisher {
    pub id: i64,
    pub first_name: String,
    pub last_name: String,
    pub gender: String,
    pub is_shift_manager: bool,
    pub priority: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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

#[derive(Default, Serialize, Deserialize)]
struct WasmDb {
    publishers: Vec<Publisher>,
    next_id: i64,
    // schedules support
    #[serde(default)]
    schedules: Vec<Schedule>,
    #[serde(default)]
    next_schedule_id: i64,
    // availability (publisher_id, schedule_id)
    #[serde(default)]
    availability: Vec<(i64, i64)>,
    // absences
    #[serde(default)]
    absences: Vec<Absence>,
    #[serde(default)]
    next_absence_id: i64,
    // shifts
    #[serde(default)]
    shifts: Vec<Shift>,
    #[serde(default)]
    next_shift_id: i64,
    // relationships (a,b,kind) where a<b
    #[serde(default)]
    relationships: Vec<(i64, i64, String)>,
}

static DB: Lazy<Mutex<WasmDb>> = Lazy::new(|| {
    let raw = storage().get_item(KEY_PUBLISHERS).ok().flatten();
    let mut db: WasmDb = raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
    if db.next_id <= 0 { db.next_id = 1; }
    if db.next_schedule_id <= 0 { db.next_schedule_id = 1; }
    if db.next_absence_id <= 0 { db.next_absence_id = 1; }
    if db.next_shift_id <= 0 { db.next_shift_id = 1; }
    if db.relationships.is_empty() { /* keep default empty */ }
    let db = db;
    Mutex::new(db)
});

fn persist() {
    if let Ok(db) = DB.lock() {
        if let Ok(json) = serde_json::to_string(&*db) {
            let _ = storage().set_item(KEY_PUBLISHERS, &json);
        }
    }
}

// API mirrors a subset of native dao
pub fn list_publishers() -> Vec<Publisher> {
    let mut v = DB.lock().unwrap().publishers.clone();
    v.sort_by(|a,b| a.first_name.to_lowercase().cmp(&b.first_name.to_lowercase()).then(a.last_name.to_lowercase().cmp(&b.last_name.to_lowercase())));
    v
}

pub fn create_publisher(first: &str, last: &str, gender: &str, is_shift_manager: bool, priority: i64) -> i64 {
    let mut db = DB.lock().unwrap();
    let id = db.next_id;
    db.next_id += 1;
    db.publishers.push(Publisher { id, first_name: first.into(), last_name: last.into(), gender: gender.into(), is_shift_manager, priority });
    drop(db);
    persist();
    id
}

pub fn delete_publisher(id: i64) {
    let mut db = DB.lock().unwrap();
    db.publishers.retain(|p| p.id != id);
    // cascade remove availability for this publisher
    db.availability.retain(|(p, _s)| *p != id);
    // cascade remove absences for this publisher
    db.absences.retain(|a| a.publisher_id != id);
    drop(db);
    persist();
}

pub fn update_publisher(id: i64, first: &str, last: &str, gender: &str, is_shift_manager: bool, priority: i64) {
    let mut db = DB.lock().unwrap();
    if let Some(p) = db.publishers.iter_mut().find(|p| p.id == id) {
        p.first_name = first.to_string();
        p.last_name = last.to_string();
        p.gender = gender.to_string();
        p.is_shift_manager = is_shift_manager;
        p.priority = priority;
    }
    drop(db);
    persist();
}

// ================= Schedules (web) =================
pub fn list_schedules() -> Vec<Schedule> {
    let mut v = DB.lock().unwrap().schedules.clone();
    v.sort_by(|a,b| a.weekday.cmp(&b.weekday).then(a.start_hour.cmp(&b.start_hour)));
    v
}

pub fn get_name_order() -> String { get_configuration().map(|c| c.name_order).unwrap_or_else(|| "first_last".into()) }

pub fn create_schedule(s: &Schedule) -> i64 {
    let mut db = DB.lock().unwrap();
    let id = db.next_schedule_id;
    db.next_schedule_id += 1;
    let mut new_s = s.clone();
    new_s.id = id;
    db.schedules.push(new_s);
    drop(db);
    persist();
    id
}

pub fn update_schedule(s: &Schedule) {
    let mut db = DB.lock().unwrap();
    if let Some(existing) = db.schedules.iter_mut().find(|x| x.id == s.id) {
        *existing = s.clone();
    }
    drop(db);
    persist();
}

pub fn delete_schedule(id: i64) {
    let mut db = DB.lock().unwrap();
    db.schedules.retain(|x| x.id != id);
    // cascade remove availability entries with this schedule
    db.availability.retain(|(_p, s)| *s != id);
    drop(db);
    persist();
}

// ================= Availability =================
pub fn list_availability_for_publisher(publisher_id: i64) -> Vec<i64> {
    DB.lock().unwrap().availability.iter().filter_map(|(p, s)| if *p == publisher_id { Some(*s) } else { None }).collect()
}

pub fn set_publisher_availability(publisher_id: i64, schedule_ids: &[i64]) {
    let mut db = DB.lock().unwrap();
    db.availability.retain(|(p, _)| *p != publisher_id);
    for sid in schedule_ids { db.availability.push((publisher_id, *sid)); }
    drop(db);
    persist();
}

pub fn list_publishers_for_schedule(schedule_id: i64) -> Vec<i64> {
    DB.lock().unwrap().availability.iter().filter_map(|(p, s)| if *s == schedule_id { Some(*p) } else { None }).collect()
}

// ================= Relationships =================
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum RelationshipKind { Mandatory, Recommended }

impl RelationshipKind {
    fn as_str(&self) -> &'static str { match self { RelationshipKind::Mandatory => "mandatory", RelationshipKind::Recommended => "recommended" } }
    fn from_str(s: &str) -> Self { match s { "mandatory" => RelationshipKind::Mandatory, _ => RelationshipKind::Recommended } }
}

pub fn add_relationship(a: i64, b: i64, kind: RelationshipKind) {
    if a == b { return; }
    let (x,y) = if a<b {(a,b)} else {(b,a)};
    let mut db = DB.lock().unwrap();
    if let Some(row) = db.relationships.iter_mut().find(|(aa,bb,_)| *aa==x && *bb==y) { row.2 = kind.as_str().to_string(); }
    else { db.relationships.push((x,y, kind.as_str().to_string())); }
    drop(db);
    persist();
}

pub fn remove_relationship(a: i64, b: i64) {
    if a == b { return; }
    let (x,y) = if a<b {(a,b)} else {(b,a)};
    let mut db = DB.lock().unwrap();
    db.relationships.retain(|(aa,bb,_)| !(*aa==x && *bb==y));
    drop(db);
    persist();
}

pub fn list_relationships_for_publisher(p: i64) -> Vec<(i64, RelationshipKind)> {
    let db = DB.lock().unwrap();
    db.relationships.iter().filter_map(|(a,b,k)| {
        if *a==p { Some((*b, RelationshipKind::from_str(k))) }
        else if *b==p { Some((*a, RelationshipKind::from_str(k))) }
        else { None }
    }).collect()
}

// ================= Absences =================
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Absence { pub id: i64, pub publisher_id: i64, pub start_date: String, pub end_date: String, pub description: Option<String> }

pub fn list_absences() -> Vec<Absence> { DB.lock().unwrap().absences.clone() }

pub fn list_future_absences(today: &str) -> Vec<Absence> {
    let db = DB.lock().unwrap();
    let t = today.to_string();
    db.absences.iter().cloned().filter(|a| a.end_date >= t).collect()
}

pub fn cleanup_expired_absences(today: &str) {
    let mut db = DB.lock().unwrap();
    let t = today.to_string();
    db.absences.retain(|a| a.end_date >= t);
    drop(db);
    persist();
}

pub fn create_absence(publisher_id: i64, start_date: &str, end_date: &str, description: Option<&str>) -> i64 {
    let mut db = DB.lock().unwrap();
    let id = db.next_absence_id;
    db.next_absence_id += 1;
    db.absences.push(Absence { id, publisher_id, start_date: start_date.to_string(), end_date: end_date.to_string(), description: description.map(|s| s.to_string()) });
    drop(db);
    persist();
    id
}

pub fn update_absence(id: i64, publisher_id: i64, start_date: &str, end_date: &str, description: Option<&str>) {
    let mut db = DB.lock().unwrap();
    if let Some(a) = db.absences.iter_mut().find(|x| x.id == id) {
        a.publisher_id = publisher_id;
        a.start_date = start_date.to_string();
        a.end_date = end_date.to_string();
        a.description = description.map(|s| s.to_string());
    }
    drop(db);
    persist();
}

pub fn delete_absence(id: i64) {
    let mut db = DB.lock().unwrap();
    db.absences.retain(|a| a.id != id);
    drop(db);
    persist();
}

// ================= Shifts (web) =================
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Shift {
    pub id: i64,
    pub start_datetime: String, // "%Y-%m-%d %H:%M:%S"
    pub end_datetime: String,
    pub location: String,
    pub publishers: Vec<i64>,
    pub warning: Option<String>,
}

pub fn list_shifts_between(start: &str, end: &str) -> Vec<Shift> {
    let db = DB.lock().unwrap();
    let s = start.to_string();
    let e = end.to_string();
    let mut v: Vec<_> = db.shifts.iter().cloned().filter(|sh| sh.start_datetime >= s && sh.end_datetime <= e).collect();
    v.sort_by(|a,b| a.start_datetime.cmp(&b.start_datetime));
    v
}

pub fn create_shift(start: &str, end: &str, location: &str, publishers: &[i64], warning: Option<&str>) -> i64 {
    let mut db = DB.lock().unwrap();
    let id = db.next_shift_id;
    db.next_shift_id += 1;
    db.shifts.push(Shift { id, start_datetime: start.to_string(), end_datetime: end.to_string(), location: location.to_string(), publishers: publishers.to_vec(), warning: warning.map(|s| s.to_string()) });
    drop(db);
    persist();
    id
}

pub fn update_shift_publishers(id: i64, publishers: &[i64], warning: Option<&str>) {
    let mut db = DB.lock().unwrap();
    if let Some(sh) = db.shifts.iter_mut().find(|s| s.id == id) {
        sh.publishers = publishers.to_vec();
        sh.warning = warning.map(|s| s.to_string());
    }
    drop(db);
    persist();
}

// Update shift start/end datetimes (web)
pub fn update_shift_datetime(id: i64, start: &str, end: &str, warning: Option<&str>) {
    let mut db = DB.lock().unwrap();
    if let Some(sh) = db.shifts.iter_mut().find(|s| s.id == id) {
        sh.start_datetime = start.to_string();
        sh.end_datetime = end.to_string();
        sh.warning = warning.map(|s| s.to_string());
    }
    drop(db);
    persist();
}

pub fn update_shift_datetime_location(id: i64, start: &str, end: &str, location: &str, warning: Option<&str>) {
    let mut db = DB.lock().unwrap();
    if let Some(sh) = db.shifts.iter_mut().find(|s| s.id == id) {
        sh.start_datetime = start.to_string();
        sh.end_datetime = end.to_string();
        sh.location = location.to_string();
        sh.warning = warning.map(|s| s.to_string());
    }
    drop(db);
    persist();
}

pub fn delete_shift(id: i64) {
    let mut db = DB.lock().unwrap();
    db.shifts.retain(|s| s.id != id);
    drop(db);
    persist();
}

pub fn delete_shifts_in_range(start: &str, end: &str) -> usize {
    let mut db = DB.lock().unwrap();
    let s = start.to_string();
    let e = end.to_string();
    let before = db.shifts.len();
    db.shifts.retain(|sh| !(sh.start_datetime >= s && sh.end_datetime <= e));
    let removed = before - db.shifts.len();
    drop(db);
    persist();
    removed
}

pub fn is_absent_on(publisher_id: i64, ymd: &str) -> bool {
    let d = ymd.to_string();
    DB.lock().unwrap().absences.iter().any(|a| a.publisher_id == publisher_id && a.start_date <= d && a.end_date >= d)
}

// Configuration stored as separate JSON object to keep compatibility
#[derive(Serialize, Deserialize, Clone)]
pub struct Configuration {
    pub congregation_name: String,
    pub theme: String,
    #[serde(default="default_name_order")] pub name_order: String,
    #[serde(default="default_week_start")] pub week_start: String,
    #[serde(default="default_language")] pub language: String,
    #[serde(default="default_date_format")] pub date_format: String,
}

fn default_name_order() -> String { "first_last".to_string() }
fn default_week_start() -> String { "monday".to_string() }
fn default_language() -> String { "system".to_string() }
fn default_date_format() -> String { "YYYY-MM-DD".to_string() }

pub fn get_configuration() -> Option<Configuration> {
    storage().get_item(KEY_CONFIGURATION).ok().flatten().and_then(|s| serde_json::from_str(&s).ok())
}

pub fn update_configuration(name: &str, theme: &str, name_order: &str, week_start: &str, language: &str, date_format: &str) {
    let cfg = Configuration {
        congregation_name: name.to_string(),
        theme: theme.to_string(),
        name_order: if name_order.is_empty() { default_name_order() } else { name_order.to_string() },
    week_start: if week_start.is_empty() { default_week_start() } else { week_start.to_string() },
    language: if language.is_empty() { default_language() } else { language.to_string() },
    date_format: if date_format.is_empty() { default_date_format() } else { date_format.to_string() },
    };
    if let Ok(json) = serde_json::to_string(&cfg) { let _ = storage().set_item(KEY_CONFIGURATION, &json); }
}

pub fn configuration_is_set() -> bool {
    if let Some(cfg) = get_configuration() {
    let name_ok = !cfg.congregation_name.trim().is_empty() && cfg.congregation_name != "Congregation";
    // Accept any theme value, including "System"
    return name_ok;
    }
    false
}

// Export/Import (excluding Configuration)
#[derive(Serialize, Deserialize)]
pub struct ExportPayload {
    pub publishers: Vec<Publisher>,
    pub next_id: i64,
    #[serde(default)]
    pub schedules: Vec<Schedule>,
    #[serde(default)]
    pub next_schedule_id: i64,
    #[serde(default)]
    pub availability: Vec<(i64, i64)>,
    #[serde(default)]
    pub absences: Vec<Absence>,
    #[serde(default)]
    pub next_absence_id: i64,
    #[serde(default)]
    pub shifts: Vec<Shift>,
    #[serde(default)]
    pub next_shift_id: i64,
    #[serde(default)]
    pub relationships: Vec<(i64, i64, String)>,
}

pub fn export_data() -> String {
    let db = DB.lock().unwrap();
    serde_json::to_string_pretty(&ExportPayload {
        publishers: db.publishers.clone(),
        next_id: db.next_id,
        schedules: db.schedules.clone(),
        next_schedule_id: db.next_schedule_id,
    availability: db.availability.clone(),
    absences: db.absences.clone(),
    next_absence_id: db.next_absence_id,
    shifts: db.shifts.clone(),
    next_shift_id: db.next_shift_id,
    relationships: db.relationships.clone(),
    }).unwrap()
}

pub fn import_data(json: &str) -> bool {
    if let Ok(payload) = serde_json::from_str::<ExportPayload>(json) {
        if let Ok(mut db) = DB.lock() {
            db.publishers = payload.publishers;
            db.next_id = payload.next_id.max(1);
            db.schedules = payload.schedules;
            db.next_schedule_id = payload.next_schedule_id.max(1);
            db.availability = payload.availability;
            db.absences = payload.absences;
            db.next_absence_id = payload.next_absence_id.max(1);
            db.shifts = payload.shifts;
            db.next_shift_id = payload.next_shift_id.max(1);
            db.relationships = payload.relationships;
            drop(db);
            persist();
            return true;
        }
    }
    false
}

// Wipe all data (except configuration)
pub fn reset_data() -> bool {
    if let Ok(mut db) = DB.lock() {
        db.publishers.clear();
        db.next_id = 1;
        db.schedules.clear();
        db.next_schedule_id = 1;
        db.availability.clear();
        db.absences.clear();
        db.next_absence_id = 1;
        db.shifts.clear();
        db.next_shift_id = 1;
        db.relationships.clear();
        drop(db);
        persist();
    // also clear configuration entry so app shows landing again
    let _ = storage().remove_item(KEY_CONFIGURATION);
        true
    } else { false }
}
