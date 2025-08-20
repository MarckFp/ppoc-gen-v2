use dioxus::prelude::*;
use crate::i18n::{
    t,
    weekdays_for_locale,
    weekday_name_for_date,
};
#[cfg(any(all(feature = "native-db", not(target_arch = "wasm32")), target_arch = "wasm32"))]
use crate::i18n::{weekday_index_for_date, weekday_index_from_name};

// Backends
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
use crate::db::dao;
#[cfg(target_arch = "wasm32")]
use crate::db::wasm_store as wasm_backend;

// Date/time imports per target
#[cfg(not(target_arch = "wasm32"))]
use chrono::{Datelike, NaiveDate};
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
use chrono::Duration;
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
use chrono::{NaiveDateTime, NaiveTime};
#[cfg(target_arch = "wasm32")]
use web_sys::window;
#[cfg(target_arch = "wasm32")]
use js_sys as js;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::Closure, JsCast};

#[derive(Clone)]
struct ShiftItem {
    id: i64,
    date: String,
    title: String,
    publishers: Vec<i64>,
    location: String,
    start_hour: String,
    end_hour: String,
}

#[derive(Clone, Default)]
struct ManualForm {
    loc: String,
    start_dt: String, // YYYY-MM-DDTHH:MM
    end_dt: String,   // YYYY-MM-DDTHH:MM
    selected_pids: Vec<i64>,
    add_pid: String,
}

#[derive(Clone, Default)]
struct AutoForm {
    start: String,
    end: String,
}

#[derive(Clone, Default)]
struct EditForm {
    shift_id: i64,
    loc: String,
    start_dt: String, // YYYY-MM-DDTHH:MM
    end_dt: String,   // YYYY-MM-DDTHH:MM
    selected_pids: Vec<i64>,
    add_pid: String,
}

#[derive(Clone)]
struct PublisherItem { id: i64, label: String }

#[derive(Clone)]
#[allow(dead_code)]
struct ScheduleFull { location: String }

// Date helpers used across the view
fn fmt_date_ymd(ymd: &(i32, u32, u32)) -> String { format!("{:04}-{:02}-{:02}", ymd.0, ymd.1, ymd.2) }

#[cfg(not(target_arch = "wasm32"))]
fn month_start_end(y: i32, m: u32) -> (i32, u32, u32) {
    let last = if m == 12 { chrono::NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap() - chrono::Duration::days(1) } else { chrono::NaiveDate::from_ymd_opt(y, m + 1, 1).unwrap() - chrono::Duration::days(1) };
    (y, m, last.day())
}

#[cfg(target_arch = "wasm32")]
fn month_start_end(y: i32, m: u32) -> (i32, u32, u32) {
    // JS trick: day 0 of next month = last day of current
    let last = js::Date::new_with_year_month_day(y as u32, m as i32, 0);
    (y, m, last.get_date() as u32)
}

// Cross-target helper for current year/month
#[cfg(target_arch = "wasm32")]
fn now_year_month() -> (i32, u32) {
    let d = js::Date::new_0();
    (d.get_full_year() as i32, d.get_month() as u32 + 1)
}

#[cfg(not(target_arch = "wasm32"))]
fn now_year_month() -> (i32, u32) {
    use chrono::Datelike;
    let now = chrono::Local::now().naive_local().date();
    (now.year(), now.month())
}

#[cfg(target_arch = "wasm32")]
fn parse_ymd(s: &str) -> (i32, u32, u32) {
    let parts: Vec<_> = s.split('-').collect();
    let y = parts.get(0).and_then(|v| v.parse::<i32>().ok()).unwrap_or(1970);
    let m = parts.get(1).and_then(|v| v.parse::<u32>().ok()).unwrap_or(1);
    let d = parts.get(2).and_then(|v| v.parse::<u32>().ok()).unwrap_or(1);
    (y, m, d)
}

