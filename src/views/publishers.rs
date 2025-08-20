use dioxus::prelude::*;
use crate::i18n::t;
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))] use crate::db::dao;
#[cfg(all(feature = "native-db", not(target_arch = "wasm32")))] use crate::db::dao::Publisher;
#[cfg(target_arch = "wasm32")] use crate::db::wasm_store as wasm_backend;
#[cfg(target_arch = "wasm32")] use wasm_backend::Publisher;
#[cfg(target_arch = "wasm32")] use web_sys::window;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
#[derive(PartialEq, Clone)]
struct Publisher { id: i64, first_name: String, last_name: String, gender: String, is_shift_manager: bool, priority: i64 }

const PAGE_SIZE: usize = 25;

// Locale-aware weekday helpers (mirrors logic in views/schedules.rs)
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
fn locale_prefix() -> String {
    window()
        .map(|w| w.navigator().language().unwrap_or_else(|| "en".to_string()).to_lowercase())
        .unwrap_or_else(|| "en".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn locale_prefix() -> String {
    std::env::var("LANG").unwrap_or_else(|_| "en".to_string()).to_lowercase()
}

#[allow(dead_code)]
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
        if !days.is_empty() { days.rotate_right(1); }
    }
    days
}

// Robust rank: accept English, Spanish, or French weekday names
#[allow(dead_code)]
fn weekday_rank_any(name: &str, week_start: &str) -> usize {
    let n = name.to_lowercase();
    let en = ["monday","tuesday","wednesday","thursday","friday","saturday","sunday"];
    // Spanish handled via match below (with/without accents)
    let fr = ["lundi","mardi","mercredi","jeudi","vendredi","samedi","dimanche"];

    // Try to find index in any language list (Monday=0..Sunday=6)
    let mut idx = en.iter().position(|d| *d == n).or_else(|| {
        // Map Spanish with accents and without
        match n.as_str() {
            "lunes" => Some(0),
            "martes" => Some(1),
            "miércoles" | "miercoles" => Some(2),
            "jueves" => Some(3),
            "viernes" => Some(4),
            "sábado" | "sabado" => Some(5),
            "domingo" => Some(6),
            _ => None,
        }
    }).or_else(|| fr.iter().position(|d| *d == n));

    let len = 7usize;
    let base = idx.get_or_insert(len);
    if *base >= len { return *base; }
    if week_start.eq_ignore_ascii_case("sunday") { (*base + 1) % len } else { *base }
}

#[derive(Clone, PartialEq)]
enum ConfirmAction { DeleteOne(i64), DeleteMany(Vec<i64>) }

#[derive(PartialEq, Clone)]
struct PublisherForm { id: Option<i64>, first_name: String, last_name: String, gender: String, is_shift_manager: bool, priority: String }

fn normalize_for_search(s: &str) -> String {
    let lower = s.to_lowercase();
    lower.replace('á', "a").replace('à', "a").replace('ä', "a").replace('â', "a")
         .replace('é', "e").replace('è', "e").replace('ë', "e").replace('ê', "e")
         .replace('í', "i").replace('ì', "i").replace('ï', "i").replace('î', "i")
         .replace('ó', "o").replace('ò', "o").replace('ö', "o").replace('ô', "o")
         .replace('ú', "u").replace('ù', "u").replace('ü', "u").replace('û', "u")
         .replace('ñ', "n")
}

