use dioxus::prelude::*;
use crate::i18n::t;

#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))] use crate::db::dao as backend;
#[cfg(target_arch = "wasm32")] use crate::db::wasm_store as backend;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
#[allow(dead_code)]
mod backend { pub fn configuration_is_set() -> bool { false } }

#[component]
#[allow(unused_mut)]
pub fn Home() -> Element {
    // Use global configured signal provided by App
    let configured: Signal<bool> = use_context();

    if !configured() {
        return rsx! { super::landpage::Landpage {} };
    }

    // Stats: compute on mount
    let mut total_publishers = use_signal(|| 0i64);
    let mut total_managers = use_signal(|| 0i64);
    let mut weakest_schedules_publishers = use_signal(|| Vec::<(String, i64)>::new());
    let mut weakest_schedules_managers = use_signal(|| Vec::<(String, i64)>::new());
    let mut top5_assigned = use_signal(|| Vec::<(String, i64)>::new());
    let mut bottom5_assigned = use_signal(|| Vec::<(String, i64)>::new());

    use_effect(move || {
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            if let Ok(pubs) = backend::list_publishers() {
                total_publishers.set(pubs.len() as i64);
                total_managers.set(pubs.iter().filter(|p| p.is_shift_manager).count() as i64);
            }
            if let Ok(schedules) = backend::list_schedules() {
                // availability per schedule
                let mut avail_counts: Vec<(String, i64, i64)> = Vec::new(); // (label, total, managers)
                for s in schedules.iter() {
                    let ids = backend::list_publishers_for_schedule(s.id).unwrap_or_default();
                    let label = format!("{} • {}–{} ({})", s.location, s.start_hour, s.end_hour, s.weekday);
                    // manager-capable among these ids
                    let managers = backend::list_publishers().unwrap_or_default().into_iter().filter(|p| ids.contains(&p.id) && p.is_shift_manager && p.gender=="Male").count() as i64;
                    avail_counts.push((label, ids.len() as i64, managers));
                }
                let mut by_pub = avail_counts.iter().map(|(l, c, _)| (l.clone(), *c)).collect::<Vec<_>>();
                by_pub.sort_by_key(|(_, c)| *c);
                weakest_schedules_publishers.set(by_pub.into_iter().take(3).collect());
                let mut by_mgr = avail_counts.iter().map(|(l, _, m)| (l.clone(), *m)).collect::<Vec<_>>();
                by_mgr.sort_by_key(|(_, c)| *c);
                weakest_schedules_managers.set(by_mgr.into_iter().take(3).collect());
            }
            // top/bottom assigned in last 60 days
            use chrono::{Local, Duration, NaiveDate, NaiveDateTime, NaiveTime};
            let end = Local::now().naive_local().date();
            let start = end - Duration::days(60);
            let hist = backend::list_shifts_between(
                NaiveDateTime::new(start, NaiveTime::from_hms_opt(0,0,0).unwrap()),
                NaiveDateTime::new(end, NaiveTime::from_hms_opt(23,59,59).unwrap()),
            ).unwrap_or_default();
            use std::collections::HashMap;
            let mut counts: HashMap<i64, i64> = HashMap::new();
            for sh in hist { for pid in sh.publishers { *counts.entry(pid).or_insert(0) += 1; } }
            let name_order = backend::get_configuration().ok().map(|c| c.name_order).unwrap_or_else(|| "first_last".into());
            let name_map = backend::list_publishers().unwrap_or_default().into_iter().map(|p| {
                let name = if name_order == "last_first" { format!("{} {}", p.last_name, p.first_name) } else { format!("{} {}", p.first_name, p.last_name) };
                (p.id, name)
            }).collect::<HashMap<_,_>>();
            let mut all: Vec<(String, i64)> = counts.into_iter().map(|(pid, c)| (name_map.get(&pid).cloned().unwrap_or_else(|| format!("#{pid}")), c)).collect();
            all.sort_by(|a,b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            top5_assigned.set(all.iter().take(5).cloned().collect());
            let mut asc = all.clone(); asc.sort_by(|a,b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0))); bottom5_assigned.set(asc.into_iter().take(5).collect());
        }
        #[cfg(target_arch = "wasm32")]
        {
            let pubs = backend::list_publishers();
            total_publishers.set(pubs.len() as i64);
            total_managers.set(pubs.iter().filter(|p| p.is_shift_manager).count() as i64);
            let schedules = backend::list_schedules();
            let mut avail_counts: Vec<(String, i64, i64)> = Vec::new();
            for s in schedules.iter() {
                let ids = backend::list_publishers_for_schedule(s.id);
                let label = format!("{} • {}–{} ({})", s.location, s.start_hour, s.end_hour, s.weekday);
                let managers = pubs.iter().filter(|p| ids.contains(&p.id) && p.is_shift_manager && p.gender=="Male").count() as i64;
                avail_counts.push((label, ids.len() as i64, managers));
            }
            let mut by_pub = avail_counts.iter().map(|(l, c, _)| (l.clone(), *c)).collect::<Vec<_>>(); by_pub.sort_by_key(|(_, c)| *c); weakest_schedules_publishers.set(by_pub.into_iter().take(3).collect());
            let mut by_mgr = avail_counts.iter().map(|(l, _, m)| (l.clone(), *m)).collect::<Vec<_>>(); by_mgr.sort_by_key(|(_, c)| *c); weakest_schedules_managers.set(by_mgr.into_iter().take(3).collect());
            // last 60 days
            let now = js_sys::Date::new_0();
            let past = js_sys::Date::new_0(); past.set_time(now.get_time() - 60.0*24.0*3600.0*1000.0);
            let hist = backend::list_shifts_between(
                &format!("{:04}-{:02}-{:02} 00:00:00", past.get_full_year() as i32, past.get_month() as u32 + 1, past.get_date() as u32),
                &format!("{:04}-{:02}-{:02} 23:59:59", now.get_full_year() as i32, now.get_month() as u32 + 1, now.get_date() as u32),
            );
            use std::collections::HashMap;
            let mut counts: HashMap<i64, i64> = HashMap::new();
            for sh in hist { for pid in sh.publishers { *counts.entry(pid).or_insert(0) += 1; } }
            let name_order = backend::get_name_order();
            let mut all: Vec<(String, i64)> = counts.into_iter().map(|(pid, c)| {
                let name = pubs.iter().find(|p| p.id==pid).map(|p| if name_order == "last_first" { format!("{} {}", p.last_name, p.first_name) } else { format!("{} {}", p.first_name, p.last_name) }).unwrap_or_else(|| format!("#{pid}"));
                (name, c)
            }).collect();
            all.sort_by(|a,b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            top5_assigned.set(all.iter().take(5).cloned().collect());
            let mut asc = all.clone(); asc.sort_by(|a,b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0))); bottom5_assigned.set(asc.into_iter().take(5).collect());
        }
    });

    rsx! {
        div { class: "space-y-6",
            h1 { class: "text-2xl sm:text-3xl font-semibold", {t("home.welcome")} }
            p { class: "text-slate-600 dark:text-slate-300", {t("home.choose")} }
            div { class: "grid gap-3 grid-cols-2 md:grid-cols-3",
                a { href: "/publishers", class: "group h-24 sm:h-28 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm flex flex-col items-center justify-center gap-1.5 hover:border-blue-400 hover:shadow transition",
                    span { class: "text-2xl sm:text-3xl", "👥" }
                    span { class: "text-xs sm:text-sm font-medium text-slate-700 dark:text-slate-200 group-hover:text-blue-600", {t("menu.publishers")} }
                }
                a { href: "/schedules", class: "group h-24 sm:h-28 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm flex flex-col items-center justify-center gap-1.5 hover:border-blue-400 hover:shadow transition",
                    span { class: "text-2xl sm:text-3xl", "📅" }
                    span { class: "text-xs sm:text-sm font-medium text-slate-700 dark:text-slate-200 group-hover:text-blue-600", {t("menu.schedules")} }
                }
                a { href: "/shifts", class: "group h-24 sm:h-28 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm flex flex-col items-center justify-center gap-1.5 hover:border-blue-400 hover:shadow transition",
                    span { class: "text-2xl sm:text-3xl", "🗓️" }
                    span { class: "text-xs sm:text-sm font-medium text-slate-700 dark:text-slate-200 group-hover:text-blue-600", {t("menu.shifts")} }
                }
                a { href: "/absences", class: "group h-24 sm:h-28 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm flex flex-col items-center justify-center gap-1.5 hover:border-blue-400 hover:shadow transition",
                    span { class: "text-2xl sm:text-3xl", "🚫" }
                    span { class: "text-xs sm:text-sm font-medium text-slate-700 dark:text-slate-200 group-hover:text-blue-600", {t("menu.absences")} }
                }
                a { href: "/configuration", class: "group h-24 sm:h-28 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm flex flex-col items-center justify-center gap-1.5 hover:border-blue-400 hover:shadow transition",
                    span { class: "text-2xl sm:text-3xl", "⚙️" }
                    span { class: "text-xs sm:text-sm font-medium text-slate-700 dark:text-slate-200 group-hover:text-blue-600", {t("menu.configuration")} }
                }
            }
            hr {}
            // Today's shifts section
            {
                // compute YYYY-MM-DD for today for header (formatted via i18n)
                #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                let today_ymd = {
                    use chrono::Datelike;
                    let now = chrono::Local::now().naive_local().date();
                    format!("{:04}-{:02}-{:02}", now.year(), now.month(), now.day())
                };
                #[cfg(target_arch = "wasm32")]
                let today_ymd = {
                    let now = js_sys::Date::new_0();
                    format!(
                        "{:04}-{:02}-{:02}",
                        now.get_full_year() as i32,
                        now.get_month() as u32 + 1,
                        now.get_date() as u32
                    )
                };
                #[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
                let today_ymd = "1970-01-01".to_string();

                // Gather today's shifts and display inline (no extra state needed)
                #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                let today_list: Vec<(String, String, Vec<String>)> = {
                    use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Timelike};
                    let name_order = backend::get_configuration().ok().map(|c| c.name_order).unwrap_or_else(|| "first_last".into());
                    let pubs = backend::list_publishers().unwrap_or_default();
                    let name_map = pubs.into_iter()
                        .map(|p| {
                            let name = if name_order == "last_first" { format!("{} {}", p.last_name, p.first_name) } else { format!("{} {}", p.first_name, p.last_name) };
                            (p.id, name)
                        })
                        .collect::<std::collections::HashMap<_, _>>();
                    let d = NaiveDate::parse_from_str(&today_ymd, "%Y-%m-%d").unwrap();
                    let start = NaiveDateTime::new(d, NaiveTime::from_hms_opt(0, 0, 0).unwrap());
                    let end = NaiveDateTime::new(d, NaiveTime::from_hms_opt(23, 59, 59).unwrap());
                    let mut v = backend::list_shifts_between(start, end).unwrap_or_default();
                    v.sort_by_key(|s| s.start);
                    v.into_iter()
                        .map(|s| {
                            let time = format!("{:02}:{:02}–{:02}:{:02}", s.start.hour(), s.start.minute(), s.end.hour(), s.end.minute());
                            let mut names = s
                                .publishers
                                .into_iter()
                                .map(|id| name_map.get(&id).cloned().unwrap_or_else(|| format!("#{id}")))
                                .collect::<Vec<_>>();
                            names.sort();
                            (time, s.location, names)
                        })
                        .collect()
                };
                #[cfg(target_arch = "wasm32")]
                let today_list: Vec<(String, String, Vec<String>)> = {
                    let name_order = backend::get_name_order();
                    let pubs = backend::list_publishers();
                    let mut v = backend::list_shifts_between(&format!("{} 00:00:00", today_ymd), &format!("{} 23:59:59", today_ymd));
                    v.sort_by(|a,b| a.start_datetime.cmp(&b.start_datetime));
                    v.into_iter()
                        .map(|s| {
                            let start_part = s.start_datetime.split(' ').nth(1).unwrap_or("");
                            let end_part = s.end_datetime.split(' ').nth(1).unwrap_or("");
                            let sh = if start_part.len() >= 5 { &start_part[..5] } else { start_part };
                            let eh = if end_part.len() >= 5 { &end_part[..5] } else { end_part };
                            let time = format!("{}–{}", sh, eh);
                            let mut names = s
                                .publishers
                                .into_iter()
                                .map(|id| {
                                    pubs.iter()
                                        .find(|p| p.id == id)
                                        .map(|p| if name_order == "last_first" { format!("{} {}", p.last_name, p.first_name) } else { format!("{} {}", p.first_name, p.last_name) })
                                        .unwrap_or_else(|| format!("#{id}"))
                                })
                                .collect::<Vec<_>>();
                            names.sort();
                            (time, s.location, names)
                        })
                        .collect()
                };
                #[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
                let today_list: Vec<(String, String, Vec<String>)> = Vec::new();

                let date_disp = crate::i18n::format_date_ymd(&today_ymd);
                rsx!(
                    div { class: "rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-3",
                        h2 { class: "text-sm font-semibold mb-2", { format!("{} — {}", t("home.today_shifts"), date_disp) } }
                        { if today_list.is_empty() {
                            rsx!( ul { class: "text-sm leading-6 list-disc list-inside text-slate-800 dark:text-slate-200",
                                li { class: "list-none text-slate-500", { t("home.no_shifts_today") } }
                            })
                        } else {
                            rsx!( ul { class: "text-sm leading-6 list-disc list-inside text-slate-800 dark:text-slate-200",
                                for (time, loc, names) in today_list.iter() {
                                    li { { format!("{} • {} — {}", time, loc, names.join(", ")) } }
                                }
                            })
                        } }
                    }
                )
            }
            hr {}
            // Quick stats (below the menu)
            div { class: "grid gap-3 grid-cols-2 md:grid-cols-2",
                div { class: "h-20 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-3 flex flex-col justify-center",
                    span { class: "text-xs text-slate-500", {t("stats.total_publishers")} }
                    span { class: "text-xl font-semibold", { total_publishers().to_string() } }
                }
                div { class: "h-20 rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-3 flex flex-col justify-center",
                    span { class: "text-xs text-slate-500", {t("stats.total_managers")} }
                    span { class: "text-xl font-semibold", { total_managers().to_string() } }
                }
            }
            // Weakest schedules lists (full width)
            div { class: "grid gap-3 grid-cols-1 md:grid-cols-2",
                div { class: "rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-3",
                    h2 { class: "text-xs text-slate-500 pb-2", {t("stats.weakest_publishers")} }
                    ul { class: "text-sm space-y-1",
                        for (label, c) in weakest_schedules_publishers.read().iter() { li { class: "flex items-center justify-between", span { {label.to_string()} } span { class: "text-slate-500", {c.to_string()} } } }
                        { weakest_schedules_publishers.read().is_empty().then(|| rsx!( li { class: "text-slate-500 text-sm", {t("stats.no_data")} } )) }
                    }
                }
                div { class: "rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-3",
                    h2 { class: "text-xs text-slate-500 pb-2", {t("stats.weakest_managers")} }
                    ul { class: "text-sm space-y-1",
                        for (label, c) in weakest_schedules_managers.read().iter() { li { class: "flex items-center justify-between", span { {label.to_string()} } span { class: "text-slate-500", {c.to_string()} } } }
                        { weakest_schedules_managers.read().is_empty().then(|| rsx!( li { class: "text-slate-500 text-sm", {t("stats.no_data")} } )) }
                    }
                }
            }
            div { class: "grid gap-3 grid-cols-1 md:grid-cols-2",
                div { class: "rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-3",
                    h2 { class: "text-xs text-slate-500 pb-2", {t("stats.top5")} }
                    ul { class: "text-sm space-y-1",
                        for (name, c) in top5_assigned.read().iter() { li { class: "flex items-center justify-between", span { {name.clone()} } span { class: "text-slate-500", {c.to_string()} } } }
                        { top5_assigned.read().is_empty().then(|| rsx!( li { class: "text-slate-500 text-sm", {t("stats.no_data_yet")} } )) }
                    }
                }
                div { class: "rounded-lg border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-3",
                    h2 { class: "text-xs text-slate-500 pb-2", {t("stats.least5")} }
                    ul { class: "text-sm space-y-1",
                        for (name, c) in bottom5_assigned.read().iter() { li { class: "flex items-center justify-between", span { {name.clone()} } span { class: "text-slate-500", {c.to_string()} } } }
                        { bottom5_assigned.read().is_empty().then(|| rsx!( li { class: "text-slate-500 text-sm", {t("stats.no_data_yet")} } )) }
                    }
                }
            }
        }
    }
}