#[component]
#[allow(unused_mut, unused_variables)]
pub fn Shifts() -> Element {
    // state
    let mut view = use_signal(|| "month".to_string()); // or "agenda"
    let mut is_small_screen = use_signal(|| false);
    let mut manual_open = use_signal(|| false);
    let mut edit_open = use_signal(|| false);
    let mut auto_open = use_signal(|| false);
    let mut export_open = use_signal(|| false);
    let mut generating = use_signal(|| false);
    // forms
    let mut auto_form = use_signal(AutoForm::default);
    #[derive(Clone, Default)]
    struct ExportForm { start: String, end: String }
    let mut export_form = use_signal(ExportForm::default);

    // data/signals required by the view
    let (yy, mm) = now_year_month();
    let mut year = use_signal(move || yy);
    let mut month = use_signal(move || mm);
    let mut list = use_signal(|| Vec::<ShiftItem>::new());
    let mut publishers_all = use_signal(|| Vec::<PublisherItem>::new());
    let mut schedules_full = use_signal(|| Vec::<ScheduleFull>::new());
    let mut selected_ids = use_signal(|| std::collections::BTreeSet::<i64>::new());
    let mut select_mode = use_signal(|| false);
    let mut manual_form = use_signal(ManualForm::default);
    let mut edit_form = use_signal(EditForm::default);
    let mut confirm_delete_id = use_signal(|| None as Option<i64>);
    let mut loc_suggestions = use_signal(|| Vec::<String>::new());

    // helper to refresh current month list and suggestions
    let refresh_month = {
        let mut list = list.clone();
        let year = year.clone();
        let month = month.clone();
        let mut publishers_all = publishers_all.clone();
        let mut schedules_full_sig = schedules_full.clone();
        let mut loc_suggestions = loc_suggestions.clone();
        move || {
            let (y, m, last_day) = month_start_end(year(), month());
            // Load publishers and schedules (full) for both targets
            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            {
                // publishers
                let pubs = dao::list_publishers().unwrap_or_default();
                let name_order = dao::get_configuration().ok().map(|c| c.name_order).unwrap_or_else(|| "first_last".into());
                let mapped: Vec<PublisherItem> = pubs
                    .iter()
                    .map(|p| {
                        let label = if name_order == "last_first" { format!("{} {}", p.last_name, p.first_name) } else { format!("{} {}", p.first_name, p.last_name) };
                        PublisherItem { id: p.id, label }
                    })
                    .collect();
                publishers_all.set(mapped);
                // schedules full
                let sch = dao::list_schedules().unwrap_or_default();
                let full: Vec<ScheduleFull> = sch.iter().map(|s| ScheduleFull { location: s.location.clone() }).collect();
                schedules_full_sig.set(full.clone());

                // list items for current month
                let start = NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                let end = NaiveDateTime::new(chrono::NaiveDate::from_ymd_opt(y, m, last_day).unwrap(), chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap());
                if let Ok(shifts) = dao::list_shifts_between(start, end) {
                    let items: Vec<ShiftItem> = shifts
                        .into_iter()
                        .map(|s| {
                            let date = s.start.date().to_string();
                            let title = format!("{} • {}–{}", s.location, s.start.format("%H:%M"), s.end.format("%H:%M"));
                            ShiftItem {
                                id: s.id,
                                date,
                                title,
                                publishers: s.publishers.clone(),
                                location: s.location.clone(),
                                start_hour: s.start.format("%H:%M").to_string(),
                                end_hour: s.end.format("%H:%M").to_string(),
                            }
                        })
                        .collect();
                    list.set(items.clone());
                    // suggestions from schedules + items
                    use std::collections::BTreeSet;
                    let mut set: BTreeSet<String> = full.iter().map(|s| s.location.clone()).collect();
                    for it in items { set.insert(it.location); }
                    loc_suggestions.set(set.into_iter().collect());
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                // publishers
                let pubs = wasm_backend::list_publishers();
                let name_order = wasm_backend::get_name_order();
                let mapped: Vec<PublisherItem> = pubs
                    .iter()
                    .map(|p| {
                        let label = if name_order == "last_first" { format!("{} {}", p.last_name, p.first_name) } else { format!("{} {}", p.first_name, p.last_name) };
                        PublisherItem { id: p.id, label }
                    })
                    .collect();
                publishers_all.set(mapped);
                // schedules full
                let sch = wasm_backend::list_schedules();
                let full: Vec<ScheduleFull> = sch.iter().map(|s| ScheduleFull { location: s.location.clone() }).collect();
                schedules_full_sig.set(full.clone());

                // list items for current month
                let start = format!("{:04}-{:02}-01 00:00:00", y, m);
                let end = format!("{:04}-{:02}-{:02} 23:59:59", y, m, last_day);
                let shifts = wasm_backend::list_shifts_between(&start, &end);
                let items: Vec<ShiftItem> = shifts
                    .into_iter()
                    .map(|s| {
                        let date = s.start_datetime[0..10].to_string();
                        let title = format!("{} • {}–{}", s.location, &s.start_datetime[11..16], &s.end_datetime[11..16]);
                        ShiftItem {
                            id: s.id,
                            date,
                            title,
                            publishers: s.publishers.clone(),
                            location: s.location.clone(),
                            start_hour: s.start_datetime[11..16].to_string(),
                            end_hour: s.end_datetime[11..16].to_string(),
                        }
                    })
                    .collect();
                // set items and build suggestions
                use std::collections::BTreeSet;
                let mut set: BTreeSet<String> = full.iter().map(|s| s.location.clone()).collect();
                for it in &items { set.insert(it.location.clone()); }
                list.set(items);
                loc_suggestions.set(set.into_iter().collect());
            }
        }
    };

    // initialize current month range and screen size; refresh data
    {
        let mut auto_form = auto_form.clone();
    let mut refresh = refresh_month.clone();
        use_effect(move || {
            let (yy, mm) = now_year_month();
            let (_, _, last) = month_start_end(yy, mm);
            auto_form.write().start = fmt_date_ymd(&(yy, mm, 1));
            auto_form.write().end = fmt_date_ymd(&(yy, mm, last));
            refresh();
        });
    }
    #[cfg(target_arch = "wasm32")]
    {
        // set initial screen size and attach resize listener
        let mut is_small_screen2 = is_small_screen.clone();
        let mut view2 = view.clone();
        use_effect(move || {
            // initial
            let width = window().and_then(|w| w.inner_width().ok()).and_then(|v| v.as_f64()).unwrap_or(1024.0);
            let small = width < 640.0;
            is_small_screen2.set(small);
            if small && view2() != "agenda" { view2.set("agenda".to_string()); }
            // listener
            let mut is_small_screen2 = is_small_screen2.clone();
            let mut view2 = view2.clone();
            let cb = Closure::wrap(Box::new(move || {
                let width = window().and_then(|w| w.inner_width().ok()).and_then(|v| v.as_f64()).unwrap_or(1024.0);
                let small = width < 640.0;
                let prev = is_small_screen2();
                if prev != small { is_small_screen2.set(small); }
                if small && view2() != "agenda" { view2.set("agenda".to_string()); }
            }) as Box<dyn FnMut()>);
            if let Some(w) = window() { let _ = w.add_event_listener_with_callback("resize", cb.as_ref().unchecked_ref()); }
            cb.forget();
        });
    }

    // delete handlers
    let delete_one = {
        let mut refresh = refresh_month.clone();
        move |id: i64| {
            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            { let _ = dao::delete_shift(id); }
            #[cfg(target_arch = "wasm32")]
            { let _ = wasm_backend::delete_shift(id); }
            refresh();
        }
    };
    let bulk_delete = {
    let mut selected_ids = selected_ids.clone();
        let mut refresh = refresh_month.clone();
        move |_| {
            let ids: Vec<i64> = selected_ids.read().iter().cloned().collect();
            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            { for id in ids { let _ = dao::delete_shift(id); } }
            #[cfg(target_arch = "wasm32")]
            { for id in ids { let _ = wasm_backend::delete_shift(id); } }
            selected_ids.write().clear();
            refresh();
        }
    };

    let prev_month = {
    let mut year = year.clone();
    let mut month = month.clone();
    let mut refresh = refresh_month.clone();
        move |_| {
            let mut yy = year();
            let mut mm = month();
            if mm == 1 { mm = 12; yy -= 1; } else { mm -= 1; }
            year.set(yy);
            month.set(mm);
            let (_, _, last) = month_start_end(yy, mm);
            auto_form.write().start = fmt_date_ymd(&(yy, mm, 1));
            auto_form.write().end = fmt_date_ymd(&(yy, mm, last));
            refresh();
        }
    };
    let next_month = {
    let mut year = year.clone();
    let mut month = month.clone();
    let mut refresh = refresh_month.clone();
        move |_| {
            let mut yy = year();
            let mut mm = month();
            if mm == 12 { mm = 1; yy += 1; } else { mm += 1; }
            year.set(yy);
            month.set(mm);
            let (_, _, last) = month_start_end(yy, mm);
            auto_form.write().start = fmt_date_ymd(&(yy, mm, 1));
            auto_form.write().end = fmt_date_ymd(&(yy, mm, last));
            refresh();
        }
    };
    let this_month = {
    let mut year = year.clone();
    let mut month = month.clone();
    let mut refresh = refresh_month.clone();
        move |_| {
            let (yy, mm) = now_year_month();
            year.set(yy);
            month.set(mm);
            let (_, _, last) = month_start_end(yy, mm);
            auto_form.write().start = fmt_date_ymd(&(yy, mm, 1));
            auto_form.write().end = fmt_date_ymd(&(yy, mm, last));
            refresh();
        }
    };

    // manual create submit
    let manual_submit = {
    let manual_form = manual_form.clone();
    let mut manual_open = manual_open.clone();
    let mut refresh = refresh_month.clone();
        move |_| {
            let f = manual_form.read().clone();
            if f.loc.trim().is_empty() || f.start_dt.is_empty() || f.end_dt.is_empty() { return; }
            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            {
                use chrono::NaiveDateTime;
                let start = f.start_dt.replace('T', " ") + ":00";
                let end = f.end_dt.replace('T', " ") + ":00";
                if let (Ok(st), Ok(et)) = (NaiveDateTime::parse_from_str(&start, "%Y-%m-%d %H:%M:%S"), NaiveDateTime::parse_from_str(&end, "%Y-%m-%d %H:%M:%S")) {
                    let _ = dao::create_shift(st, et, &f.loc, &f.selected_pids, None);
                    refresh();
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                let start = f.start_dt.replace('T', " ") + ":00";
                let end = f.end_dt.replace('T', " ") + ":00";
                let _ = wasm_backend::create_shift(&start, &end, &f.loc, &f.selected_pids, None);
                refresh();
            }
            manual_open.set(false);
        }
    };

    // edit submit (update publishers and date/time/location)
    let edit_submit = {
        let edit_form = edit_form.clone();
        let mut edit_open = edit_open.clone();
        let mut refresh = refresh_month.clone();
        move |_| {
            let f = edit_form.read();
            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            {
                use chrono::NaiveDateTime;
                // update publishers
                let _ = dao::update_shift_publishers(f.shift_id, &f.selected_pids, None);
                // update datetime + location if valid
                if !f.start_dt.is_empty() && !f.end_dt.is_empty() {
                    let start = f.start_dt.replace('T', " ") + ":00";
                    let end = f.end_dt.replace('T', " ") + ":00";
                    if let (Ok(st), Ok(et)) = (NaiveDateTime::parse_from_str(&start, "%Y-%m-%d %H:%M:%S"), NaiveDateTime::parse_from_str(&end, "%Y-%m-%d %H:%M:%S")) {
                        let _ = dao::update_shift_datetime_location(f.shift_id, st, et, &f.loc, None);
                    }
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                // update publishers
                let _ = wasm_backend::update_shift_publishers(f.shift_id, &f.selected_pids, None);
                // update datetime + location
                if !f.start_dt.is_empty() && !f.end_dt.is_empty() {
                    let start = f.start_dt.replace('T', " ") + ":00";
                    let end = f.end_dt.replace('T', " ") + ":00";
                    let _ = wasm_backend::update_shift_datetime_location(f.shift_id, &start, &end, &f.loc, None);
                }
            }
            refresh();
            edit_open.set(false);
        }
    };

    // auto-generate (synchronous, blocks UI while running)
    let do_autogen = {
    let mut generating_sig = generating.clone();
    let auto_form_sig = auto_form.clone();
    let mut auto_open_sig = auto_open.clone();
    let refresh_fn = refresh_month.clone();
        move |_| {
            if generating_sig() { return; }
            // Close the modal first to avoid re-entrant borrows from conditional UI
            auto_open_sig.set(false);

            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            {
                generating_sig.set(true);
                let form = auto_form_sig.read().clone();
                use std::collections::{HashMap, HashSet};
                let start_d = NaiveDate::parse_from_str(&form.start, "%Y-%m-%d").unwrap();
                let end_d = NaiveDate::parse_from_str(&form.end, "%Y-%m-%d").unwrap();
                let schedules = dao::list_schedules().unwrap_or_default();
                let publishers = dao::list_publishers().unwrap_or_default();
                // relationships map
                let mut rel_map: HashMap<i64, Vec<(i64, dao::RelationshipKind)>> = HashMap::new();
                for p in &publishers { if let Ok(rs) = dao::list_relationships_for_publisher(p.id) { rel_map.insert(p.id, rs); } }
                // fairness window
                let hist_start = start_d - Duration::days(60);
                let hist = dao::list_shifts_between(
                    NaiveDateTime::new(hist_start, NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
                    NaiveDateTime::new(end_d, NaiveTime::from_hms_opt(23, 59, 59).unwrap()),
                )
                .unwrap_or_default();
                let mut recent_count: HashMap<i64, i32> = HashMap::new();
                let mut pair_count: HashMap<(i64, i64), i32> = HashMap::new();
                for sh in &hist {
                    for &p in &sh.publishers {
                        *recent_count.entry(p).or_insert(0) += 1;
                    }
                    for i in 0..sh.publishers.len() {
                        for j in (i + 1)..sh.publishers.len() {
                            let a = sh.publishers[i].min(sh.publishers[j]);
                            let b = sh.publishers[i].max(sh.publishers[j]);
                            *pair_count.entry((a, b)).or_insert(0) += 1;
                        }
                    }
                }
                let mut assigned_on_day: HashMap<NaiveDate, HashSet<i64>> = HashMap::new();
                let seed = chrono::Local::now().timestamp_nanos_opt().unwrap_or(0) as u64;
                let mut rand_for = |pid: i64, day: NaiveDate| -> f64 {
                    let x = (pid as u64)
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(seed ^ (day.num_days_from_ce() as u64));
                    ((x >> 11) as f64) / (u64::MAX >> 11) as f64
                };

                let mut d = start_d;
                while d <= end_d {
                    let day_idx = weekday_index_for_date(d.year(), d.month(), d.day());
                    for s in schedules.iter() {
                        if weekday_index_from_name(&s.weekday) != day_idx { continue; }
                        let start_dt = NaiveDateTime::new(d, NaiveTime::parse_from_str(&s.start_hour, "%H:%M").unwrap());
                        let end_dt = NaiveDateTime::new(d, NaiveTime::parse_from_str(&s.end_hour, "%H:%M").unwrap());
                        // skip existing identical shift
                        let existing = dao::list_shifts_between(start_dt, end_dt).unwrap_or_default();
                        if existing.iter().any(|e| e.location == s.location && e.start == start_dt && e.end == end_dt) { continue; }
                        // candidates
                        let avail_ids = dao::list_publishers_for_schedule(s.id).unwrap_or_default();
                        let day_assigned = assigned_on_day.entry(d).or_default().clone();
                        let candidates: Vec<_> = publishers
                            .iter()
                            .filter(|p| avail_ids.contains(&p.id))
                            .filter(|p| !day_assigned.contains(&p.id))
                            .filter(|p| !dao::is_absent_on(p.id, d).unwrap_or(false))
                            .cloned()
                            .collect();

                        let score = |p_id: i64, selected: &Vec<i64>| -> f64 {
                            let p = publishers.iter().find(|x| x.id == p_id).unwrap();
                            let base = (p.priority as f64) * 10.0;
                            let rec_pen = (*recent_count.get(&p_id).unwrap_or(&0)) as f64 * 2.0;
                            let pair_pen: f64 = selected
                                .iter()
                                .map(|&o| {
                                    let a = p_id.min(o);
                                    let b = p_id.max(o);
                                    (*pair_count.get(&(a, b)).unwrap_or(&0)) as f64 * 1.5
                                })
                                .sum();
                            // relationship bonus for recommended, enforced for mandatory
                            let mut rel_bonus = 0.0;
                            if let Some(rs) = rel_map.get(&p_id) {
                                for &o in selected.iter() {
                                    if let Some((_, kind)) = rs.iter().find(|(oid, _)| *oid == o) {
                                        match kind { dao::RelationshipKind::Recommended => rel_bonus += 2.0, dao::RelationshipKind::Mandatory => rel_bonus += 5.0 }
                                    }
                                }
                            }
                            let jitter = rand_for(p_id, d) * 3.0;
                            base + jitter + rel_bonus - rec_pen - pair_pen
                        };

                        let mut selected: Vec<i64> = Vec::new();
                        let mut warning: Option<String> = None;
                        // managers first (male)
                        let mut mgr_pool: Vec<_> = candidates
                            .iter()
                            .filter(|p| p.is_shift_manager && p.gender == "Male")
                            .map(|p| p.id)
                            .collect();
                        mgr_pool.sort_by(|a, b| {
                            use std::cmp::Ordering;
                            score(*b, &selected)
                                .partial_cmp(&score(*a, &selected))
                                .unwrap_or(Ordering::Equal)
                                .then_with(|| a.cmp(b))
                        });
                        for pid in mgr_pool.into_iter().take(s.num_shift_managers as usize) {
                            if !selected.contains(&pid) { selected.push(pid); }
                        }
                        // brothers (male) including managers
                        let male_have = selected.iter().filter(|pid| publishers.iter().any(|p| p.id == **pid && p.gender == "Male")).count();
                        let male_needed = (s.num_brothers as usize).saturating_sub(male_have);
                        let mut male_pool: Vec<_> = candidates.iter().filter(|p| p.gender == "Male" && !selected.contains(&p.id)).map(|p| p.id).collect();
                        male_pool.sort_by(|a, b| {
                            use std::cmp::Ordering;
                            score(*b, &selected)
                                .partial_cmp(&score(*a, &selected))
                                .unwrap_or(Ordering::Equal)
                                .then_with(|| a.cmp(b))
                        });
                        for pid in male_pool.into_iter().take(male_needed) { selected.push(pid); }
                        // sisters
                        let female_needed = s.num_sisters as usize;
                        let mut female_pool: Vec<_> = candidates.iter().filter(|p| p.gender == "Female" && !selected.contains(&p.id)).map(|p| p.id).collect();
                        female_pool.sort_by(|a, b| {
                            use std::cmp::Ordering;
                            score(*b, &selected)
                                .partial_cmp(&score(*a, &selected))
                                .unwrap_or(Ordering::Equal)
                                .then_with(|| a.cmp(b))
                        });
                        for pid in female_pool.into_iter().take(female_needed) { selected.push(pid); }
                        // Enforce mandatory relationships: if one selected, ensure its mandatory partners are added if available
                        {
                            let mut must_have: Vec<i64> = Vec::new();
                            for pid in selected.iter().copied() {
                                if let Some(rs) = rel_map.get(&pid) {
                                    for (oid, k) in rs.iter() {
                                        if matches!(k, dao::RelationshipKind::Mandatory) { must_have.push(*oid); }
                                    }
                                }
                            }
                            for oid in must_have {
                                if !selected.contains(&oid) && candidates.iter().any(|p| p.id==oid) && !dao::is_absent_on(oid, d).unwrap_or(false) {
                                    selected.push(oid);
                                }
                            }
                        }
                        // Rebalance to respect manager and gender minima
                        {
                            use std::collections::HashSet;
                            // Build quick lookups
                            let is_manager = |pid: i64| publishers.iter().any(|p| p.id==pid && p.is_shift_manager && p.gender=="Male");
                            let is_male = |pid: i64| publishers.iter().any(|p| p.id==pid && p.gender=="Male");
                            let is_female = |pid: i64| publishers.iter().any(|p| p.id==pid && p.gender=="Female");
                            let mut mandatory_set: HashSet<i64> = HashSet::new();
                            let sel_snapshot = selected.clone();
                            for &pid in &sel_snapshot { if let Some(rs) = rel_map.get(&pid) { for &(oid, ref k) in rs.iter() { if matches!(k, dao::RelationshipKind::Mandatory) && sel_snapshot.contains(&oid) { mandatory_set.insert(pid); mandatory_set.insert(oid); } } } }
                            let mut count_mgr = selected.iter().filter(|pid| is_manager(**pid)).count() as i64;
                            let mut count_male = selected.iter().filter(|pid| is_male(**pid)).count() as i64;
                            let mut count_female = selected.iter().filter(|pid| is_female(**pid)).count() as i64;

                            // Reduce managers if exceeding required
                            if count_mgr > s.num_shift_managers {
                                let excess = (count_mgr - s.num_shift_managers) as usize;
                                // candidates to remove: managers not in mandatory_set, lowest score first
                                let mut mgr_removals: Vec<(i64, f64)> = selected.iter().cloned().filter(|pid| is_manager(*pid) && !mandatory_set.contains(pid)).map(|pid| (pid, score(pid, &selected))).collect();
                                mgr_removals.sort_by(|a,b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                let mut removed = 0usize;
                                for (pid,_sc) in mgr_removals {
                                    if removed >= excess { break; }
                                    if let Some(pos) = selected.iter().position(|x| *x==pid) { selected.remove(pos); removed+=1; count_mgr-=1; count_male-=1; }
                                }
                                if removed < excess { warning = Some("Could not reduce extra managers due to mandatory pairs".into()); }
                            }

                            // Ensure minimum sisters
                            while count_female < s.num_sisters {
                                // pick best female candidate not selected
                                let mut fem_pool: Vec<(i64,f64)> = candidates.iter().filter(|p| p.gender=="Female" && !selected.contains(&p.id)).map(|p| (p.id, score(p.id, &selected))).collect();
                                if fem_pool.is_empty() { warning = Some("Fewer sisters available than required".into()); break; }
                                fem_pool.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                                let (add_id, _) = fem_pool[0];
                                if selected.len() >= s.num_publishers as usize {
                                    // remove lowest scoring non-mandatory male (prefer non-manager)
                                    let mut male_candidates: Vec<(i64,f64)> = selected.iter().cloned().filter(|pid| is_male(*pid) && !mandatory_set.contains(pid) && !is_manager(*pid)).map(|pid| (pid, score(pid, &selected))).collect();
                                    if male_candidates.is_empty() { male_candidates = selected.iter().cloned().filter(|pid| is_male(*pid) && !mandatory_set.contains(pid)).map(|pid| (pid, score(pid, &selected))).collect(); }
                                    male_candidates.sort_by(|a,b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                    if let Some((rm,_)) = male_candidates.first().cloned() { if let Some(pos)=selected.iter().position(|x| *x==rm){ selected.remove(pos); if is_manager(rm){ count_mgr-=1; } count_male-=1; } } else { warning = Some("Cannot free slot to add required sister".into()); break; }
                                }
                                selected.push(add_id); count_female+=1;
                            }

                            // Ensure minimum brothers
                            while count_male < s.num_brothers {
                                // pick best male candidate not selected (prefer non-manager if manager quota met)
                                let prefer_non_mgr = count_mgr >= s.num_shift_managers;
                                let mut male_pool: Vec<(i64,f64)> = candidates.iter().filter(|p| p.gender=="Male" && !selected.contains(&p.id) && (!prefer_non_mgr || !p.is_shift_manager)).map(|p| (p.id, score(p.id, &selected))).collect();
                                if male_pool.is_empty() { // fallback allow managers
                                    male_pool = candidates.iter().filter(|p| p.gender=="Male" && !selected.contains(&p.id)).map(|p| (p.id, score(p.id, &selected))).collect();
                                }
                                if male_pool.is_empty() { warning = Some("Fewer brothers available than required".into()); break; }
                                male_pool.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                                let (add_id, _) = male_pool[0];
                                if selected.len() >= s.num_publishers as usize {
                                    // remove lowest scoring non-mandatory female
                                    let mut fem_candidates: Vec<(i64,f64)> = selected.iter().cloned().filter(|pid| is_female(*pid) && !mandatory_set.contains(pid)).map(|pid| (pid, score(pid, &selected))).collect();
                                    fem_candidates.sort_by(|a,b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                    if let Some((rm,_)) = fem_candidates.first().cloned() { if let Some(pos)=selected.iter().position(|x| *x==rm){ selected.remove(pos); count_female-=1; } } else { warning = Some("Cannot free slot to add required brother".into()); break; }
                                }
                                if is_manager(add_id) { count_mgr+=1; }
                                selected.push(add_id); count_male+=1;
                            }
                        }
                        // fill remaining
                        let remaining_slots = (s.num_publishers as usize).saturating_sub(selected.len());
                        if remaining_slots > 0 {
                            let mut rest: Vec<_> = candidates.iter().filter(|p| !selected.contains(&p.id)).map(|p| p.id).collect();
                            rest.sort_by(|a, b| {
                                use std::cmp::Ordering;
                                score(*b, &selected)
                                    .partial_cmp(&score(*a, &selected))
                                    .unwrap_or(Ordering::Equal)
                                    .then_with(|| a.cmp(b))
                            });
                            for pid in rest.into_iter().take(remaining_slots) { selected.push(pid); }
                        }
                        // Ensure we don't exceed capacity; prefer keeping mandatory pairs
                        if selected.len() > s.num_publishers as usize {
                            use std::collections::HashSet;
                            let limit = s.num_publishers as usize;
                            let selected_clone = selected.clone();
                            let mut mandatory_set: HashSet<i64> = HashSet::new();
                            for &pid in &selected_clone {
                                if let Some(rs) = rel_map.get(&pid) {
                                    for &(oid, ref kind) in rs.iter() {
                                        if matches!(kind, dao::RelationshipKind::Mandatory) && selected_clone.contains(&oid) {
                                            mandatory_set.insert(pid);
                                            mandatory_set.insert(oid);
                                        }
                                    }
                                }
                            }
                            // removable are those not in mandatory_set
                            let mut removable: Vec<i64> = selected.iter().copied().filter(|pid| !mandatory_set.contains(pid)).collect();
                            // Build a score list to avoid borrowing 'selected' in sort comparator
                            let mut scored: Vec<(i64, f64)> = removable.iter().map(|pid| (*pid, score(*pid, &selected))).collect();
                            scored.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                            // remove lowest scoring until within limit
                            for &(pid, _) in scored.iter().rev() { // lowest score last
                                if selected.len() <= limit { break; }
                                if let Some(pos) = selected.iter().position(|x| *x == pid) { selected.remove(pos); }
                            }
                            if selected.len() > limit {
                                // As last resort, drop lowest-scoring overall
                                let mut overall: Vec<(i64,f64)> = selected.iter().map(|pid| (*pid, score(*pid, &selected))).collect();
                                overall.sort_by(|a,b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                while selected.len() > limit {
                                    if let Some((pid,_)) = overall.first().copied() {
                                        if let Some(pos) = selected.iter().position(|x| *x == pid) { selected.remove(pos); }
                                        overall.remove(0);
                                    } else { break; }
                                }
                                warning = Some("Had to drop some selections due to capacity; mandatory pairs may be affected".into());
                            }
                            if warning.is_none() { warning = Some("Trimmed extra selections to fit capacity".into()); }
                        }
                        if selected.len() < s.num_publishers as usize { warning = Some("Not enough available publishers".into()); }
                        let _ = dao::create_shift(start_dt, end_dt, &s.location, &selected, warning.as_deref());
                        let set = assigned_on_day.entry(d).or_default();
                        for &pid in &selected {
                            set.insert(pid);
                            *recent_count.entry(pid).or_insert(0) += 1;
                            for &other in &selected { if pid < other { *pair_count.entry((pid, other)).or_insert(0) += 1; } }
                        }
                    }
                    d = d.succ_opt().unwrap();
                }
                generating_sig.set(false);
                let mut refresh = refresh_fn.clone();
                refresh();
            }

            #[cfg(target_arch = "wasm32")]
            {
                // Defer heavy work to the next tick to avoid re-entrant borrows in the click handler
                let mut generating = generating_sig.clone();
                let auto_form = auto_form_sig.clone();
                let mut refresh = refresh_fn.clone();
                let cb = Closure::wrap(Box::new(move || {
                    generating.set(true);
                    let form = auto_form.read().clone();
                    use std::collections::{HashMap, HashSet};
                    let start = form.start.clone();
                    let end = form.end.clone();
                    let schedules = wasm_backend::list_schedules();
                    let publishers = wasm_backend::list_publishers();
                    // relationships map
                    let mut rel_map: std::collections::HashMap<i64, Vec<(i64, wasm_backend::RelationshipKind)>> = std::collections::HashMap::new();
                    for p in &publishers { let rs = wasm_backend::list_relationships_for_publisher(p.id); rel_map.insert(p.id, rs); }
                    // fairness from last 60 days
                    let (sy, sm, sd) = parse_ymd(&start);
                    let mut hist_start = js::Date::new_with_year_month_day(sy as u32, (sm as i32) - 1, sd as i32);
                    hist_start.set_time(hist_start.get_time() - 60.0 * 24.0 * 3600.0 * 1000.0);
                    let hist = wasm_backend::list_shifts_between(
                        &format!("{:04}-{:02}-{:02} 00:00:00", hist_start.get_full_year() as i32, hist_start.get_month() as u32 + 1, hist_start.get_date() as u32),
                        &format!("{} 23:59:59", end),
                    );
                    let mut recent_count: HashMap<i64, i32> = HashMap::new();
                    let mut pair_count: HashMap<(i64, i64), i32> = HashMap::new();
                    for sh in &hist {
                        for &p in &sh.publishers { *recent_count.entry(p).or_insert(0) += 1; }
                        for i in 0..sh.publishers.len() { for j in (i + 1)..sh.publishers.len() { let a = sh.publishers[i].min(sh.publishers[j]); let b = sh.publishers[i].max(sh.publishers[j]); *pair_count.entry((a, b)).or_insert(0) += 1; } }
                    }
                    let mut assigned_on_day: HashMap<String, HashSet<i64>> = HashMap::new();
                    let (ey, em, ed) = parse_ymd(&end);
                    let mut cur = js::Date::new_with_year_month_day(sy as u32, (sm as i32) - 1, sd as i32);
                    let end_d = js::Date::new_with_year_month_day(ey as u32, (em as i32) - 1, ed as i32);
                    while cur.value_of() <= end_d.value_of() {
                        let y = cur.get_full_year() as i32; let m = cur.get_month() as u32 + 1; let d = cur.get_date() as u32;
                        let ymd = format!("{:04}-{:02}-{:02}", y, m, d);
                        let day_idx = weekday_index_for_date(y, m, d);
                        // Deterministic jitter for this day
                        let day_seed: u64 = ((y as u64) << 32) ^ ((m as u64) << 16) ^ (d as u64);
                        let rand_for = |pid: i64| -> f64 {
                            let mut x = (pid as u64).wrapping_mul(6364136223846793005).wrapping_add(day_seed);
                            // splitmix64 steps
                            x ^= x >> 30; x = x.wrapping_mul(0xbf58476d1ce4e5b9);
                            x ^= x >> 27; x = x.wrapping_mul(0x94d049bb133111eb);
                            x ^= x >> 31;
                            (x as f64) / (u64::MAX as f64)
                        };
                        for s in schedules.iter() {
                            if weekday_index_from_name(&s.weekday) != day_idx { continue; }
                            let start_dt = format!("{} {}:00", ymd, s.start_hour);
                            let end_dt = format!("{} {}:00", ymd, s.end_hour);
                            let existing = wasm_backend::list_shifts_between(&start_dt, &end_dt);
                            if existing.iter().any(|e| e.location == s.location && e.start_datetime == start_dt && e.end_datetime == end_dt) { continue; }
                            let avail_ids = wasm_backend::list_publishers_for_schedule(s.id);
                            let day_assigned = assigned_on_day.entry(ymd.clone()).or_default().clone();
                            let candidates: Vec<_> = publishers.iter().filter(|p| avail_ids.contains(&p.id)).filter(|p| !day_assigned.contains(&p.id)).filter(|p| !wasm_backend::is_absent_on(p.id, &ymd)).cloned().collect();
                            let score = |p_id: i64, selected: &Vec<i64>| -> f64 {
                                let p = publishers.iter().find(|x| x.id == p_id).unwrap();
                                let base = (p.priority as f64) * 10.0;
                                let rec_pen = (*recent_count.get(&p_id).unwrap_or(&0)) as f64 * 2.0;
                                let pair_pen: f64 = selected.iter().map(|&o| { let a = p_id.min(o); let b = p_id.max(o); (*pair_count.get(&(a,b)).unwrap_or(&0)) as f64 * 1.5 }).sum();
                                let mut rel_bonus = 0.0;
                                if let Some(rs) = rel_map.get(&p_id) {
                                    for &o in selected.iter() {
                                        if let Some((_, kind)) = rs.iter().find(|(oid, _)| *oid == o) {
                                            match kind { wasm_backend::RelationshipKind::Recommended => rel_bonus += 2.0, wasm_backend::RelationshipKind::Mandatory => rel_bonus += 5.0 }
                                        }
                                    }
                                }
                                let jitter = rand_for(p_id) * 3.0;
                                base + jitter + rel_bonus - rec_pen - pair_pen
                            };
                            let mut selected: Vec<i64> = Vec::new();
                            let mut warning: Option<String> = None;
                            let mut mgr_pool: Vec<_> = candidates.iter().filter(|p| p.is_shift_manager && p.gender == "Male").map(|p| p.id).collect();
                            mgr_pool.sort_by(|a,b| {
                                use std::cmp::Ordering;
                                score(*b,&selected)
                                    .partial_cmp(&score(*a,&selected))
                                    .unwrap_or(Ordering::Equal)
                                    .then_with(|| a.cmp(b))
                            });
                            for pid in mgr_pool.into_iter().take(s.num_shift_managers as usize) { if !selected.contains(&pid) { selected.push(pid); } }
                            let male_have = selected.iter().filter(|pid| publishers.iter().any(|p| p.id == **pid && p.gender == "Male")).count();
                            let male_needed = (s.num_brothers as usize).saturating_sub(male_have);
                            let mut male_pool: Vec<_> = candidates.iter().filter(|p| p.gender == "Male" && !selected.contains(&p.id)).map(|p| p.id).collect();
                            male_pool.sort_by(|a,b| {
                                use std::cmp::Ordering;
                                score(*b,&selected)
                                    .partial_cmp(&score(*a,&selected))
                                    .unwrap_or(Ordering::Equal)
                                    .then_with(|| a.cmp(b))
                            });
                            for pid in male_pool.into_iter().take(male_needed) { selected.push(pid); }
                            let female_needed = s.num_sisters as usize;
                            let mut female_pool: Vec<_> = candidates.iter().filter(|p| p.gender == "Female" && !selected.contains(&p.id)).map(|p| p.id).collect();
                            female_pool.sort_by(|a,b| {
                                use std::cmp::Ordering;
                                score(*b,&selected)
                                    .partial_cmp(&score(*a,&selected))
                                    .unwrap_or(Ordering::Equal)
                                    .then_with(|| a.cmp(b))
                            });
                            for pid in female_pool.into_iter().take(female_needed) { selected.push(pid); }
                            // Enforce mandatory relationships for already selected publishers
                            {
                                let mut must_have: Vec<i64> = Vec::new();
                                for pid in selected.iter().copied() {
                                    if let Some(rs) = rel_map.get(&pid) {
                                        for (oid, k) in rs.iter() {
                                            if matches!(k, wasm_backend::RelationshipKind::Mandatory) { must_have.push(*oid); }
                                        }
                                    }
                                }
                                for oid in must_have {
                                    if !selected.contains(&oid) && candidates.iter().any(|p| p.id==oid) && !wasm_backend::is_absent_on(oid, &ymd) {
                                        selected.push(oid);
                                    }
                                }
                            }
                            // Rebalance to respect manager and gender minima
                            {
                                use std::collections::HashSet;
                                let is_manager = |pid: i64| publishers.iter().any(|p| p.id==pid && p.is_shift_manager && p.gender=="Male");
                                let is_male = |pid: i64| publishers.iter().any(|p| p.id==pid && p.gender=="Male");
                                let is_female = |pid: i64| publishers.iter().any(|p| p.id==pid && p.gender=="Female");
                                let mut mandatory_set: HashSet<i64> = HashSet::new();
                                let sel_snapshot = selected.clone();
                                for &pid in &sel_snapshot { if let Some(rs) = rel_map.get(&pid) { for &(oid, ref k) in rs.iter() { if matches!(k, wasm_backend::RelationshipKind::Mandatory) && sel_snapshot.contains(&oid) { mandatory_set.insert(pid); mandatory_set.insert(oid); } } } }
                                let mut count_mgr = selected.iter().filter(|pid| is_manager(**pid)).count() as i64;
                                let mut count_male = selected.iter().filter(|pid| is_male(**pid)).count() as i64;
                                let mut count_female = selected.iter().filter(|pid| is_female(**pid)).count() as i64;

                                // Reduce managers if exceeding required
                                if count_mgr > s.num_shift_managers {
                                    let excess = (count_mgr - s.num_shift_managers) as usize;
                                    let mut mgr_removals: Vec<(i64, f64)> = selected.iter().cloned().filter(|pid| is_manager(*pid) && !mandatory_set.contains(pid)).map(|pid| (pid, score(pid, &selected))).collect();
                                    mgr_removals.sort_by(|a,b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                    let mut removed = 0usize;
                                    for (pid, _) in mgr_removals { if removed>=excess { break; } if let Some(pos)=selected.iter().position(|x| *x==pid){ selected.remove(pos); removed+=1; count_mgr-=1; count_male-=1; } }
                                    if removed < excess { warning = Some("Could not reduce extra managers due to mandatory pairs".into()); }
                                }

                                // Ensure minimum sisters
                                while count_female < s.num_sisters {
                                    let mut fem_pool: Vec<(i64,f64)> = candidates.iter().filter(|p| p.gender=="Female" && !selected.contains(&p.id)).map(|p| (p.id, score(p.id, &selected))).collect();
                                    if fem_pool.is_empty() { warning = Some("Fewer sisters available than required".into()); break; }
                                    fem_pool.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                                    let (add_id, _) = fem_pool[0];
                                    if selected.len() >= s.num_publishers as usize {
                                        let mut male_candidates: Vec<(i64,f64)> = selected.iter().cloned().filter(|pid| is_male(*pid) && !mandatory_set.contains(pid) && !is_manager(*pid)).map(|pid| (pid, score(pid, &selected))).collect();
                                        if male_candidates.is_empty() { male_candidates = selected.iter().cloned().filter(|pid| is_male(*pid) && !mandatory_set.contains(pid)).map(|pid| (pid, score(pid, &selected))).collect(); }
                                        male_candidates.sort_by(|a,b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                        if let Some((rm,_)) = male_candidates.first().cloned() { if let Some(pos)=selected.iter().position(|x| *x==rm){ selected.remove(pos); if is_manager(rm){ count_mgr-=1; } count_male-=1; } } else { warning = Some("Cannot free slot to add required sister".into()); break; }
                                    }
                                    selected.push(add_id); count_female+=1;
                                }

                                // Ensure minimum brothers
                                while count_male < s.num_brothers {
                                    let prefer_non_mgr = count_mgr >= s.num_shift_managers;
                                    let mut male_pool: Vec<(i64,f64)> = candidates.iter().filter(|p| p.gender=="Male" && !selected.contains(&p.id) && (!prefer_non_mgr || !p.is_shift_manager)).map(|p| (p.id, score(p.id, &selected))).collect();
                                    if male_pool.is_empty() { male_pool = candidates.iter().filter(|p| p.gender=="Male" && !selected.contains(&p.id)).map(|p| (p.id, score(p.id, &selected))).collect(); }
                                    if male_pool.is_empty() { warning = Some("Fewer brothers available than required".into()); break; }
                                    male_pool.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                                    let (add_id, _) = male_pool[0];
                                    if selected.len() >= s.num_publishers as usize {
                                        let mut fem_candidates: Vec<(i64,f64)> = selected.iter().cloned().filter(|pid| is_female(*pid) && !mandatory_set.contains(pid)).map(|pid| (pid, score(pid, &selected))).collect();
                                        fem_candidates.sort_by(|a,b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                        if let Some((rm,_)) = fem_candidates.first().cloned() { if let Some(pos)=selected.iter().position(|x| *x==rm){ selected.remove(pos); count_female-=1; } } else { warning = Some("Cannot free slot to add required brother".into()); break; }
                                    }
                                    if is_manager(add_id) { count_mgr+=1; }
                                    selected.push(add_id); count_male+=1;
                                }
                            }
                            let remaining_slots = (s.num_publishers as usize).saturating_sub(selected.len());
                            if remaining_slots > 0 {
                                let mut rest: Vec<_> = candidates.iter().filter(|p| !selected.contains(&p.id)).map(|p| p.id).collect();
                                rest.sort_by(|a,b| {
                                    use std::cmp::Ordering;
                                    score(*b,&selected)
                                        .partial_cmp(&score(*a,&selected))
                                        .unwrap_or(Ordering::Equal)
                                        .then_with(|| a.cmp(b))
                                });
                                for pid in rest.into_iter().take(remaining_slots) { selected.push(pid); }
                            }
                            // Trim if over capacity; prefer keeping mandatory pairs
                            if selected.len() > s.num_publishers as usize {
                                use std::collections::HashSet;
                                let limit = s.num_publishers as usize;
                                let selected_clone = selected.clone();
                                let mut mandatory_set: HashSet<i64> = HashSet::new();
                                for &pid in &selected_clone {
                                    if let Some(rs) = rel_map.get(&pid) {
                                        for &(oid, ref kind) in rs.iter() {
                                            if matches!(kind, wasm_backend::RelationshipKind::Mandatory) && selected_clone.contains(&oid) {
                                                mandatory_set.insert(pid);
                                                mandatory_set.insert(oid);
                                            }
                                        }
                                    }
                                }
                                let mut removable: Vec<i64> = selected.iter().copied().filter(|pid| !mandatory_set.contains(pid)).collect();
                                let mut scored: Vec<(i64,f64)> = removable.iter().map(|pid| (*pid, score(*pid, &selected))).collect();
                                scored.sort_by(|a,b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                                for &(pid, _) in scored.iter().rev() {
                                    if selected.len() <= limit { break; }
                                    if let Some(pos) = selected.iter().position(|x| *x == pid) { selected.remove(pos); }
                                }
                                if selected.len() > limit {
                                    let mut overall: Vec<(i64,f64)> = selected.iter().map(|pid| (*pid, score(*pid, &selected))).collect();
                                    overall.sort_by(|a,b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                                    while selected.len() > limit {
                                        if let Some((pid,_)) = overall.first().copied() {
                                            if let Some(pos) = selected.iter().position(|x| *x == pid) { selected.remove(pos); }
                                            overall.remove(0);
                                        } else { break; }
                                    }
                                    warning = Some("Had to drop some selections due to capacity; mandatory pairs may be affected".into());
                                }
                                if warning.is_none() { warning = Some("Trimmed extra selections to fit capacity".into()); }
                            }
                            if selected.len() < s.num_publishers as usize { warning = Some("Not enough available publishers".into()); }
                            let _ = wasm_backend::create_shift(&start_dt, &end_dt, &s.location, &selected, warning.as_deref());
                            let set = assigned_on_day.entry(ymd.clone()).or_default();
                            for &pid in &selected { set.insert(pid); *recent_count.entry(pid).or_insert(0) += 1; for &other in &selected { if pid < other { *pair_count.entry((pid, other)).or_insert(0) += 1; } } }
                        }
                        cur.set_date(cur.get_date() + 1);
                    }
                    generating.set(false);
                    refresh();
                }) as Box<dyn FnMut()>);
                if let Some(w) = window() { let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 0); }
                cb.forget();
            }
        }
    };

    // export to PDF (web: open print-friendly window and trigger print)
    let do_export = {
        let export_form = export_form.clone();
        let publishers_all = publishers_all.clone();
        let mut export_open = export_open.clone();
        move |_| {
            let start = export_form.read().start.clone();
            let end = export_form.read().end.clone();
            if start.is_empty() || end.is_empty() { return; }
            #[cfg(target_arch = "wasm32")]
            {
                use std::collections::BTreeMap;
                // fetch shifts
                let shifts = wasm_backend::list_shifts_between(&format!("{} 00:00:00", start), &format!("{} 23:59:59", end));
                // map publishers
                let mut name_for: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
                for p in publishers_all.read().iter() { name_for.insert(p.id, p.label.clone()); }
                // group by date and sort by start time
                let mut by_day: BTreeMap<String, Vec<(String, String, Vec<String>)>> = BTreeMap::new();
                for s in shifts.into_iter() {
                    let date = s.start_datetime[0..10].to_string();
                    let start_h = s.start_datetime[11..16].to_string();
                    let end_h = s.end_datetime[11..16].to_string();
                    let loc = s.location.clone();
                    let mut names: Vec<String> = s.publishers.iter().map(|pid| name_for.get(pid).cloned().unwrap_or_else(|| format!("#{}", pid))).collect();
                    names.sort();
                    let entry = by_day.entry(date).or_default();
                    entry.push((format!("{}–{}", start_h, end_h), loc, names));
                }
                for (_d, v) in by_day.iter_mut() { v.sort_by(|a,b| a.0.cmp(&b.0)); }
                // helpers
                fn esc(s: &str) -> String {
                    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
                }
                fn human_date(ymd: &str) -> String {
                    let parts: Vec<&str> = ymd.split('-').collect();
                    if parts.len() != 3 { return ymd.to_string(); }
                    let y = parts[0].parse::<i32>().unwrap_or(1970);
                    let m = parts[1].parse::<u32>().unwrap_or(1);
                    let d = parts[2].parse::<u32>().unwrap_or(1);
                    let wd = weekday_name_for_date(y,m,d);
                    let names = ["January","February","March","April","May","June","July","August","September","October","November","December"];
                    format!("{}, {} {} {}", wd, names[(m-1) as usize], d, y)
                }
                // build HTML
                let mut body = String::new();
                body.push_str(&format!("<h1>Shifts from {} to {}</h1>", esc(&start), esc(&end)));
                for (day, items) in by_day.iter() {
                    body.push_str(&format!("<section class=\"day\"><h2>{}</h2>", esc(&human_date(day))));
                    for (hh, loc, names) in items.iter() {
                        body.push_str(&format!(
                            "<div class=\"card\"><div class=\"hdr\"><span class=\"time\">{}</span><span class=\"loc\">{}</span></div><div class=\"names\">{}</div></div>",
                            esc(hh), esc(loc), esc(&names.join(", "))
                        ));
                    }
                    body.push_str("</section>");
                }
                let css = r#"
                    :root { --ink:#0f172a; --muted:#475569; --accent:#2563eb; }
                    *{ box-sizing: border-box; }
                    body{ font: 14px/1.4 ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Noto Sans, Ubuntu, Cantarell, Helvetica Neue, Arial, "Apple Color Emoji", "Segoe UI Emoji"; color: var(--ink); margin: 24px; }
                    h1{ font-size:20px; margin:0 0 12px; }
                    h2{ font-size:16px; margin:16px 0 8px; color: var(--accent); border-bottom:1px solid #e2e8f0; padding-bottom:4px; }
                    .day{ break-inside: avoid; page-break-inside: avoid; margin-bottom: 10px; }
                    .card{ border:1px solid #e2e8f0; border-radius:8px; padding:8px 10px; margin:6px 0; }
                    .hdr{ display:flex; align-items:center; gap:10px; margin-bottom:4px; }
                    .time{ font-weight:600; padding:2px 6px; border-radius:999px; border:1px solid #cbd5e1; }
                    .loc{ color: var(--muted); }
                    .names{ font-size:13px; }
                    @page { margin: 18mm; }
                    @media print { body{ margin:0; } }
                "#;
                let html = format!(
                    "<!doctype html><html><head><meta charset=\"utf-8\"><title>Shifts</title><style>{}</style></head><body>{}</body></html>",
                    css, body
                );
                if let Some(w) = window() {
                    if let Ok(win_opt) = w.open_with_url_and_target("about:blank", "_blank") {
                        if let Some(win) = win_opt {
                            if let Some(doc) = win.document() {
                                if let Some(el) = doc.document_element() {
                                    el.set_inner_html(&html);
                                }
                                let _ = win.focus();
                                let win_clone = win.clone();
                                let cb = Closure::wrap(Box::new(move || { let _ = win_clone.print(); }) as Box<dyn FnMut()>);
                                let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 250);
                                cb.forget();
                            }
                        }
                    }
                }
            }
            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            {
                // TODO: Implement native PDF export (e.g., using a PDF crate). For now, no-op.
            }
            export_open.set(false);
        }
    };

    // UI rendering
    let (mstart_y, mstart_m, m_last) = month_start_end(year(), month());
    let month_label = {
        let y = year();
        let m = month();
        format!("{} {}", crate::i18n::t(&format!("months.long.{}", m)), y)
    };

    let month_start = (mstart_y, mstart_m, 1u32);
    let month_end = (mstart_y, mstart_m, m_last);
    // Precompute week start and leading blanks count
    #[cfg(not(target_arch = "wasm32"))]
    let week_start: String = {
        #[cfg(feature = "native-db")]
        { dao::get_configuration().ok().map(|c| c.week_start).unwrap_or_else(|| "monday".into()) }
        #[cfg(not(feature = "native-db"))]
        { "monday".into() }
    };
    #[cfg(target_arch = "wasm32")]
    let week_start: String = { wasm_backend::get_configuration().map(|c| c.week_start).unwrap_or_else(|| "monday".into()) };
    let blanks_count: usize = {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let wd = NaiveDate::from_ymd_opt(month_start.0, month_start.1, 1).unwrap().weekday().num_days_from_monday() as usize;
            match week_start.as_str() { "sunday" => (wd + 1) % 7, _ => wd }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let d = js::Date::new_with_year_month_day(month_start.0 as u32, (month_start.1 as i32) - 1, 1);
            let w = d.get_day() as usize;
            match week_start.as_str() { "sunday" => w, _ => (w + 6) % 7 }
        }
    };
    let month_items = list.read().clone();
    let filtered_items: Vec<ShiftItem> = month_items.clone();

    rsx! {
        div { class: "min-h-[70vh] flex items-start justify-center",
            div { class: "w-full max-w-5xl mx-auto space-y-5",
                div { class: "flex items-center justify-between gap-2",
                    a { href: "/", class: "inline-flex items-center gap-2 h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                        span { "←" } span { class: "hidden sm:inline", {t("nav.home")} }
                    }
                    div { class: "flex items-center gap-2",
                        button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: prev_month, {t("common.prev")} }
                        button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: this_month, {t("common.today")} }
                        button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: next_month, {t("common.next")} }
                    }
                }
                div { class: "rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-4 sm:p-5 space-y-4",
                    div { class: "flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2",
                        h1 { class: "text-xl sm:text-2xl font-semibold", {t("nav.shifts")}, " — ", {month_label.clone()} }
                        div { class: "flex items-center gap-2 w-full sm:w-auto",
                            // Selection controls appear to the left when in agenda view
                            { (view() == "agenda").then(|| rsx!(
                                div { class: "flex items-center gap-2",
                                    button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: { let mut select_mode = select_mode.clone(); move |_| select_mode.set(!select_mode()) }, { if select_mode() { t("common.done") } else { t("common.select") } } }
                                    { select_mode().then(|| rsx!(
                                        div { class: "flex items-center gap-1",
                                            input { r#type: "checkbox",
                                                checked: { selected_ids.read().len() == filtered_items.len() && !filtered_items.is_empty() },
                                                disabled: filtered_items.is_empty(),
                                                onchange: {
                                                    let mut selected_ids = selected_ids.clone();
                                                    let ids: Vec<i64> = filtered_items.iter().map(|it| it.id).collect();
                                                    move |_| {
                                                        let mut set = selected_ids.write();
                                                        if set.len() == ids.len() && !ids.is_empty() { set.clear(); }
                                                        else { set.clear(); for id in &ids { set.insert(*id); } }
                                                    }
                                                }
                                            }
                                            span { class: "text-sm text-slate-600 dark:text-slate-300", {t("common.all")} }
                                        }
                                    )) }
                                    { (!selected_ids.read().is_empty()).then(|| rsx!( button { class: "h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white", onclick: bulk_delete, { format!("{} ({})", t("common.delete_selected"), selected_ids.read().len()) } } )) }
                                }
                            )) }
                            {
                                if is_small_screen() {
                                    rsx!( span { class: "text-xs px-2 py-1 rounded border border-slate-300 dark:border-slate-600 bg-slate-50 dark:bg-slate-900/40 text-slate-600 dark:text-slate-300 ml-auto", {t("shifts.agenda")} } )
                                } else {
                                    rsx!( select { class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-2 text-sm ml-auto", value: view(), onchange: {
                                            let mut view = view.clone();
                                            let mut select_mode = select_mode.clone();
                                            let mut selected_ids = selected_ids.clone();
                                            move |e| {
                                                let v = e.value();
                                                view.set(v.clone());
                                                if v == "month" { select_mode.set(false); selected_ids.write().clear(); }
                                            }
                                        },
                                        option { value: "month", {t("shifts.month")} }
                                        option { value: "agenda", {t("shifts.agenda")} }
                                    } )
                                }
                            }
                        }
                    }
                    div { class: "flex flex-col sm:flex-row items-center justify-between gap-2",
                        div { class: "flex items-center gap-2",
                            button { class: "h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium", onclick: move |_| manual_open.set(true), {t("shifts.new")} }
                            button { class: format!("h-9 px-3 rounded-md {} text-white text-sm font-medium inline-flex items-center gap-2", if generating() { "bg-slate-400 cursor-not-allowed" } else { "bg-emerald-600 hover:bg-emerald-500" }), disabled: generating(), onclick: move |_| auto_open.set(true),
                                span { { if generating() { "⏳" } else { "⚙️" } } } span { {t("shifts.auto_generate")} }
                            }
                            button { class: "h-9 px-3 rounded-md bg-purple-600 hover:bg-purple-500 text-white text-sm font-medium", onclick: move |_| export_open.set(true), {t("shifts.export")} }
                        }
                        div { class: "text-sm text-slate-600 dark:text-slate-300", {format!("{}–{}", fmt_date_ymd(&month_start), fmt_date_ymd(&month_end))} }
                    }
                    
                    {
                    if view() == "agenda" { rsx!(
                        ul { class: "divide-y divide-slate-200 dark:divide-slate-700",
                            for item in filtered_items.clone() {
                                li { class: "py-3 flex items-start justify-between gap-3",
                                    div { class: "min-w-0 w-full cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-700/30 rounded-md px-3 -mx-3 py-2", onclick: { let mut edit_form = edit_form.clone(); let mut edit_open = edit_open.clone(); let it = item.clone(); move |_| { let mut w = edit_form.write(); w.shift_id = it.id; w.selected_pids = it.publishers.clone(); w.add_pid.clear(); w.loc = it.location.clone(); w.start_dt = format!("{}T{}", it.date.clone(), it.start_hour.clone()); w.end_dt = format!("{}T{}", it.date.clone(), it.end_hour.clone()); edit_open.set(true); } },
                                        div { class: "text-sm text-slate-500 flex items-center gap-2",
                                            {
                                                // Show date and weekday for clarity
                                                let parts: Vec<&str> = item.date.split('-').collect();
                                                if parts.len()==3 {
                                                    let y = parts[0].parse::<i32>().unwrap_or(1970);
                                                    let m = parts[1].parse::<u32>().unwrap_or(1);
                                                    let d = parts[2].parse::<u32>().unwrap_or(1);
                                                    let wd = weekday_name_for_date(y,m,d);
                                                    format!("{} ({})", item.date.clone(), wd)
                                                } else { item.date.clone() }
                                            }
                                            { select_mode().then(|| rsx!( input { r#type: "checkbox", checked: selected_ids.read().contains(&item.id), onclick: move |e| e.stop_propagation(), onchange: { let mut selected_ids = selected_ids.clone(); let id = item.id; move |_| { let mut set = selected_ids.write(); if set.contains(&id) { set.remove(&id); } else { set.insert(id); } } } } )) }
                                        }
                                        div { class: "font-medium",
                                            span { {item.title.clone()} }
                                        }
                                        {
                                            // render assigned publisher names instead of count
                                            let mut names: Vec<String> = item.publishers.iter().filter_map(|pid| publishers_all.read().iter().find(|p| p.id == *pid).map(|p| p.label.clone())).collect();
                                            names.sort();
                                            rsx!( div { class: "text-xs text-slate-600 dark:text-slate-300", { if names.is_empty() { t("shifts.no_publishers_assigned") } else { names.join(", ") } } } )
                                        }
                                    }
                                }
                            }
                            { (filtered_items.is_empty()).then(|| rsx!( li { class: "py-3 text-sm text-slate-500", {t("shifts.none_in_range")} } )) }
                        }
                    ) } else { rsx!(
                        div { class: "grid grid-cols-7 gap-2",
                            for wd in weekdays_for_locale() { div { class: "text-xs font-semibold text-slate-500 text-center", {wd} } }
                            for _i in 0..blanks_count { div { class: "min-h-24 rounded-md bg-transparent" } }
                            for day in 1..=month_end.2 {
                                div { class: "rounded-md border border-slate-200 dark:border-slate-700 p-2 space-y-1",
                                    div { class: "text-xs text-slate-500 mb-1", {format!("{}", day)} }
                                    for it in filtered_items.iter().filter(|it| it.date == format!("{:04}-{:02}-{:02}", month_start.0, month_start.1, day)) {
                                        {
                                            // color dot per location (better contrast than tinted background)
                                            let hue = { let mut h: u32 = 0; for b in it.location.as_bytes() { h = h.wrapping_mul(16777619) ^ (*b as u32); } h % 360 };
                                            let dot_style = format!("background-color: hsl({hue}, 70%, 45%); width:8px; height:8px; border-radius:9999px; display:inline-block;");
                                            let mut names: Vec<String> = it.publishers.iter().filter_map(|pid| publishers_all.read().iter().find(|p| p.id == *pid).map(|p| p.label.clone())).collect();
                                            names.sort();
                                            rsx!( div { class: "text-[12px] flex flex-col gap-1 cursor-pointer border rounded p-2",
                                                onclick: { let mut edit_form = edit_form.clone(); let mut edit_open = edit_open.clone(); let it2 = it.clone(); move |_| { let mut w = edit_form.write(); w.shift_id = it2.id; w.selected_pids = it2.publishers.clone(); w.add_pid.clear(); w.loc = it2.location.clone(); w.start_dt = format!("{}T{}", it2.date.clone(), it2.start_hour.clone()); w.end_dt = format!("{}T{}", it2.date.clone(), it2.end_hour.clone()); edit_open.set(true); } },
                                                div { class: "flex items-center gap-2 flex-wrap",
                                                    span { style: dot_style }
                                                    span { class: "font-semibold text-slate-900 dark:text-slate-100", {it.location.clone()} }
                                                    span { class: "text-xs text-slate-600 dark:text-slate-300", {format!("{}–{}", it.start_hour.clone(), it.end_hour.clone())} }
                                                }
                                                div { class: "text-[12px] text-slate-800 dark:text-slate-200 whitespace-normal break-words",
                                                    { if names.is_empty() { t("shifts.no_publishers_assigned") } else { names.join(", ") } }
                                                }
                                            })
                                        }
                                    }
                                }
                            }
                        }
                    ) }
                    }
                }
            }
        }

        // Manual create modal
        { manual_open().then(|| rsx!(
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold", {t("shifts.new_title")} }
                    div { class: "grid grid-cols-1 gap-3",
                        // location with suggestions
                        div { class: "space-y-1",
                            label { class: "text-xs text-slate-600 dark:text-slate-300", {t("schedules.location")} }
                            input { r#type: "text", list: "locs", placeholder: t("schedules.location_placeholder"), class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm w-full", value: manual_form.read().loc.clone(), oninput: move |e| manual_form.write().loc = e.value() }
                            datalist { id: "locs",
                                for v in loc_suggestions.read().iter() { option { value: "{v}" } }
                            }
                        }
                        // start/end datetime
                        div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                            div { class: "space-y-1",
                                label { class: "text-xs text-slate-600 dark:text-slate-300", {t("shifts.start_datetime")} }
                                input { r#type: "datetime-local", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm w-full", value: manual_form.read().start_dt.clone(), oninput: move |e| manual_form.write().start_dt = e.value() }
                            }
                            div { class: "space-y-1",
                                label { class: "text-xs text-slate-600 dark:text-slate-300", {t("shifts.end_datetime")} }
                                input { r#type: "datetime-local", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm w-full", value: manual_form.read().end_dt.clone(), oninput: move |e| manual_form.write().end_dt = e.value() }
                            }
                        }
                        // add publishers
                        div { class: "space-y-2",
                            div { class: "flex items-center gap-2",
                                select { class: "h-9 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-2 text-sm w-full", value: manual_form.read().add_pid.clone(), onchange: move |e| manual_form.write().add_pid = e.value(),
                                    option { value: "", {t("common.select_publisher")} }
                                    {
                                        let selected = manual_form.read().selected_pids.clone();
                                        rsx!( for p in publishers_all.read().iter().filter(|p| !selected.contains(&p.id)) { option { value: "{p.id}", "{p.label}" } } )
                                    }
                                }
                                button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: move |_| {
                                    let add = manual_form.read().add_pid.clone();
                                    if let Ok(pid) = add.parse::<i64>() {
                                        let mut w = manual_form.write();
                                        if !w.selected_pids.contains(&pid) { w.selected_pids.push(pid); }
                                        w.add_pid.clear();
                                    }
                                }, {t("common.add")} }
                            }
                            // current selected publishers
                            div { class: "flex flex-wrap gap-2",
                                for pid in manual_form.read().selected_pids.clone() {
                                    {
                                        let pub_name = publishers_all.read().iter().find(|p| p.id == pid).map(|p| p.label.clone()).unwrap_or_else(|| format!("#{}", pid));
                                        rsx!( button { class: "text-xs px-2 py-1 rounded border border-slate-300 dark:border-slate-600", onclick: move |_| { manual_form.write().selected_pids.retain(|x| *x != pid); }, "", {pub_name}, " ✕" } )
                                    }
                                }
                            }
                            // warnings (simple absence/same-day checks)
                            {
                                let mut warns: Vec<String> = Vec::new();
                                if !manual_form.read().start_dt.is_empty() {
                                    let date_s = manual_form.read().start_dt.split('T').next().unwrap_or("").to_string();
                                    for pid in manual_form.read().selected_pids.iter() {
                                        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                                        {
                                            if let Ok(d) = chrono::NaiveDate::parse_from_str(&date_s, "%Y-%m-%d") {
                                                let name = publishers_all.read().iter().find(|pp| pp.id == *pid).map(|pp| pp.label.clone()).unwrap_or_else(|| format!("#{pid}"));
                                                if dao::is_absent_on(*pid, d).unwrap_or(false) { warns.push(format!("{} {}", name, t("shifts.warn_absent_generic"))); }
                                                let day_start = chrono::NaiveDateTime::new(d, chrono::NaiveTime::from_hms_opt(0,0,0).unwrap());
                                                let day_end = chrono::NaiveDateTime::new(d, chrono::NaiveTime::from_hms_opt(23,59,59).unwrap());
                                                let existing = dao::list_shifts_between(day_start, day_end).unwrap_or_default();
                                                if existing.iter().any(|sh| sh.publishers.contains(pid)) { warns.push(format!("{} {}", name, t("shifts.warn_already_has_shift"))); }
                                            }
                                        }
                                        #[cfg(target_arch = "wasm32")]
                                        {
                                            let name = publishers_all.read().iter().find(|pp| pp.id == *pid).map(|pp| pp.label.clone()).unwrap_or_else(|| format!("#{pid}"));
                                            if wasm_backend::is_absent_on(*pid, &date_s) { warns.push(format!("{} {}", name, t("shifts.warn_absent_generic"))); }
                                            let existing = wasm_backend::list_shifts_between(&format!("{} 00:00:00", date_s), &format!("{} 23:59:59", date_s));
                                            if existing.iter().any(|sh| sh.publishers.contains(pid)) { warns.push(format!("{} {}", name, t("shifts.warn_already_has_shift"))); }
                                        }
                                    }
                                }
                                (!warns.is_empty()).then(|| rsx!( div { class: "rounded-md bg-amber-50 dark:bg-amber-900/30 border border-amber-200 dark:border-amber-800 p-2 text-amber-800 dark:text-amber-200 text-xs space-y-1",
                                    for w in warns { div { {w} } }
                                } ))
                            }
                        }
                    }
                    div { class: "flex items-center justify-end gap-2",
                        button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: move |_| manual_open.set(false), {t("common.cancel")} }
                        button { class: "h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white", onclick: manual_submit, {t("common.create")} }
                    }
                }
            }
        )) }
        // Export modal
        { export_open().then(|| rsx!(
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold", {t("shifts.export_title")} }
                    p { class: "text-sm text-slate-600 dark:text-slate-300", {t("shifts.export_desc")} }
                    div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                        input { r#type: "date", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm", value: export_form.read().start.clone(), oninput: move |e| export_form.write().start = e.value() }
                        input { r#type: "date", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm", value: export_form.read().end.clone(), oninput: move |e| export_form.write().end = e.value() }
                    }
                    div { class: "flex items-center justify-end gap-2",
                        button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: move |_| export_open.set(false), {t("common.cancel")} }
                        button { class: "h-9 px-3 rounded-md bg-purple-600 hover:bg-purple-500 text-white", onclick: do_export, {t("shifts.export_pdf")} }
                    }
                }
            }
        )) }

        // Edit modal
        { edit_open().then(|| rsx!(
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold", {t("shifts.edit_title")} }
                    div { class: "space-y-2",
                        // location and datetime fields
                        div { class: "space-y-1",
                            label { class: "text-xs text-slate-600 dark:text-slate-300", {t("schedules.location")} }
                            input { r#type: "text", list: "locs", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm w-full", value: edit_form.read().loc.clone(), oninput: move |e| { let mut w = edit_form.write(); w.loc = e.value(); } }
                            datalist { id: "locs",
                                for v in loc_suggestions.read().iter() { option { value: "{v}" } }
                            }
                        }
                        div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                            div { class: "space-y-1",
                                label { class: "text-xs text-slate-600 dark:text-slate-300", {t("shifts.start_datetime")} }
                                input { r#type: "datetime-local", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm w-full", value: edit_form.read().start_dt.clone(), oninput: move |e| { let mut w = edit_form.write(); w.start_dt = e.value(); } }
                            }
                            div { class: "space-y-1",
                                label { class: "text-xs text-slate-600 dark:text-slate-300", {t("shifts.end_datetime")} }
                                input { r#type: "datetime-local", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm w-full", value: edit_form.read().end_dt.clone(), oninput: move |e| { let mut w = edit_form.write(); w.end_dt = e.value(); } }
                            }
                        }
                        div { class: "flex items-center gap-2",
                            select { class: "h-9 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-2 text-sm w-full", value: edit_form.read().add_pid.clone(), onchange: move |e| { let mut w = edit_form.write(); w.add_pid = e.value(); },
                                option { value: "", {t("common.select_publisher")} }
                                for p in publishers_all.read().iter() { option { value: "{p.id}", "{p.label}" } }
                            }
                            button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: move |_| {
                                let add = edit_form.read().add_pid.clone();
                                if let Ok(pid) = add.parse::<i64>() {
                                    let mut w = edit_form.write();
                                    if !w.selected_pids.contains(&pid) { w.selected_pids.push(pid); }
                                    w.add_pid.clear();
                                }
                            }, {t("common.add")} }
                        }
                        div { class: "flex flex-wrap gap-2",
                            for pid in edit_form.read().selected_pids.clone() {
                                {
                                    let pub_name = publishers_all.read().iter().find(|p| p.id == pid).map(|p| p.label.clone()).unwrap_or_else(|| format!("#{}", pid));
                                    rsx!( button { class: "text-xs px-2 py-1 rounded border border-slate-300 dark:border-slate-600", onclick: move |_| { let mut w = edit_form.write(); w.selected_pids.retain(|x| *x != pid); }, "", {pub_name}, " ✕" } )
                                }
                            }
                        }
                        {
                            let mut warns: Vec<String> = Vec::new();
                            if !edit_form.read().start_dt.is_empty() {
                                let date_s = edit_form.read().start_dt.split('T').next().unwrap_or("").to_string();
                                for pid in edit_form.read().selected_pids.iter() {
                                    #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                                    {
                                        if let Ok(d) = chrono::NaiveDate::parse_from_str(&date_s, "%Y-%m-%d") {
                                            let name = publishers_all.read().iter().find(|pp| pp.id == *pid).map(|pp| pp.label.clone()).unwrap_or_else(|| format!("#{pid}"));
                                            if dao::is_absent_on(*pid, d).unwrap_or(false) { warns.push(format!("{} {}", name, t("shifts.warn_absent_generic"))); }
                                            let day_start = chrono::NaiveDateTime::new(d, chrono::NaiveTime::from_hms_opt(0,0,0).unwrap());
                                            let day_end = chrono::NaiveDateTime::new(d, chrono::NaiveTime::from_hms_opt(23,59,59).unwrap());
                                            let existing = dao::list_shifts_between(day_start, day_end).unwrap_or_default();
                                            if existing.iter().any(|sh| sh.id != edit_form.read().shift_id && sh.publishers.contains(pid)) { warns.push(format!("{} {}", name, t("shifts.warn_already_has_shift"))); }
                                        }
                                    }
                                    #[cfg(target_arch = "wasm32")]
                                    {
                                        let name = publishers_all.read().iter().find(|pp| pp.id == *pid).map(|pp| pp.label.clone()).unwrap_or_else(|| format!("#{pid}"));
                                        if wasm_backend::is_absent_on(*pid, &date_s) { warns.push(format!("{} {}", name, t("shifts.warn_absent_generic"))); }
                                        let existing = wasm_backend::list_shifts_between(&format!("{} 00:00:00", date_s), &format!("{} 23:59:59", date_s));
                                        if existing.iter().any(|sh| sh.id != edit_form.read().shift_id && sh.publishers.contains(pid)) { warns.push(format!("{} {}", name, t("shifts.warn_already_has_shift"))); }
                                    }
                                }
                            }
                            (!warns.is_empty()).then(|| rsx!( div { class: "rounded-md bg-amber-50 dark:bg-amber-900/30 border border-amber-200 dark:border-amber-800 p-2 text-amber-800 dark:text-amber-200 text-xs space-y-1",
                                for w in warns { div { {w} } }
                            } ))
                        }
                    }
                    div { class: "flex items-center justify-end gap-2",
                        // delete from inside edit
                        button { class: "h-9 px-3 rounded-md border border-red-300 text-red-700", onclick: { let mut confirm_delete_id = confirm_delete_id.clone(); let id = edit_form.read().shift_id; move |_| { confirm_delete_id.set(Some(id)); } }, {t("common.delete")} }
                        button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: move |_| { edit_open.set(false) }, {t("common.cancel")} }
                        button { class: "h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white", onclick: edit_submit, {t("common.save")} }
                    }
                }
            }
        )) }
        // Confirm delete modal
        { confirm_delete_id().map(|id| rsx!(
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-sm rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold", {t("shifts.confirm_delete_title")} }
                    p { class: "text-sm text-slate-600 dark:text-slate-300", {t("shifts.confirm_delete_message")} }
                    div { class: "flex items-center justify-end gap-2",
                        button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: { let mut confirm_delete_id = confirm_delete_id.clone(); move |_| confirm_delete_id.set(None) }, {t("common.cancel")} }
                        button { class: "h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white", onclick: { let mut delete_one = delete_one.clone(); let mut confirm_delete_id = confirm_delete_id.clone(); move |_| { if let Some(x) = confirm_delete_id() { delete_one(x); } confirm_delete_id.set(None); } }, {t("common.delete")} }
                    }
                }
            }
        )) }
        // Auto-generate modal
        { auto_open().then(|| rsx!(
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold", {t("shifts.auto_title")} }
                    p { class: "text-sm text-slate-600 dark:text-slate-300", {t("shifts.auto_desc")} }
                    div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                        input { r#type: "date", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm", value: auto_form.read().start.clone(), oninput: move |e| auto_form.write().start = e.value() }
                        input { r#type: "date", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm", value: auto_form.read().end.clone(), oninput: move |e| auto_form.write().end = e.value() }
                    }
                    div { class: "flex items-center justify-end gap-2",
                        button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: move |_| auto_open.set(false), {t("common.cancel")} }
                        button { class: { format!("h-9 px-3 rounded-md {} text-white", if generating() { "bg-slate-400" } else { "bg-emerald-600 hover:bg-emerald-500" }) }, disabled: generating(), onclick: do_autogen, {t("shifts.generate")} }
                    }
                }
            }
        )) }
    }
}