#[component]
pub fn Publishers() -> Element {
    let list = use_signal(|| Vec::<Publisher>::new());
    let mut query = use_signal(|| String::new());
    let mut modal_open = use_signal(|| false);
    let mut form = use_signal(|| PublisherForm { id: None, first_name: String::new(), last_name: String::new(), gender: "Male".into(), is_shift_manager: false, priority: "5".into() });
    let mut error = use_signal(|| Option::<String>::None);
    let mut current_page = use_signal(|| 0usize);
    let mut selected = use_signal(|| Vec::<i64>::new());
    let mut confirm_action = use_signal(|| Option::<ConfirmAction>::None);
    let mut select_mode = use_signal(|| false);
    // schedules list (id, label) and availability selected for current form
    let schedules = use_signal(|| Vec::<(i64, String)>::new());
    let mut avail_selected = use_signal(|| Vec::<i64>::new());
    // Relationships state for the current form
    let mut rel_selected = use_signal(|| Vec::<(i64, String)>::new()); // (other_id, kind: 'recommended'|'mandatory')
    let mut rel_add_pid = use_signal(|| String::new());
    let mut rel_add_kind = use_signal(|| "recommended".to_string());

    use_effect(move || {
    #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            if let Ok(items) = dao::list_publishers() { list.set(items); }
            if let Ok(mut schs) = dao::list_schedules() {
                let week_start = dao::get_configuration().ok().map(|c| c.week_start).unwrap_or_else(|| "monday".into());
                schs.sort_by(|a,b| {
                    weekday_rank_any(&a.weekday, &week_start)
                        .cmp(&weekday_rank_any(&b.weekday, &week_start))
                        .then(a.start_hour.cmp(&b.start_hour))
                        .then(a.location.cmp(&b.location))
                });
                let mapped = schs.into_iter().map(|s| (s.id, format!("{} • {} {}-{}", s.weekday, s.location, s.start_hour, s.end_hour))).collect();
                schedules.set(mapped);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let mut list_sig = list.clone();
            list_sig.set(wasm_backend::list_publishers());
            let mut v = wasm_backend::list_schedules();
            let week_start = wasm_backend::get_configuration().map(|c| c.week_start).unwrap_or_else(|| "monday".into());
            v.sort_by(|a,b| {
                weekday_rank_any(&a.weekday, &week_start)
                    .cmp(&weekday_rank_any(&b.weekday, &week_start))
                    .then(a.start_hour.cmp(&b.start_hour))
                    .then(a.location.cmp(&b.location))
            });
            let mapped = v.into_iter().map(|s| (s.id, format!("{} • {} {}-{}", s.weekday, s.location, s.start_hour, s.end_hour))).collect();
            let mut schedules_sig = schedules.clone();
            schedules_sig.set(mapped);
        }
    });

    let filtered = || {
        let q = normalize_for_search(&query.read());
        let items = list.read().clone();
        if q.is_empty() { return items; }
        items.into_iter().filter(|p| {
            let name = normalize_for_search(&format!("{} {}", p.first_name, p.last_name));
            name.contains(&q)
        }).collect::<Vec<_>>()
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
    let mut clear_selection = {
        let mut selected = selected.clone();
        move || selected.set(vec![])
    };

    let open_create = move |_| {
        error.set(None);
        form.set(PublisherForm { id: None, first_name: String::new(), last_name: String::new(), gender: "Male".into(), is_shift_manager: false, priority: "5".into() });
    avail_selected.set(vec![]);
    rel_selected.set(vec![]);
    rel_add_pid.set(String::new());
    rel_add_kind.set("recommended".into());
        modal_open.set(true);
    };
    let mut open_edit_id = {
        let list = list.clone();
        let mut form = form.clone();
        let mut modal_open = modal_open.clone();
    move |id: i64| {
            error.set(None);
            if let Some(p) = list.read().iter().find(|x| x.id == id).cloned() {
                form.set(PublisherForm { id: Some(p.id), first_name: p.first_name, last_name: p.last_name, gender: p.gender, is_shift_manager: p.is_shift_manager, priority: p.priority.to_string() });
        // load availability for this publisher
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        if let Ok(a) = dao::list_availability_for_publisher(id) { avail_selected.set(a); }
        #[cfg(target_arch = "wasm32")]
        { avail_selected.set(wasm_backend::list_availability_for_publisher(id)); }
        // load relationships for this publisher
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            if let Ok(rels) = dao::list_relationships_for_publisher(id) {
                let v: Vec<(i64,String)> = rels.into_iter().map(|(oid, k)| {
                    let kind = match k { dao::RelationshipKind::Mandatory => "mandatory".to_string(), dao::RelationshipKind::Recommended => "recommended".to_string() };
                    (oid, kind)
                }).collect();
                rel_selected.set(v);
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let rels = wasm_backend::list_relationships_for_publisher(id);
            let v: Vec<(i64,String)> = rels.into_iter().map(|(oid, k)| {
                let kind = match k { wasm_backend::RelationshipKind::Mandatory => "mandatory".to_string(), wasm_backend::RelationshipKind::Recommended => "recommended".to_string() };
                (oid, kind)
            }).collect();
            rel_selected.set(v);
        }
                modal_open.set(true);
            }
        }
    };
    let close_modal = move |_| modal_open.set(false);

    let on_submit = move |_| {
        error.set(None);
        let f = form.read().clone();
    if f.first_name.trim().is_empty() || f.last_name.trim().is_empty() { error.set(Some(t("publishers.error_required"))); return; }
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            if let Some(id) = f.id {
                if dao::update_publisher(id, &f.first_name, &f.last_name, &f.gender, f.is_shift_manager, f.priority.parse().unwrap_or(5)).is_err() { error.set(Some(t("publishers.error_update"))); return; }
                // save availability
                let _ = dao::set_publisher_availability(id, &avail_selected.read());
                // sync relationships
                use std::collections::HashSet;
                let target: Vec<(i64,String)> = rel_selected.read().clone();
                let existing = dao::list_relationships_for_publisher(id).unwrap_or_default();
                let target_ids: HashSet<i64> = target.iter().map(|(oid,_)| *oid).collect();
                for (oid, _k) in existing.iter() { if !target_ids.contains(oid) { let _ = dao::remove_relationship(id, *oid); } }
                for (oid, kind) in target.iter() {
                    let k = if kind == "mandatory" { dao::RelationshipKind::Mandatory } else { dao::RelationshipKind::Recommended };
                    let _ = dao::add_relationship(id, *oid, k);
                }
                if let Ok(items) = dao::list_publishers() { list.set(items); }
            } else {
                match dao::create_publisher(&f.first_name, &f.last_name, &f.gender, f.is_shift_manager, f.priority.parse().unwrap_or(5)) {
                    Ok(new_id) => {
                        let _ = dao::set_publisher_availability(new_id, &avail_selected.read());
                        // add relationships for new publisher
                        for (oid, kind) in rel_selected.read().iter() {
                            let k = if kind == "mandatory" { dao::RelationshipKind::Mandatory } else { dao::RelationshipKind::Recommended };
                            let _ = dao::add_relationship(new_id, *oid, k);
                        }
                        if let Ok(items) = dao::list_publishers() { list.set(items); }
                    }
                    Err(_) => { error.set(Some(t("publishers.error_create"))); return; }
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(id) = f.id {
                wasm_backend::update_publisher(id, &f.first_name, &f.last_name, &f.gender, f.is_shift_manager, f.priority.parse().unwrap_or(5));
                wasm_backend::set_publisher_availability(id, &avail_selected.read());
                // sync relationships
                use std::collections::HashSet;
                let target: Vec<(i64,String)> = rel_selected.read().clone();
                let existing = wasm_backend::list_relationships_for_publisher(id);
                let target_ids: HashSet<i64> = target.iter().map(|(oid,_)| *oid).collect();
                for (oid, _k) in existing.iter() { if !target_ids.contains(oid) { wasm_backend::remove_relationship(id, *oid); } }
                for (oid, kind) in target.iter() {
                    let k = if kind == "mandatory" { wasm_backend::RelationshipKind::Mandatory } else { wasm_backend::RelationshipKind::Recommended };
                    wasm_backend::add_relationship(id, *oid, k);
                }
            } else {
                let new_id = wasm_backend::create_publisher(&f.first_name, &f.last_name, &f.gender, f.is_shift_manager, f.priority.parse().unwrap_or(5));
                wasm_backend::set_publisher_availability(new_id, &avail_selected.read());
                for (oid, kind) in rel_selected.read().iter() {
                    let k = if kind == "mandatory" { wasm_backend::RelationshipKind::Mandatory } else { wasm_backend::RelationshipKind::Recommended };
                    wasm_backend::add_relationship(new_id, *oid, k);
                }
            }
            let mut list_sig = list.clone();
            list_sig.set(wasm_backend::list_publishers());
        }
    modal_open.set(false);
    clear_selection();
    };

    let delete_publisher = move |id: i64| {
        #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
        {
            if dao::delete_publisher(id).is_ok() {
                if let Ok(items) = dao::list_publishers() { list.set(items); }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            wasm_backend::delete_publisher(id);
            let mut list_sig = list.clone();
            list_sig.set(wasm_backend::list_publishers());
        }
        #[cfg(all(not(target_arch = "wasm32"), not(feature = "native-db")))]
        { let _ = id; }
    };

    rsx! {
        div { class: "min-h-[70vh] flex items-start justify-center",
            div { class: "w-full max-w-2xl mx-auto space-y-5",
                div { class: "flex items-center justify-between",
                    a {
                        href: "/",
                        class: "inline-flex items-center gap-2 h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                        span { "←" }
                        span { class: "hidden sm:inline", {t("nav.home")} }
                    }
                    button {
                        class: "inline-flex items-center gap-2 h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition",
                        onclick: open_create,
                        span { "➕" }
                        span { class: "hidden sm:inline", {t("common.new")} }
                    }
                }
                div { class: "rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-sm p-4 sm:p-5 space-y-4",
                    div { class: "flex flex-col sm:flex-row gap-2 sm:items-center sm:justify-between",
                        h1 { class: "text-xl sm:text-2xl font-semibold", {t("nav.publishers")} }
                        input {
                            class: "h-10 w-full sm:w-64 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            placeholder: t("common.search_placeholder"),
                            value: query.read().clone(),
                            oninput: move |e| {
                                query.set(e.value());
                                current_page.set(0);
                            },
                        }
                    }
                    {
                        // selection + bulk actions + pagination header bar
                        let all_items = filtered();
                        let total = all_items.len();
                        let pages = if total == 0 { 1 } else { ((total - 1) / PAGE_SIZE) + 1 };
                        let page = current_page.read().clone().min(pages - 1);
                        let start = page * PAGE_SIZE;
                        let end = core::cmp::min(start + PAGE_SIZE, total);
                        let page_items = all_items[start..end].to_vec();
                        let page_ids: Vec<i64> = page_items.iter().map(|p| p.id).collect();
                        let sel = selected.read().clone();
                        let all_selected_on_page = !page_ids.is_empty()

                            // controls row
                            // left: selection toggle + (conditional) select-all and bulk delete
                            // right: pagination controls
                            // List
                            && page_ids.iter().all(|id| sel.contains(id));
                        rsx! {
                            div { class: "flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between",
                                div { class: "flex items-center gap-3",
                                    button {
                                        class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600",
                                        onclick: move |_| select_mode.set(!select_mode()),
                                        {if select_mode() { t("common.done") } else { t("common.select") }}
                                    }
                                    {select_mode().then(|| rsx! {
                                        div { class: "flex items-center gap-3",
                                            label { class: "inline-flex items-center gap-2 text-sm text-slate-600 dark:text-slate-300",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: all_selected_on_page,
                                                    onchange: move |e| {
                                                        let check = e.value().parse::<bool>().unwrap_or(false);
                                                        let mut v = selected.read().clone();
                                                        if check {
                                                            for id in page_ids.clone() {
                                                                if !v.contains(&id) {
                                                                    v.push(id);
                                                                }
                                                            }
                                                        } else {
                                                            v.retain(|id| !page_ids.contains(id));
                                                        }
                                                        selected.set(v);
                                                    },
                                                }
                                                span { {t("common.select_all_page")} }
                                            }
                                            {(selected.read().len() > 0).then(|| rsx! {
                                                button {
                                                    class: "inline-flex items-center gap-2 h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium transition",
                                                    onclick: move |_| {
                                                        confirm_action.set(Some(ConfirmAction::DeleteMany(selected.read().clone())))
                                                    },
                                                    span { "🗑️" }
                                                    span { class: "hidden sm:inline",
                                                        {format!("{} ({})", t("common.delete_selected"), selected.read().len())}
                                                    }
                                                }
                                            })}
                                        }
                                    })}
                                }
                                div { class: "flex items-center gap-2 text-sm text-slate-600 dark:text-slate-300",
                                    span {
                                        {
                                            format!(
                                                "{}–{} {} {}",
                                                if total == 0 { 0 } else { start + 1 },
                                                end,
                                                t("common.of"),
                                                total,
                                            )
                                        }
                                    }
                                    div { class: "flex items-center gap-1",
                                        button {
                                            class: "h-8 px-2 rounded-md border border-slate-300 dark:border-slate-600 disabled:opacity-50",
                                            disabled: page == 0,
                                            onclick: move |_| {
                                                if page > 0 {
                                                    current_page.set(page - 1);
                                                }
                                            },
                                            {t("common.prev")}
                                        }
                                        button {
                                            class: "h-8 px-2 rounded-md border border-slate-300 dark:border-slate-600 disabled:opacity-50",
                                            disabled: page + 1 >= pages,
                                            onclick: move |_| {
                                                if page + 1 < pages {
                                                    current_page.set(page + 1);
                                                }
                                            },
                                            {t("common.next")}
                                        }
                                    }
                                }
                            }
                            {
                                if page_items.is_empty() {
                                    rsx! {
                                        div { class: "text-sm text-slate-600 dark:text-slate-300", {t("publishers.empty")} }
                                    }
                                } else {
                                    rsx! {
                                        ul { class: "divide-y divide-slate-200 dark:divide-slate-700",
                                            for p in page_items.into_iter() {
                                                li { class: "py-3 flex items-center justify-between gap-3",
                                                    div { class: "flex items-center gap-3 min-w-0 w-full",
                                                        {select_mode().then(|| rsx! {
                                                            input {
                                                                r#type: "checkbox",
                                                                checked: is_selected(p.id),
                                                                onchange: move |_| toggle_selected(p.id),
                                                            }
                                                        })}
                                                        div {
                                                            class: "min-w-0 flex-1 cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-700/30 rounded-md px-3 -mx-3 py-2",
                                                            onclick: move |_| open_edit_id(p.id),
                                                            div { class: "font-medium text-slate-800 dark:text-slate-100",
                                                                {
                                                                    #[cfg(target_arch = "wasm32")]
                                                                    {
                                                                        let order = wasm_backend::get_name_order();
                                                                        if order == "last_first" {
                                                                            format!("{} {}", p.last_name, p.first_name)
                                                                        } else {
                                                                            format!("{} {}", p.first_name, p.last_name)
                                                                        }
                                                                    }
                                                                    #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                                                                    { format!("{} {}", p.first_name, p.last_name) }
                                                                }
                                                            }
                                                            div { class: "text-xs text-slate-500 flex items-center gap-2",
                                                                span { {if p.gender == "Male" { "♂️" } else { "♀️" }} }
                                                                span {
                                                                    {
                                                                        let gender = if p.gender == "Male" {
                                                                            t("publishers.gender.male")
                                                                        } else {
                                                                            t("publishers.gender.female")
                                                                        };
                                                                        let mgr = if p.is_shift_manager {
                                                                            format!(" • {}", t("publishers.manager_short"))
                                                                        } else {
                                                                            String::new()
                                                                        };
                                                                        format!("{}{} • {} {}", gender, mgr, t("publishers.priority"), p.priority)
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // gender segmented buttons
        // Availability section
        // Relationships section
        // Add control
        // List existing relationships
        // In-edit delete button (when editing existing)
        {modal_open().then(|| rsx! {
            div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                    h2 { class: "text-lg font-semibold",
                        {
                            if form.read().id.is_some() {
                                t("publishers.edit_title")
                            } else {
                                t("publishers.new_title")
                            }
                        }
                    }
                    {error.read().as_ref().map(|err| rsx! {
                        p { class: "text-red-600 text-sm", {err.clone()} }
                    })}
                    div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                        input {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            placeholder: {t("publishers.first_name")},
                            value: form.read().first_name.clone(),
                            oninput: move |e| form.write().first_name = e.value(),
                        }
                        input {
                            class: "h-10 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                            placeholder: {t("publishers.last_name")},
                            value: form.read().last_name.clone(),
                            oninput: move |e| form.write().last_name = e.value(),
                        }
                    }
                    div { class: "grid grid-cols-1 sm:grid-cols-2 gap-3",
                        div { class: "flex items-center gap-2",
                            button {
                                class: {
                                    format!(
                                        "h-10 px-3 rounded-md text-sm font-medium {}",
                                        if form.read().gender == "Male" {
                                            "bg-blue-600 text-white"
                                        } else {
                                            "border border-slate-300 dark:border-slate-600"
                                        },
                                    )
                                },
                                onclick: move |_| form.write().gender = "Male".into(),
                                {format!("♂️ {}", t("publishers.gender.male"))}
                            }
                            button {
                                class: {
                                    format!(
                                        "h-10 px-3 rounded-md text-sm font-medium {}",
                                        if form.read().gender == "Female" {
                                            "bg-blue-600 text-white"
                                        } else {
                                            "border border-slate-300 dark:border-slate-600"
                                        },
                                    )
                                },
                                onclick: move |_| form.write().gender = "Female".into(),
                                {format!("♀️ {}", t("publishers.gender.female"))}
                            }
                        }
                        div { class: "flex items-center gap-2",
                            input {
                                r#type: "checkbox",
                                checked: form.read().is_shift_manager,
                                onchange: move |e| form.write().is_shift_manager = e.value().parse::<bool>().unwrap_or(false),
                            }
                            span { class: "text-sm", {t("publishers.shift_manager")} }
                        }
                    }
                    div { class: "grid grid-cols-1 gap-3",
                        div { class: "flex items-center gap-2",
                            label { class: "text-sm", {t("publishers.priority")} }
                            input {
                                class: "h-10 w-20 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500",
                                value: form.read().priority.clone(),
                                oninput: move |e| form.write().priority = e.value(),
                            }
                        }
                    }
                    div { class: "space-y-2",
                        h3 { class: "text-sm font-semibold text-slate-700 dark:text-slate-200",
                            {t("publishers.availability")}
                        }
                        {
                            let schs = schedules.read().clone();
                            if schs.is_empty() {
                                rsx! {
                                    p { class: "text-sm text-slate-500", {t("publishers.no_schedules_yet")} }
                                }
                            } else {
                                let mut toggle_avail = {
                                    let mut avail_selected = avail_selected.clone();
                                    move |sid: i64| {
                                        let mut v = avail_selected.read().clone();
                                        if let Some(pos) = v.iter().position(|x| *x == sid) {
                                            v.remove(pos);
                                        } else {
                                            v.push(sid);
                                        }
                                        avail_selected.set(v);
                                    }
                                };
                                rsx! {
                                    ul { class: "divide-y divide-slate-200 dark:divide-slate-700 rounded-md border border-slate-200 dark:border-slate-700",
                                        for (sid , label) in schs.into_iter() {
                                            li { class: "px-3 py-2 flex items-center gap-3",
                                                input {
                                                    r#type: "checkbox",
                                                    checked: avail_selected.read().contains(&sid),
                                                    onchange: move |_| toggle_avail(sid),
                                                }
                                                span { class: "text-sm", {label} }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "space-y-2",
                        h3 { class: "text-sm font-semibold text-slate-700 dark:text-slate-200",
                            {t("publishers.relationships")}
                        }
                        div { class: "flex items-center gap-2",
                            select {
                                class: "h-9 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-2 text-sm w-full",
                                value: rel_add_pid.read().clone(),
                                onchange: move |e| rel_add_pid.set(e.value()),
                                option { value: "", {t("common.select_publisher")} }
                                {
                                    let cur_id = form.read().id.unwrap_or(-1);
                                    let existing_ids: std::collections::HashSet<i64> = rel_selected
                                        .read()
                                        .iter()
                                        .map(|(oid, _)| *oid)
                                        .collect();
                                    rsx! {
                                        for p in list.read().iter().filter(|pp| pp.id != cur_id && !existing_ids.contains(&pp.id)) {
                                            option { value: {format!("{}", p.id)}, {format!("{} {}", p.first_name, p.last_name)} }
                                        }
                                    }
                                }
                            }
                            select {
                                class: "h-9 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-2 text-sm",
                                value: rel_add_kind.read().clone(),
                                onchange: move |e| rel_add_kind.set(e.value()),
                                option { value: "recommended", {t("publishers.recommended")} }
                                option { value: "mandatory", {t("publishers.mandatory")} }
                            }
                            button {
                                class: "h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600",
                                onclick: move |_| {
                                    let pid_input = rel_add_pid.read().clone();
                                    if let Ok(pid) = pid_input.parse::<i64>() {
                                        let mut v = rel_selected.read().clone();
                                        if let Some(existing) = v.iter_mut().find(|(oid, _)| *oid == pid) {
                                            existing.1 = rel_add_kind.read().clone();
                                        } else {
                                            v.push((pid, rel_add_kind.read().clone()));
                                        }
                                        rel_selected.set(v);
                                        rel_add_pid.set(String::new());
                                    }
                                },
                                {t("publishers.add")}
                            }
                        }
                        {
                            let rels = rel_selected.read().clone();
                            if rels.is_empty() {
                                rsx! {
                                    p { class: "text-sm text-slate-500", {t("publishers.no_relationships_yet")} }
                                }
                            } else {
                                rsx! {
                                    ul { class: "divide-y divide-slate-200 dark:divide-slate-700 rounded-md border border-slate-200 dark:border-slate-700",
                                        for (oid , kind) in rels.into_iter() {
                                            li { class: "px-3 py-2 flex items-center gap-3 justify-between",
                                                div { class: "text-sm",
                                                    {
                                                        if let Some(pp) = list.read().iter().find(|pp| pp.id == oid) {
                                                            format!("{} {}", pp.first_name, pp.last_name)
                                                        } else {
                                                            format!("#{}", oid)
                                                        }
                                                    }
                                                }
                                                div { class: "flex items-center gap-2",
                                                    select {
                                                        class: "h-8 rounded-md border border-slate-300 dark:border-slate-600 bg-white dark:bg-slate-900 px-2 text-xs",
                                                        value: kind.clone(),
                                                        onchange: move |e| {
                                                            let mut v = rel_selected.read().clone();
                                                            if let Some(it) = v.iter_mut().find(|(id, _)| *id == oid) {
                                                                it.1 = e.value();
                                                            }
                                                            rel_selected.set(v);
                                                        },
                                                        option { value: "recommended", {t("publishers.recommended")} }
                                                        option { value: "mandatory", {t("publishers.mandatory")} }
                                                    }
                                                    button {
                                                        class: "h-8 px-2 rounded-md border border-red-300 text-red-700 text-xs",
                                                        onclick: move |_| {
                                                            let mut v = rel_selected.read().clone();
                                                            v.retain(|(id, _)| *id != oid);
                                                            rel_selected.set(v);
                                                        },
                                                        {t("publishers.remove")}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "flex items-center justify-between gap-2",
                        {form.read().id.map(|eid| rsx! {
                            button {
                                class: "inline-flex items-center h-9 px-3 rounded-md border border-red-300 text-red-700 text-sm font-medium transition",
                                onclick: move |_| {
                                    confirm_action.set(Some(ConfirmAction::DeleteOne(eid)));
                                },
                                {t("common.delete")}
                            }
                        })}
                        div { class: "flex items-center gap-2",
                            button {
                                class: "inline-flex items-center h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                                onclick: close_modal,
                                {t("common.cancel")}
                            }
                            button {
                                class: "inline-flex items-center h-9 px-3 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-sm font-medium transition",
                                onclick: on_submit,
                                {if form.read().id.is_some() { t("common.save") } else { t("common.create") }}
                            }
                        }
                    }
                }
            }
        })}

        // Confirm modal
        {
            confirm_action
                .read()
                // also unselect if present
                // ensure edit modal is closed if open
                // close edit modal if open
                .as_ref()
                .map(|action| {
                    let message = match action {
                        ConfirmAction::DeleteOne(_id) => t("publishers.confirm_delete_one"),
                        ConfirmAction::DeleteMany(ids) => {
                            format!(
                                "{} ({})",
                                t("publishers.confirm_delete_many"),
                                ids.len(),
                            )
                        }
                    };
                    rsx! {
                        div { class: "fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4",
                            div { class: "w-full max-w-md rounded-xl border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-800 shadow-lg p-5 space-y-4",
                                h2 { class: "text-lg font-semibold", {t("common.confirm_delete_title")} }
                                p { class: "text-sm text-slate-600 dark:text-slate-300", {message} }
                                div { class: "flex items-center justify-end gap-2",
                                    button {
                                        class: "inline-flex items-center h-9 px-3 rounded-md border border-slate-300 dark:border-slate-600 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 text-sm font-medium transition",
                                        onclick: move |_| confirm_action.set(None),
                                        {t("common.cancel")}
                                    }
                                    button {
                                        class: "inline-flex items-center h-9 px-3 rounded-md bg-red-600 hover:bg-red-500 text-white text-sm font-medium transition",
                                        onclick: move |_| {
                                            let act = confirm_action.read().clone();
                                            match act {
                                                Some(ConfirmAction::DeleteOne(id)) => {
                                                    delete_publisher(id);
                                                    let mut v = selected.read().clone();
                                                    v.retain(|x| *x != id);
                                                    selected.set(v);
                                                    modal_open.set(false);
                                                }
                                                Some(ConfirmAction::DeleteMany(_ids)) => {
                                                    #[cfg(all(feature = "native-db", not(target_arch = "wasm32")))]
                                                    {
                                                        for id in _ids.iter().copied() {
                                                            let _ = dao::delete_publisher(id);
                                                        }
                                                        if let Ok(items) = dao::list_publishers() {
                                                            let mut list_sig = list.clone();
                                                            list_sig.set(items);
                                                        }
                                                    }
                                                    #[cfg(target_arch = "wasm32")]
                                                    {
                                                        for id in _ids.iter().copied() {
                                                            wasm_backend::delete_publisher(id);
                                                        }
                                                        let mut list_sig = list.clone();
                                                        list_sig.set(wasm_backend::list_publishers());
                                                    }
                                                    selected.set(vec![]);
                                                    modal_open.set(false);
                                                }
                                                None => {}
                                            }
                                            confirm_action.set(None);
                                        },
                                        span { "🗑️" }
                                        span { class: "hidden sm:inline", {t("common.delete")} }
                                    }
                                }
                            }
                        }
                    }
                })
        }
    }
}
