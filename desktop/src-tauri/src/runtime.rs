use crate::{
    core::Resolution,
    engine::{Engine, TaskInfo},
    model::{Action, Generation, Model, Request, View},
    storage::Store,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{sync::mpsc, task::JoinHandle};
pub struct CompletedPaste {
    pub text: String,
    pub revision: u64,
    pub task: Option<String>,
}
pub struct Service {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub paste_completed: mpsc::UnboundedSender<CompletedPaste>,
    pub shutdown: mpsc::UnboundedSender<()>,
    pub exiting: Arc<std::sync::atomic::AtomicBool>,
    pub tx: mpsc::UnboundedSender<Request>,
    pub view: Arc<Mutex<View>>,
}
enum Event {
    Ready(u64, Result<Engine, String>),
    Tasks(u64, Result<(Vec<TaskInfo>, Option<String>), String>, bool),
    Context(u64, u64, Result<crate::core::Context, String>),
    Preflight(u64, u64, Generation, Result<crate::core::Context, String>),
    Generated(Generation, Result<serde_json::Value, String>, f64),
}
fn abort(job: &mut Option<JoinHandle<()>>) {
    if let Some(job) = job.take() {
        job.abort()
    }
}
fn schedule(model: &Model, due: &mut Option<Instant>) {
    if model.view.settings.automatic
        && model.view.connected
        && model.context.is_some()
        && !model.view.active
        && model.view.phase == "idle"
    {
        *due = Some(Instant::now() + Duration::from_millis(850));
    }
}
fn publish(app: &AppHandle, shared: &Arc<Mutex<View>>, model: &mut Model) {
    model.sync();
    if let Ok(mut v) = shared.lock() {
        *v = model.view.clone();
    }
    let _ = app.emit("view", &model.view);
}
fn save(store: &Store, model: &mut Model) {
    model.sync();
    model.view.storage_problem = store.save(&model.saved).err();
}
fn preflight(
    engine: &Engine,
    g: Generation,
    epoch: u64,
    ticket: u64,
    tx: mpsc::UnboundedSender<Event>,
) -> JoinHandle<()> {
    let engine = engine.clone();
    tokio::spawn(async move {
        let result = engine.context(&g.task.id).await;
        let _ = tx.send(Event::Preflight(epoch, ticket, g, result));
    })
}
fn read_context(
    engine: &Engine,
    id: String,
    epoch: u64,
    ticket: u64,
    tx: mpsc::UnboundedSender<Event>,
) -> JoinHandle<()> {
    let engine = engine.clone();
    tokio::spawn(async move {
        let result = engine.context(&id).await;
        let _ = tx.send(Event::Context(epoch, ticket, result));
    })
}
fn connect(dir: PathBuf, epoch: u64, tx: mpsc::UnboundedSender<Event>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let _ = tx.send(Event::Ready(epoch, Engine::connect(&dir).await));
    })
}
pub fn start(app: AppHandle, dir: PathBuf, legacy: Option<PathBuf>) -> Service {
    let (store, saved, error) = Store::load(dir.clone(), legacy.as_deref());
    let mut model = Model::new(saved, error);
    let shared = Arc::new(Mutex::new(model.view.clone()));
    let view = shared.clone();
    let (shutdown, mut stop) = mpsc::unbounded_channel::<()>();
    let exiting = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let finished = exiting.clone();
    let (tx, mut rx) = mpsc::unbounded_channel::<Request>();
    let (paste_completed, mut pastes) = mpsc::unbounded_channel::<CompletedPaste>();
    tauri::async_runtime::spawn(async move {
        let (events, mut results) = mpsc::unbounded_channel();
        let mut epoch = 0;
        let mut list_revision = 0;
        let mut ticket = 0;
        let mut applied = 0;
        let mut connection = Some(connect(dir.clone(), epoch, events.clone()));
        let mut engine: Option<Engine> = None;
        let mut generation: Option<JoinHandle<()>> = None;
        let mut context_job: Option<JoinHandle<()>> = None;
        let mut listing: Option<JoinHandle<()>> = None;
        let mut due = None;
        let mut next_poll = Instant::now() + Duration::from_secs(2);
        let mut cursor: Option<String> = None;
        let mut clock = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                biased;
                _ = stop.recv() => break,
                request=rx.recv()=>{
                    let Some(request) = request else { break };
                    if request.sequence <= model.view.acknowledged {
                        continue;
                    }
                    model.view.acknowledged = request.sequence;
                    let previous = model.view.revision;
                    let mut persist = false;
                    let mut should_schedule = false;
                    let mut generate: Option<(bool, Option<Resolution>)> = None;
                    match request.action {
                        Action::Edit { draft } => {
                            model.edit(draft);
                            persist = true;
                            should_schedule = true;
                        }
                        Action::Select { id } => {
                            if engine.is_none() {
                                continue;
                            }
                            if let Some(task) = model.view.tasks.iter().find(|t| t.id == id).cloned() {
                                epoch += 1;
                                model.select(task);
                                abort(&mut context_job);
                                due = None;
                                persist = true;
                                if let Some(e) = &engine {
                                    ticket += 1;
                                    context_job = Some(read_context(e, id, epoch, ticket, events.clone()));
                                }
                            }
                        }
                        Action::Insert { index, revision } => {
                            model.insert(index, revision);
                            persist = true;
                            should_schedule = true;
                        }
                        Action::Undo => {
                            model.undo();
                            persist = true;
                            should_schedule = true;
                        }
                        Action::Clear => {
                            model.clear();
                            persist = true;
                            should_schedule = true;
                        }
                        Action::Copy => {
                            if !model.view.draft.text.trim().is_empty() {
                                let ok = arboard::Clipboard::new()
                                    .and_then(|mut c| c.set_text(model.view.draft.text.trim().to_string()))
                                    .is_ok();
                                model.copy_result(ok);
                                persist = true;
                                should_schedule = true;
                            }
                        }
                        Action::Expand { revision } => {
                            if revision == model.view.revision {
                                generate = Some((true, None));
                            }
                        }
                        Action::Choose { index, revision } => {
                            if revision == model.view.revision && model.view.phase == "clarification" {
                                if let Some(r) = model.resolution(index) {
                                    generate = Some((true, Some(r)));
                                }
                            }
                        }
                        Action::KeepOriginal => {
                            if ["expanding", "clarification"].contains(&model.view.phase.as_str()) {
                                model.invalidate();
                                model.view.focus += 1;
                                model.view.status = "Kept your original words".into();
                                should_schedule = true;
                            }
                        }
                        Action::Hover { value } => model.hover(value),
                        Action::Refresh => {
                            if !model.view.active
                                && !["expanding", "clarification"].contains(&model.view.phase.as_str())
                            {
                                generate = Some((false, None));
                            }
                        }
                        Action::Settings { mut settings } => {
                            settings.normalize();
                            model.view.settings = settings;
                            if let Some(w) = app.get_webview_window("main") {
                                if let Err(e) = w.set_always_on_top(model.view.settings.floating) {
                                    model.view.problem = Some(e.to_string());
                                }
                            }
                            if model.view.phase != "expanding" && model.view.phase != "clarification" {
                                model.invalidate();
                                if !model.view.settings.automatic {
                                    model.view.status = "Automatic suggestions paused".into();
                                }
                                should_schedule = true;
                            }
                            persist = true;
                        }
                        Action::Tasks { search: s, more } => {
                            if let Some(e) = &engine {
                                list_revision += 1;
                                let rev = list_revision;
                                model.view.loading_tasks = true;
                                abort(&mut listing);
                                let e = e.clone();
                                let q = s;
                                let c = if more { cursor.clone() } else { None };
                                let tx = events.clone();
                                listing = Some(tokio::spawn(async move {
                                    let result = e.tasks(&q, c.as_deref()).await;
                                    let _ = tx.send(Event::Tasks(rev, result, more));
                                }));
                            }
                        }
                        Action::Reconnect => {
                            epoch += 1;
                            list_revision += 1;
                            cursor = None;
                            model.view.loading_tasks = false;
                            abort(&mut connection);
                            abort(&mut context_job);
                            abort(&mut listing);
                            model.invalidate();
                            model.context = None;
                            engine = None;
                            model.view.connected = false;
                            model.view.connecting = true;
                            model.view.problem = None;
                            model.view.status = "Connecting to Codex…".into();
                            connection = Some(connect(dir.clone(), epoch, events.clone()));
                        }
                    }
                    if model.view.revision != previous {
                        abort(&mut generation);
                        due = None;
                    }
                    if should_schedule {
                        schedule(&model, &mut due)
                    }
                    if let (Some(e), Some((expand, resolution))) = (&engine, generate) {
                        if let Some(g) = model.begin(expand, resolution) {
                            abort(&mut generation);
                            due = None;
                            ticket += 1;
                            generation = Some(preflight(e, g, epoch, ticket, events.clone()));
                        }
                    }
                    if persist {
                        save(&store, &mut model)
                    }
                    publish(&app, &shared, &mut model);
                },
                Some(paste) = pastes.recv() => {
                    if model.paste_completed(&paste.text, paste.revision, paste.task.as_deref()) {
                        abort(&mut generation);
                        due = None;
                        save(&store, &mut model);
                        schedule(&model, &mut due);
                        publish(&app, &shared, &mut model);
                    }
                }
                event=results.recv()=>{
                    let Some(event) = event else { break };
                    match event {
                        Event::Ready(rev, result) if rev == epoch => {
                            model.view.connecting = false;
                            match result {
                                Ok(e) => {
                                    model.view.connected = true;
                                    model.view.model = e.model.clone();
                                    model.view.expansion_model = e.expansion_model.clone();
                                    model.view.status = "Choose a task to begin".into();
                                    list_revision += 1;
                                    let rev = list_revision;
                                    let clone = e.clone();
                                    let tx = events.clone();
                                    listing = Some(tokio::spawn(async move {
                                        let r = clone.tasks("", None).await;
                                        let _ = tx.send(Event::Tasks(rev, r, false));
                                    }));
                                    if let Some(id) = model.saved.selected_task_id.clone() {
                                        model.select(TaskInfo {
                                            id: id.clone(),
                                            title: "Saved Codex task".into(),
                                        });
                                        ticket += 1;
                                        context_job = Some(read_context(&e, id, epoch, ticket, events.clone()));
                                    }
                                    engine = Some(e);
                                    if let Some(w) = app.get_webview_window("main") {
                                        let _ = w.set_always_on_top(model.view.settings.floating);
                                    }
                                }
                                Err(e) => {
                                    model.view.problem = Some(e);
                                    model.view.status = "Connection unavailable — your draft is editable".into();
                                }
                            }
                        }
                        Event::Tasks(rev, result, more) if rev == list_revision => {
                            model.view.loading_tasks = false;
                            match result {
                                Ok((tasks, next)) => {
                                    cursor = next;
                                    model.view.more = cursor.is_some();
                                    if more {
                                        for task in tasks {
                                            if !model.view.tasks.iter().any(|t| t.id == task.id) {
                                                model.view.tasks.push(task)
                                            }
                                        }
                                    } else {
                                        model.view.tasks = tasks;
                                    }
                                    if let Some(selected) = &model.view.selected {
                                        if let Some(task) = model.view.tasks.iter().find(|t| t.id == selected.id) {
                                            model.view.selected = Some(task.clone());
                                        }
                                    }
                                }
                                Err(e) => model.view.problem = Some(e),
                            }
                        }
                        Event::Context(rev, t, result) if rev == epoch && t >= applied => {
                            context_job = None;
                            applied = t;
                            match result {
                                Ok(ctx) => {
                                    if model.update_context(ctx) {
                                        abort(&mut generation);
                                        due = None;
                                        schedule(&model, &mut due);
                                    }
                                }
                                Err(e) => {
                                    model.context_failed(e);
                                    abort(&mut generation);
                                    due = None;
                                }
                            }
                        }
                        Event::Preflight(rev, t, g, result)
                            if rev == epoch && g.revision == model.view.revision =>
                        {
                            if t < applied {
                                model.invalidate();
                                generation = None;
                                schedule(&model, &mut due);
                            } else {
                                applied = t;
                                match result {
                                    Ok(ctx) if ctx == g.context && !ctx.active => {
                                        if let Some(e) = &engine {
                                            let e = e.clone();
                                            let tx = events.clone();
                                            generation = Some(tokio::spawn(async move {
                                                let start = Instant::now();
                                                let result = e
                                                    .generate(
                                                        &g.target,
                                                        &g.context,
                                                        &g.task.title,
                                                        &g.summary,
                                                        g.expand,
                                                        g.resolution.as_ref(),
                                                    )
                                                    .await;
                                                let _ = tx.send(Event::Generated(
                                                    g,
                                                    result,
                                                    start.elapsed().as_secs_f64(),
                                                ));
                                            }));
                                        }
                                    }
                                    Ok(ctx) => {
                                        model.update_context(ctx);
                                        generation = None;
                                        schedule(&model, &mut due);
                                    }
                                    Err(e) => {
                                        model.context_failed(e);
                                        generation = None;
                                    }
                                }
                            }
                        }
                        Event::Generated(g, result, latency) if g.revision == model.view.revision => {
                            let success = result.is_ok();
                            model.accept(&g, result, latency);
                            generation = None;
                            save(&store, &mut model);
                            if g.expand && success && model.view.phase == "idle" {
                                schedule(&model, &mut due);
                            }
                        }
                        _ => {}
                    }
                    publish(&app, &shared, &mut model);
                },
                _=clock.tick()=>{
                    if next_poll <= Instant::now() {
                        next_poll = Instant::now() + Duration::from_secs(2);
                        if context_job.as_ref().is_none_or(|j| j.is_finished()) {
                            if let (Some(e), Some(task)) = (&engine, &model.view.selected) {
                                ticket += 1;
                                context_job = Some(read_context(
                                    e,
                                    task.id.clone(),
                                    epoch,
                                    ticket,
                                    events.clone(),
                                ));
                            }
                        }
                    }
                    if due.is_some_and(|d| d <= Instant::now()) {
                        due = None;
                        if let Some(e) = &engine {
                            if let Some(g) = model.begin(false, None) {
                                abort(&mut generation);
                                ticket += 1;
                                generation = Some(preflight(e, g, epoch, ticket, events.clone()));
                                publish(&app, &shared, &mut model);
                            }
                        }
                    }
                }
            }
        }
        for job in [
            generation.take(),
            context_job.take(),
            connection.take(),
            listing.take(),
        ]
        .into_iter()
        .flatten()
        {
            job.abort();
            let _ = job.await;
        }
        drop(engine);
        save(&store, &mut model);
        finished.store(true, std::sync::atomic::Ordering::SeqCst);
        app.exit(0);
    });
    Service {
        paste_completed,
        tx,
        view,
        shutdown,
        exiting,
    }
}
