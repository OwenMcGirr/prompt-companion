use crate::{
    core::{self, Clarification, Context, Draft, Expansion, Resolution, Target},
    engine::TaskInfo,
    storage::{Saved, Settings},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub draft: Draft,
    pub selected: Option<TaskInfo>,
    pub tasks: Vec<TaskInfo>,
    pub more: bool,
    pub loading_tasks: bool,
    pub settings: Settings,
    pub revision: u64,
    pub acknowledged: u64,
    pub focus: u64,
    pub connected: bool,
    pub connecting: bool,
    pub status: String,
    pub problem: Option<String>,
    pub storage_problem: Option<String>,
    pub context_status: String,
    pub active: bool,
    pub phrases: Vec<String>,
    pub can_insert: bool,
    pub can_expand: bool,
    pub phase: String,
    pub clarification: Option<Clarification>,
    pub copied: bool,
    pub undo_available: bool,
    pub typed: usize,
    pub inserted: usize,
    pub accepted: usize,
    pub latency: Option<f64>,
    pub model: String,
    pub expansion_model: String,
}
#[derive(Clone)]
struct Batch {
    revision: u64,
    target: Target,
    phrases: Vec<String>,
}
#[derive(Clone)]
pub struct Generation {
    pub revision: u64,
    pub target: Target,
    pub task: TaskInfo,
    pub context: Context,
    pub summary: String,
    pub expand: bool,
    pub resolution: Option<Resolution>,
}
pub struct Model {
    pub view: View,
    pub saved: Saved,
    pub context: Option<Context>,
    pub summary: String,
    batch: Option<Batch>,
    pending: Option<Batch>,
    pending_choices: Option<Clarification>,
    hover: bool,
    undo: HashMap<String, Vec<Draft>>,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    Edit { draft: Draft },
    Select { id: String },
    Insert { index: usize, revision: u64 },
    Undo,
    Clear,
    Copy,
    Expand { revision: u64 },
    Choose { index: usize, revision: u64 },
    KeepOriginal,
    Hover { value: bool },
    Refresh,
    Reconnect,
    Tasks { search: String, more: bool },
    Settings { settings: Settings },
}
#[derive(Debug, Deserialize)]
pub struct Request {
    pub sequence: u64,
    pub action: Action,
}
impl Model {
    pub fn new(saved: Saved, error: Option<String>) -> Self {
        let draft = saved
            .drafts
            .get(
                saved
                    .selected_task_id
                    .as_deref()
                    .unwrap_or("__unassigned__"),
            )
            .cloned()
            .unwrap_or_default();
        let view = View {
            draft,
            settings: saved.settings.clone(),
            selected: saved.selected_task_id.as_ref().map(|id| TaskInfo {
                id: id.clone(),
                title: "Saved Codex task".into(),
            }),
            tasks: Vec::new(),
            more: false,
            loading_tasks: false,
            revision: 0,
            acknowledged: 0,
            focus: 0,
            connected: false,
            connecting: true,
            status: "Connecting to Codex…".into(),
            problem: None,
            storage_problem: error,
            context_status: "Choose a task for relevant suggestions".into(),
            active: false,
            phrases: Vec::new(),
            can_insert: false,
            can_expand: false,
            phase: "idle".into(),
            clarification: None,
            copied: false,
            undo_available: false,
            typed: 0,
            inserted: 0,
            accepted: 0,
            latency: None,
            model: String::new(),
            expansion_model: String::new(),
        };
        Self {
            view,
            saved,
            context: None,
            summary: String::new(),
            batch: None,
            pending: None,
            pending_choices: None,
            hover: false,
            undo: HashMap::new(),
        }
    }
    fn key(&self) -> String {
        self.view
            .selected
            .as_ref()
            .map(|t| t.id.clone())
            .unwrap_or_else(|| "__unassigned__".into())
    }
    pub fn sync(&mut self) {
        self.view.active = self.context.as_ref().is_some_and(|c| c.active);
        self.view.can_insert = !self.view.active
            && self.view.phase == "idle"
            && self.context.is_some()
            && self.batch.as_ref().is_some_and(|b| {
                b.revision == self.view.revision
                    && b.target.draft == self.view.draft
                    && !b.phrases.is_empty()
            });
        self.view.can_expand = self.view.connected
            && !self.view.connecting
            && !["expanding", "clarification"].contains(&self.view.phase.as_str())
            && !self.view.active
            && self.context.is_some()
            && !self.view.draft.text.trim().is_empty();
        self.view.undo_available = self.undo.get(&self.key()).is_some_and(|h| !h.is_empty());
        self.saved.settings = self.view.settings.clone();
        self.saved
            .drafts
            .insert(self.key(), self.view.draft.clone());
    }
    pub fn invalidate(&mut self) {
        self.view.revision += 1;
        self.view.phase = "idle".into();
        self.view.clarification = None;
        self.pending_choices = None;
        self.pending = None;
        self.sync();
    }
    fn remember(&mut self) {
        let d = self.view.draft.clone();
        let h = self.undo.entry(self.key()).or_default();
        h.push(d);
        if h.len() > 100 {
            h.remove(0);
        }
    }
    pub fn edit(&mut self, draft: Draft) {
        let draft = draft.normalized();
        if draft == self.view.draft {
            return;
        }
        let old = self
            .batch
            .clone()
            .filter(|b| b.revision == self.view.revision && b.target.draft == self.view.draft);
        let target = Target::new(&self.view.draft);
        if draft.text != self.view.draft.text {
            self.remember();
            self.view.typed +=
                core::count(&draft.text).saturating_sub(core::count(&self.view.draft.text));
        }
        self.invalidate();
        self.view.draft = draft;
        self.view.copied = false;
        let next = Target::new(&self.view.draft);
        if let Some(old) = old {
            if next.before == target.before
                && next.after == target.after
                && !next.partial.is_empty()
                && next.partial.starts_with(&target.partial)
            {
                let phrases = old
                    .phrases
                    .iter()
                    .filter_map(|s| next.normalize(s))
                    .collect();
                self.present(Batch {
                    revision: self.view.revision,
                    target: next,
                    phrases,
                });
            }
        }
        self.sync();
    }
    pub fn select(&mut self, task: TaskInfo) {
        self.sync();
        let unassigned = if self.view.selected.is_none() {
            self.view.draft.clone()
        } else {
            Draft::default()
        };
        self.invalidate();
        self.context = None;
        self.summary.clear();
        self.view.phrases.clear();
        self.batch = None;
        self.view.draft = self
            .saved
            .drafts
            .get(&task.id)
            .cloned()
            .unwrap_or(unassigned);
        if self.view.selected.is_none() {
            self.saved
                .drafts
                .insert("__unassigned__".into(), Draft::default());
        }
        self.saved.selected_task_id = Some(task.id.clone());
        self.view.selected = Some(task);
        self.view.focus += 1;
        self.view.context_status = "Reading this task…".into();
        self.view.status = "Loading conversation…".into();
        self.view.problem = None;
        self.sync();
    }
    pub fn update_context(&mut self, context: Context) -> bool {
        self.view.problem = None;
        self.view.context_status = if context.active {
            "Task active — generation paused"
        } else if context.partial {
            "Recent conversation + opening messages"
        } else {
            "Conversation connected"
        }
        .into();
        if self.context.as_ref() == Some(&context) {
            return false;
        }
        let expanding = self.view.phase == "expanding" || self.view.phase == "clarification";
        self.invalidate();
        self.summary.clear();
        self.context = Some(context);
        self.batch = None;
        self.view.phrases.clear();
        self.sync();
        self.view.status = if self.view.active {
            "Task active — suggestions paused until it finishes"
        } else if expanding {
            "Conversation changed — click Expand again to use the latest context"
        } else {
            "Ready when you are"
        }
        .into();
        true
    }
    pub fn context_failed(&mut self, error: String) {
        self.invalidate();
        self.context = None;
        self.batch = None;
        self.view.phrases.clear();
        self.view.context_status = "Activity check unavailable — generation paused".into();
        self.view.problem = Some(error);
        self.sync();
    }
    pub fn insert(&mut self, index: usize, revision: u64) {
        self.sync();
        if revision != self.view.revision || !self.view.can_insert {
            return;
        }
        if let Some(next) = self
            .batch
            .as_ref()
            .and_then(|b| b.phrases.get(index).and_then(|s| b.target.insert(s)))
        {
            self.remember();
            self.view.inserted +=
                core::count(&next.text).saturating_sub(core::count(&self.view.draft.text));
            self.view.accepted += 1;
            self.invalidate();
            self.view.draft = next;
            self.view.copied = false;
            self.view.focus += 1;
            self.sync();
        }
    }
    pub fn undo(&mut self) {
        let key = self.key();
        if let Some(d) = self.undo.get_mut(&key).and_then(Vec::pop) {
            self.invalidate();
            self.view.draft = d;
            self.view.copied = false;
            self.view.focus += 1;
            self.sync();
        }
    }
    pub fn clear(&mut self) {
        if !self.view.draft.text.is_empty() {
            self.remember();
            self.invalidate();
            self.view.draft = Draft::default();
            self.view.copied = false;
            self.view.focus += 1;
            self.sync();
        }
    }
    pub fn copy_result(&mut self, ok: bool) {
        if self.view.draft.text.trim().is_empty() {
            return;
        }
        self.invalidate();
        if ok {
            self.clear();
            self.view.copied = true;
            self.view.problem = None;
            self.view.status = "Copied — paste into your Codex prompt".into();
        } else {
            self.view.copied = false;
            self.view.problem =
                Some("Couldn’t copy. Your draft has been kept; try Copy Prompt again.".into());
        }
        self.sync();
    }
    pub fn hover(&mut self, value: bool) {
        self.hover = value;
        if !value {
            if let Some(b) = self.pending.take() {
                self.present(b)
            }
            if let Some(c) = self.pending_choices.take() {
                self.view.clarification = Some(c);
            }
        }
        self.sync();
    }
    fn present(&mut self, b: Batch) {
        if b.revision != self.view.revision || b.target.draft != self.view.draft || self.view.active
        {
            return;
        }
        if self.hover {
            self.pending = Some(b);
            return;
        }
        self.view.phrases = b.phrases.clone();
        self.batch = Some(b);
        self.sync();
    }
    pub fn begin(&mut self, expand: bool, resolution: Option<Resolution>) -> Option<Generation> {
        self.sync();
        if !self.view.connected || self.view.active || self.context.is_none() {
            return None;
        }
        if expand {
            if resolution.is_none() && !self.view.can_expand {
                return None;
            }
            if resolution.is_some() && self.view.phase != "clarification" {
                return None;
            }
        } else if self.view.phase == "expanding" || self.view.phase == "clarification" {
            return None;
        }
        self.invalidate();
        self.view.phase = if expand { "expanding" } else { "predicting" }.into();
        self.view.status = if expand {
            "Expanding your words…"
        } else {
            "Thinking of useful phrases…"
        }
        .into();
        self.view.problem = None;
        self.sync();
        Some(Generation {
            revision: self.view.revision,
            target: Target::new(&self.view.draft),
            task: self.view.selected.clone()?,
            context: self.context.clone()?,
            summary: self.summary.clone(),
            expand,
            resolution,
        })
    }
    pub fn accept(&mut self, g: &Generation, result: Result<Value, String>, latency: f64) {
        if g.revision != self.view.revision
            || g.target.draft != self.view.draft
            || self.view.selected.as_ref().map(|t| &t.id) != Some(&g.task.id)
            || self.view.active
        {
            return;
        }
        let outcome = result.and_then(|v| {
            if g.expand {
                core::expansion(&v, g.resolution.is_some())
                    .map(|e| (Some(e), Vec::new(), String::new()))
            } else {
                g.target.phrases(&v).map(|p| {
                    (
                        None,
                        p,
                        core::prefix(v["context_summary"].as_str().unwrap_or(""), 2000),
                    )
                })
            }
        });
        match outcome {
            Ok((Some(Expansion::Expanded(text)), _, _)) => {
                self.remember();
                self.view.inserted +=
                    core::count(&text).saturating_sub(core::count(&self.view.draft.text));
                self.invalidate();
                self.view.draft = Draft::at_end(text);
                self.view.focus += 1;
                self.view.copied = false;
                self.view.status =
                    "Expanded — edit, copy, or Undo to restore your shorthand".into();
            }
            Ok((Some(Expansion::Clarification(c)), _, _)) => {
                self.view.phase = "clarification".into();
                self.view.status = c.question.clone();
                if self.hover {
                    self.pending_choices = Some(c)
                } else {
                    self.view.clarification = Some(c)
                }
            }
            Ok((None, phrases, summary)) => {
                self.view.phase = "idle".into();
                self.summary = summary;
                self.present(Batch {
                    revision: g.revision,
                    target: g.target.clone(),
                    phrases,
                });
                self.view.status = if self.hover {
                    "Phrases ready — move off the buttons to show them"
                } else {
                    "Choose a phrase or keep typing"
                }
                .into();
            }
            Err(e) => {
                self.invalidate();
                self.view.problem = Some(e);
                self.view.status = "Your draft is unchanged — try again when ready".into();
            }
        }
        self.view.latency = Some(latency);
        self.sync();
    }
    pub fn resolution(&self, index: usize) -> Option<Resolution> {
        let c = self.view.clarification.as_ref()?;
        Some(Resolution {
            question: c.question.clone(),
            choice: c.choices.get(index)?.clone(),
        })
    }
}
