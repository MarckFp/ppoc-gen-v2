use dioxus::prelude::*;
use crate::i18n::t;
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))] use crate::db::dao;
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))] use crate::db::dao::Schedule as NativeSchedule;
#[cfg(target_arch = "wasm32")] use crate::db::wasm_store as wasm_backend;
#[cfg(target_arch = "wasm32")] use wasm_backend::Schedule as WebSchedule;
#[cfg(target_arch = "wasm32")] use web_sys::window;

const PAGE_SIZE: usize = 25;

#[derive(Clone, PartialEq)]
enum ConfirmAction { DeleteOne(i64), DeleteMany(Vec<i64>) }

#[derive(PartialEq, Clone)]
struct ScheduleForm {
    id: Option<i64>,
    location: String,
    start_hour: String,
    end_hour: String,
    weekday: String,
    description: String,
    num_publishers: String,
    num_shift_managers: String,
    num_brothers: String,
    num_sisters: String,
}

#[derive(Clone)]
struct ScheduleListItem {
    id: i64,
    title: String,
    subtitle: String,
}

fn normalize(s: &str) -> String { s.to_lowercase() }

#[cfg(target_arch = "wasm32")]
fn locale_prefix() -> String {
    window()
        .map(|w| w.navigator().language().unwrap_or_else(|| "en".to_string()).to_lowercase())
        .unwrap_or_else(|| "en".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn locale_prefix() -> String {
    std::env::var("LANG").unwrap_or_else(|_| "en".to_string()).to_lowercase()
}

fn weekdays_for_locale() -> Vec<&'static str> {
    let lp = locale_prefix();
    if lp.starts_with("es") {
        vec!["Lunes","Martes","Miércoles","Jueves","Viernes","Sábado","Domingo"]
    } else if lp.starts_with("fr") {
        vec!["Lundi","Mardi","Mercredi","Jeudi","Vendredi","Samedi","Dimanche"]
    } else {
        vec!["Monday","Tuesday","Wednesday","Thursday","Friday","Saturday","Sunday"]
    }
}

#[allow(dead_code)]
fn weekday_order_list(week_start: &str) -> Vec<String> {
    let mut days: Vec<String> = weekdays_for_locale().into_iter().map(|s| s.to_string()).collect();
    if week_start.eq_ignore_ascii_case("sunday") {
        // rotate so Sunday goes first (Sunday is last in weekdays_for_locale())
        if !days.is_empty() { days.rotate_right(1); }
    }
    days
}

#[allow(dead_code)]
fn weekday_rank(name: &str, order: &[String]) -> usize {
    let lower = name.to_lowercase();
    order.iter().position(|d| d.to_lowercase() == lower).unwrap_or(order.len())
}

#[component]
#[allow(unused_mut)]
pub fn Schedules() -> Element {
    let mut list = use_signal(|| Vec::<ScheduleListItem>::new());
    let mut raw = use_signal(|| Vec::<ScheduleListItem>::new());
    let mut query = use_signal(|| String::new());
    let mut loc_suggestions = use_signal(|| Vec::<String>::new());
    let mut modal_open = use_signal(|| false);
    let mut form = use_signal(|| ScheduleForm { id: None, location: String::new(), start_hour: "09:00".into(), end_hour: "12:00".into(), weekday: "Monday".into(), description: String::new(), num_publishers: "4".into(), num_shift_managers: "1".into(), num_brothers: "2".into(), num_sisters: "2".into() });
    let mut error = use_signal(|| Option::<String>::None);
    let mut current_page = use_signal(|| 0usize);
    let mut selected = use_signal(|| Vec::<i64>::new());
    let mut confirm_action = use_signal(|| Option::<ConfirmAction>::None);
    let mut select_mode = use_signal(|| false);

    use_effect(move || {
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        if let Ok(mut items) = dao::list_schedules() {
            let week_start = dao::get_configuration().ok().map(|c| c.week_start).unwrap_or_else(|| "monday".into());
            let order = weekday_order_list(&week_start);
            items.sort_by(|a, b| weekday_rank(&a.weekday, &order).cmp(&weekday_rank(&b.weekday, &order)).then(a.start_hour.cmp(&b.start_hour)));
            let mapped = items.into_iter().map(|s: NativeSchedule| ScheduleListItem {
                id: s.id,
                title: format!("{} • {}–{}", s.location, s.start_hour, s.end_hour),
                subtitle: format!("{}, {} {}, {} {}, {} {}, {} {}", s.weekday, s.num_publishers, t("schedules.pubs_short"), s.num_shift_managers, t("schedules.managers_short"), s.num_brothers, t("schedules.brothers"), s.num_sisters, t("schedules.sisters")),
            }).collect::<Vec<_>>();
            raw.set(mapped.clone());
            // build unique location suggestions from mapped items' titles
            let mut set = std::collections::BTreeSet::<String>::new();
            for it in &mapped { if let Some((loc, _)) = it.title.split_once(" • ") { set.insert(loc.to_string()); } }
            loc_suggestions.set(set.into_iter().collect());
            list.set(mapped);
        }
        #[cfg(target_arch = "wasm32")]
        {
            let mut items = wasm_backend::list_schedules();
            let week_start = wasm_backend::get_configuration().map(|c| c.week_start).unwrap_or_else(|| "monday".into());
            let order = weekday_order_list(&week_start);
            items.sort_by(|a, b| weekday_rank(&a.weekday, &order).cmp(&weekday_rank(&b.weekday, &order)).then(a.start_hour.cmp(&b.start_hour)));
            let mapped = items.into_iter().map(|s: WebSchedule| ScheduleListItem {
                id: s.id,
                title: format!("{} • {}–{}", s.location, s.start_hour, s.end_hour),
                subtitle: format!("{}, {} {}, {} {}, {} {}, {} {}", s.weekday, s.num_publishers, t("schedules.pubs_short"), s.num_shift_managers, t("schedules.managers_short"), s.num_brothers, t("schedules.brothers"), s.num_sisters, t("schedules.sisters")),
            }).collect::<Vec<_>>();
            raw.set(mapped.clone());
            let mut set = std::collections::BTreeSet::<String>::new();
            for it in &mapped { if let Some((loc, _)) = it.title.split_once(" • ") { set.insert(loc.to_string()); } }
            loc_suggestions.set(set.into_iter().collect());
            list.set(mapped);
        }
    });

    // search
    let mut apply_filter = {
        let mut list = list.clone();
        let raw = raw.clone();
        let query = query.clone();
        move || {
            let q = normalize(&query.read());
            if q.is_empty() { list.set(raw.read().clone()); return; }
            let items = raw.read().iter().cloned().filter(|i| normalize(&i.title).contains(&q) || normalize(&i.subtitle).contains(&q)).collect::<Vec<_>>();
            list.set(items);
            current_page.set(0);
        }
    };

    // selection helpers
    let is_selected = {
        let selected = selected.clone();
        move |id: i64| selected.read().contains(&id)
    };
    let mut toggle_selected = {
        let mut selected = selected.clone();
        move |id: i64| {
            let mut v = selected.read().clone();
            if let Some(pos) = v.iter().position(|x| *x == id) { v.remove(pos); } else { v.push(id); }
            selected.set(v);
        }
    };

    let open_create = move |_| {
        error.set(None);
        form.set(ScheduleForm { id: None, location: String::new(), start_hour: "09:00".into(), end_hour: "12:00".into(), weekday: "Monday".into(), description: String::new(), num_publishers: "4".into(), num_shift_managers: "1".into(), num_brothers: "2".into(), num_sisters: "2".into() });
        modal_open.set(true);
    };
    let mut open_edit_id = {
        let mut _form_cl = form.clone();
        let mut _modal_open_cl = modal_open.clone();
        move |_id: i64| {
            error.set(None);
            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
            if let Ok(items) = dao::list_schedules() {
                if let Some(s) = items.into_iter().find(|x| x.id == _id) {
                    _form_cl.set(ScheduleForm { id: Some(s.id), location: s.location, start_hour: s.start_hour, end_hour: s.end_hour, weekday: s.weekday, description: s.description.unwrap_or_default(), num_publishers: s.num_publishers.to_string(), num_shift_managers: s.num_shift_managers.to_string(), num_brothers: s.num_brothers.to_string(), num_sisters: s.num_sisters.to_string() });
                    _modal_open_cl.set(true);
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                if let Some(s) = wasm_backend::list_schedules().into_iter().find(|x| x.id == _id) {
                    _form_cl.set(ScheduleForm { id: Some(s.id), location: s.location, start_hour: s.start_hour, end_hour: s.end_hour, weekday: s.weekday, description: s.description.unwrap_or_default(), num_publishers: s.num_publishers.to_string(), num_shift_managers: s.num_shift_managers.to_string(), num_brothers: s.num_brothers.to_string(), num_sisters: s.num_sisters.to_string() });
                    _modal_open_cl.set(true);
                }
            }
        }
    };

    let on_submit = move |_| {
        error.set(None);
        let f = form.read().clone();
        if f.location.trim().is_empty() { error.set(Some(t("schedules.error_location_required"))); return; }
        let np = f.num_publishers.parse::<i64>().unwrap_or(0);
        let nm = f.num_shift_managers.parse::<i64>().unwrap_or(0);
        let nb = f.num_brothers.parse::<i64>().unwrap_or(0);
        let ns = f.num_sisters.parse::<i64>().unwrap_or(0);
        if nm + nb + ns > np { error.set(Some(t("schedules.error_counts_exceed_total"))); return; }
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            let s = NativeSchedule { id: f.id.unwrap_or_default(), location: f.location, start_hour: f.start_hour, end_hour: f.end_hour, weekday: f.weekday, description: if f.description.trim().is_empty() { None } else { Some(f.description) }, num_publishers: np, num_shift_managers: nm, num_brothers: nb, num_sisters: ns };
            if s.id > 0 { let _ = dao::update_schedule(&s); } else { let _ = dao::create_schedule(&s); }
            if let Ok(mut items) = dao::list_schedules() {
                let week_start = dao::get_configuration().ok().map(|c| c.week_start).unwrap_or_else(|| "monday".into());
                let order = weekday_order_list(&week_start);
                items.sort_by(|a, b| weekday_rank(&a.weekday, &order).cmp(&weekday_rank(&b.weekday, &order)).then(a.start_hour.cmp(&b.start_hour)));
                let mapped = items.into_iter().map(|s: NativeSchedule| ScheduleListItem {
                    id: s.id,
                    title: format!("{} • {}–{}", s.location, s.start_hour, s.end_hour),
                    subtitle: format!("{}, {} {}, {} {}, {} {}, {} {}", s.weekday, s.num_publishers, t("schedules.pubs_short"), s.num_shift_managers, t("schedules.managers_short"), s.num_brothers, t("schedules.brothers"), s.num_sisters, t("schedules.sisters")),
                }).collect::<Vec<_>>();
                raw.set(mapped.clone()); list.set(mapped);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let s = WebSchedule { id: f.id.unwrap_or_default(), location: f.location, start_hour: f.start_hour, end_hour: f.end_hour, weekday: f.weekday, description: if f.description.trim().is_empty() { None } else { Some(f.description) }, num_publishers: np, num_shift_managers: nm, num_brothers: nb, num_sisters: ns };
            if s.id > 0 { wasm_backend::update_schedule(&s); } else { let _ = wasm_backend::create_schedule(&s); }
            let mut items = wasm_backend::list_schedules();
            let week_start = wasm_backend::get_configuration().map(|c| c.week_start).unwrap_or_else(|| "monday".into());
            let order = weekday_order_list(&week_start);
            items.sort_by(|a, b| weekday_rank(&a.weekday, &order).cmp(&weekday_rank(&b.weekday, &order)).then(a.start_hour.cmp(&b.start_hour)));
            let mapped = items.into_iter().map(|s: WebSchedule| ScheduleListItem {
                id: s.id,
                title: format!("{} • {}–{}", s.location, s.start_hour, s.end_hour),
                subtitle: format!("{}, {} {}, {} {}, {} {}, {} {}", s.weekday, s.num_publishers, t("schedules.pubs_short"), s.num_shift_managers, t("schedules.managers_short"), s.num_brothers, t("schedules.brothers"), s.num_sisters, t("schedules.sisters")),
            }).collect::<Vec<_>>();
            raw.set(mapped.clone()); list.set(mapped);
        }
        modal_open.set(false);
        selected.set(vec![]);
    };

    let mut delete_schedule = move |_id: i64| {
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        { let _ = dao::delete_schedule(_id); if let Ok(mut items) = dao::list_schedules() { let week_start = dao::get_configuration().ok().map(|c| c.week_start).unwrap_or_else(|| "monday".into()); let order = weekday_order_list(&week_start); items.sort_by(|a, b| weekday_rank(&a.weekday, &order).cmp(&weekday_rank(&b.weekday, &order)).then(a.start_hour.cmp(&b.start_hour))); let mapped = items.into_iter().map(|s: NativeSchedule| ScheduleListItem { id: s.id, title: format!("{} • {}–{}", s.location, s.start_hour, s.end_hour), subtitle: format!("{}, {} {}, {} {}, {} {}, {} {}", s.weekday, s.num_publishers, t("schedules.pubs_short"), s.num_shift_managers, t("schedules.managers_short"), s.num_brothers, t("schedules.brothers"), s.num_sisters, t("schedules.sisters")) }).collect::<Vec<_>>(); raw.set(mapped.clone()); list.set(mapped); } }
        #[cfg(target_arch = "wasm32")]
        { wasm_backend::delete_schedule(_id); let mut items = wasm_backend::list_schedules(); let week_start = wasm_backend::get_configuration().map(|c| c.week_start).unwrap_or_else(|| "monday".into()); let order = weekday_order_list(&week_start); items.sort_by(|a, b| weekday_rank(&a.weekday, &order).cmp(&weekday_rank(&b.weekday, &order)).then(a.start_hour.cmp(&b.start_hour))); let mapped = items.into_iter().map(|s: WebSchedule| ScheduleListItem { id: s.id, title: format!("{} • {}–{}", s.location, s.start_hour, s.end_hour), subtitle: format!("{}, {} {}, {} {}, {} {}, {} {}", s.weekday, s.num_publishers, t("schedules.pubs_short"), s.num_shift_managers, t("schedules.managers_short"), s.num_brothers, t("schedules.brothers"), s.num_sisters, t("schedules.sisters")) }).collect::<Vec<_>>(); raw.set(mapped.clone()); list.set(mapped); }
    };

    rsx! {
        div { class: "min-h-[70vh] flex items-start justify-center",
            div { class: "w-full max-w-2xl mx-auto space-y-5",
                div { class: "flex items-center justify-between",
                    a { href: "/", class: "inline-flex items-center gap-2 h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                        span { "←" }
                        span { class: "hidden sm:inline", {t("nav.home")} }
                    }
                    button { class: "inline-flex items-center gap-2 h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition", onclick: open_create,
                        span { "➕" } span { class: "hidden sm:inline", {t("common.new")} }
                    }
                }
                div { class: "rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-4 sm:p-5 space-y-4",
                    div { class: "flex flex-col sm:flex-row gap-2 sm:items-center sm:justify-between",
                        h1 { class: "text-xl sm:text-2xl font-semibold", {t("nav.schedules")} }
                        input { class: "h-10 w-full sm:w-64 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", placeholder: t("common.search_placeholder"), value: query.read().clone(), oninput: move |e| { query.set(e.value()); apply_filter(); } }
                    }
                    {
                        let all_items = list.read().clone();
                        let total = all_items.len();
                        let pages = if total == 0 { 1 } else { ((total - 1) / PAGE_SIZE) + 1 };
                        let page = current_page.read().clone().min(pages - 1);
                        let start = page * PAGE_SIZE;
                        let end = core::cmp::min(start + PAGE_SIZE, total);
                        let page_items = all_items[start..end].to_vec();
                        let page_ids: Vec<i64> = page_items.iter().map(|p| p.id).collect();
                        let sel = selected.read().clone();
                        let all_selected_on_page = !page_ids.is_empty() && page_ids.iter().all(|id| sel.contains(id));

                        rsx!(
                            // controls row
                            div { class: "flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between",
                                // left: selection toggle and conditional select-all and bulk delete
                                div { class: "flex items-center gap-3",
                                    button { class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600", onclick: move |_| select_mode.set(!select_mode()), { if select_mode() { t("common.done") } else { t("common.select") } } }
                                    { select_mode().then(|| rsx!(
                                        div { class: "flex items-center gap-3",
                                            label { class: "inline-flex items-center gap-2 text-sm text-slate-600 dark:text-slate-300",
                                                input { r#type: "checkbox", checked: all_selected_on_page, onchange: move |e| {
                                                    let check = e.value().parse::<bool>().unwrap_or(false);
                                                    let mut v = selected.read().clone();
                                                    if check { for id in page_ids.clone() { if !v.contains(&id) { v.push(id); } } } else { v.retain(|id| !page_ids.contains(id)); }
                                                    selected.set(v);
                                                } }
                                                span { {t("common.select_all_page")} }
                                            }
                                            { (selected.read().len() > 0).then(|| rsx!(
                                                button { class: "inline-flex items-center gap-2 h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium transition",
                                                    onclick: move |_| confirm_action.set(Some(ConfirmAction::DeleteMany(selected.read().clone()))),
                                                    span { "🗑️" } span { class: "hidden sm:inline", {format!("{} ({})", t("common.delete_selected"), selected.read().len())} }
                                                }
                                            )) }
                                        }
                                    )) }
                                }
                                // right: pagination controls
                                div { class: "flex items-center gap-2 text-sm text-slate-600 dark:text-slate-300",
                                    span { {format!("{}–{} {} {}", if total==0 {0} else {start+1}, end, t("common.of"), total)} }
                                    div { class: "flex items-center gap-1",
                                        button { class: "h-8 px-2 rounded-md border border-slate-300 dark:border-slate-600 disabled:opacity-50", disabled: page == 0,
                                            onclick: move |_| { if page > 0 { current_page.set(page - 1); } }, {t("common.prev")} }
                                        button { class: "h-8 px-2 rounded-md border border-slate-300 dark:border-slate-600 disabled:opacity-50", disabled: page+1 >= pages,
                                            onclick: move |_| { if page + 1 < pages { current_page.set(page + 1); } }, {t("common.next")} }
                                    }
                                }
                            }
                            // List
                            {
                                if page_items.is_empty() {
                                    rsx!( div { class: "text-sm text-slate-600 dark:text-slate-300", {t("schedules.empty")} } )
                                } else {
                                    rsx!(
                                        ul { class: "divide-y divide-slate-200 dark:divide-slate-700",
                                            for p in page_items.into_iter() {
                                                li { class: "py-3 flex items-center justify-between gap-3",
                                                    div { class: "flex items-center gap-3 min-w-0 w-full",
                                                        { select_mode().then(|| rsx!( input { r#type: "checkbox", checked: is_selected(p.id), onchange: move |_| toggle_selected(p.id) } )) }
                                                        div { class: "min-w-0 flex-1 cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-700/30 rounded-md px-3 -mx-3 py-2",
                                                            onclick: move |_| open_edit_id(p.id),
                                                            div { class: "font-medium text-slate-800 dark:text-slate-100", {p.title.clone()} }
                                                            div { class: "text-xs text-slate-500", {p.subtitle.clone()} }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    )
                                }
                            }
                        )
                    }
                }
            }
        }

        { modal_open().then(|| rsx!(
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-lg rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold", { if form.read().id.is_some() { t("schedules.edit_title") } else { t("schedules.new_title") } } }
                    { error.read().as_ref().map(|err| rsx!( p { class: "text-red-600 text-sm", {err.clone()} } )) }
                    div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                        div { class: "space-y-1",
                            input { class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 w-full", list: "schedule-locs", placeholder: t("schedules.location"), value: form.read().location.clone(), oninput: move |e| form.write().location = e.value() }
                            datalist { id: "schedule-locs",
                                for v in loc_suggestions.read().iter() { option { value: "{v}" } }
                            }
                        }
                        select { class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: form.read().weekday.clone(), oninput: move |e| form.write().weekday = e.value(),
                            { rsx!(
                                for day in weekdays_for_locale() { option { value: "{day}", "{day}" } }
                            ) }
                        }
                    }
                    div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                        input { r#type: "time", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", placeholder: t("schedules.start"), value: form.read().start_hour.clone(), oninput: move |e| form.write().start_hour = e.value() }
                        input { r#type: "time", class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", placeholder: t("schedules.end"), value: form.read().end_hour.clone(), oninput: move |e| form.write().end_hour = e.value() }
                    }
                    textarea { class: "rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 w-full h-20", placeholder: t("schedules.description_optional"), value: form.read().description.clone(), oninput: move |e| form.write().description = e.value() }
                    div { class: "grid grid-cols-2 gap-3",
                        div { class: "flex items-center gap-2", label { class: "text-sm", {t("schedules.publishers")} } input { r#type: "number", min: "0", class: "h-10 w-24 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: form.read().num_publishers.clone(), oninput: move |e| form.write().num_publishers = e.value() } }
                        div { class: "flex items-center gap-2", label { class: "text-sm", {t("schedules.managers")} } input { r#type: "number", min: "0", class: "h-10 w-24 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: form.read().num_shift_managers.clone(), oninput: move |e| form.write().num_shift_managers = e.value() } }
                        div { class: "flex items-center gap-2", label { class: "text-sm", {t("schedules.brothers")} } input { r#type: "number", min: "0", class: "h-10 w-24 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: form.read().num_brothers.clone(), oninput: move |e| form.write().num_brothers = e.value() } }
                        div { class: "flex items-center gap-2", label { class: "text-sm", {t("schedules.sisters")} } input { r#type: "number", min: "0", class: "h-10 w-24 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500", value: form.read().num_sisters.clone(), oninput: move |e| form.write().num_sisters = e.value() } }
                    }
                    div { class: "flex items-center justify-between gap-2",
                        { form.read().id.map(|eid| rsx!( button { class: "inline-flex items-center h-9 px-3 rounded-md border border-red-300 text-red-700 text-sm font-medium transition",
                            onclick: move |_| { confirm_action.set(Some(ConfirmAction::DeleteOne(eid))); }, {t("common.delete")} } )) }
                        div { class: "flex items-center gap-2",
                            button { class: "inline-flex items-center h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition", onclick: move |_| modal_open.set(false), {t("common.cancel")} }
                            button { class: "inline-flex items-center h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition", onclick: on_submit, { if form.read().id.is_some() { t("common.save") } else { t("common.create") } } }
                        }
                    }
                }
            }
        )) }

        // Confirm modal
        { confirm_action.read().as_ref().map(|action| {
            let message = match action {
                ConfirmAction::DeleteOne(_id) => t("schedules.confirm_delete_one"),
                ConfirmAction::DeleteMany(ids) => format!("{} ({})", t("schedules.confirm_delete_many"), ids.len()),
            };
            rsx!(
                div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                    div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                        h2 { class: "text-lg font-semibold", {t("common.confirm_delete_title")} }
                        p { class: "text-sm text-slate-600 dark:text-slate-300", {message} }
                        div { class: "flex items-center justify-end gap-2",
                            button { class: "inline-flex items-center h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                                onclick: move |_| confirm_action.set(None), {t("common.cancel")} }
                            button { class: "inline-flex items-center h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium transition",
                                onclick: move |_| {
                                    let act = confirm_action.read().clone();
                                    match act {
                                        Some(ConfirmAction::DeleteOne(id)) => {
                                            delete_schedule(id);
                                            let mut v = selected.read().clone(); v.retain(|x| *x != id); selected.set(v);
                                        }
                                        Some(ConfirmAction::DeleteMany(_ids)) => {
                                            #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                                            { for id in _ids.iter().copied() { let _ = dao::delete_schedule(id); } if let Ok(mut items) = dao::list_schedules() { let week_start = dao::get_configuration().ok().map(|c| c.week_start).unwrap_or_else(|| "monday".into()); let order = weekday_order_list(&week_start); items.sort_by(|a, b| weekday_rank(&a.weekday, &order).cmp(&weekday_rank(&b.weekday, &order)).then(a.start_hour.cmp(&b.start_hour))); let mapped = items.into_iter().map(|s: NativeSchedule| ScheduleListItem { id: s.id, title: format!("{} • {}–{}", s.location, s.start_hour, s.end_hour), subtitle: format!("{}, {} {}, {} {}, {} {}, {} {}", s.weekday, s.num_publishers, t("schedules.pubs_short"), s.num_shift_managers, t("schedules.managers_short"), s.num_brothers, t("schedules.brothers"), s.num_sisters, t("schedules.sisters")) }).collect::<Vec<_>>(); raw.set(mapped.clone()); list.set(mapped); } }
                                            #[cfg(target_arch = "wasm32")]
                                            { for id in _ids.iter().copied() { wasm_backend::delete_schedule(id); } let mut items = wasm_backend::list_schedules(); let week_start = wasm_backend::get_configuration().map(|c| c.week_start).unwrap_or_else(|| "monday".into()); let order = weekday_order_list(&week_start); items.sort_by(|a, b| weekday_rank(&a.weekday, &order).cmp(&weekday_rank(&b.weekday, &order)).then(a.start_hour.cmp(&b.start_hour))); let mapped = items.into_iter().map(|s: WebSchedule| ScheduleListItem { id: s.id, title: format!("{} • {}–{}", s.location, s.start_hour, s.end_hour), subtitle: format!("{}, {} {}, {} {}, {} {}, {} {}", s.weekday, s.num_publishers, t("schedules.pubs_short"), s.num_shift_managers, t("schedules.managers_short"), s.num_brothers, t("schedules.brothers"), s.num_sisters, t("schedules.sisters")) }).collect::<Vec<_>>(); raw.set(mapped.clone()); list.set(mapped); }
                                            selected.set(vec![]);
                                        }
                                        None => {}
                                    }
                                    confirm_action.set(None);
                                },
                                span { "🗑️" } span { class: "hidden sm:inline", {t("common.delete")} }
                            }
                        }
                    }
                }
            )
        }) }
    }
}
