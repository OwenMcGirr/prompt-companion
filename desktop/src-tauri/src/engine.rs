use crate::{
    activity,
    core::{self, Context, Resolution, Target},
    rpc::{Rpc, DISABLED},
    storage::write_private,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::Mutex;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
}
impl TaskInfo {
    fn parse(v: &Value) -> Option<Self> {
        if v["ephemeral"] == true {
            return None;
        }
        let id = v["id"].as_str()?.into();
        let title = v["name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| v["preview"].as_str().filter(|s| !s.is_empty()))
            .unwrap_or("Untitled task");
        Some(Self {
            id,
            title: core::prefix(title, 140),
        })
    }
}
#[derive(Clone)]
pub struct Engine {
    history: Arc<Mutex<Rpc>>,
    pub model: String,
    pub expansion_model: String,
    dir: PathBuf,
    config: Value,
}
fn remove_nulls(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), remove_nulls(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(
            a.iter()
                .filter(|v| !v.is_null())
                .map(remove_nulls)
                .collect(),
        ),
        _ => v.clone(),
    }
}
impl Engine {
    pub async fn connect(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let mut rpc = Rpc::start(dir).await?;
        let base = rpc
            .call("config/read", json!({"includeLayers":false}))
            .await?;
        let account = rpc
            .call("account/read", json!({"refreshToken":false}))
            .await?;
        if account["account"]["type"] != "chatgpt" {
            return Err("Sign in to Codex with ChatGPT, then Reconnect. API-key authentication is not supported.".into());
        }
        let models = rpc.call("model/list", json!({})).await?;
        let names: Vec<_> = models["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|m| m["model"].as_str())
            .collect();
        let model = if names.contains(&"gpt-5.6-luna") {
            "gpt-5.6-luna"
        } else {
            names
                .iter()
                .copied()
                .find(|n| {
                    ["luna", "mini", "spark"]
                        .iter()
                        .any(|part| n.contains(part))
                })
                .ok_or("No supported fast model available")?
        }
        .to_string();
        let expansion_model = if names.contains(&"gpt-5.6-sol") {
            "gpt-5.6-sol".to_string()
        } else {
            model.clone()
        };
        let raw = std::fs::read(rpc.home.join("models_cache.json"))
            .map_err(|_| "Codex model catalog unavailable. Open Codex once, then reconnect.")?;
        let catalog: Value =
            serde_json::from_slice(&raw).map_err(|_| "Unsupported model catalog")?;
        let entries: Vec<_> = catalog["models"]
            .as_array()
            .ok_or("Unsupported model catalog")?
            .iter()
            .filter(|m| m["slug"] == model || m["slug"] == expansion_model)
            .map(|m| {
                let mut m = m.clone();
                m["apply_patch_tool_type"] = Value::Null;
                m["experimental_supported_tools"] = json!([]);
                m["supports_search_tool"] = json!(false);
                m["node_repl_disabled"] = json!(true);
                m
            })
            .collect();
        if !entries.iter().any(|m| m["slug"] == model)
            || !entries.iter().any(|m| m["slug"] == expansion_model)
        {
            return Err("Selected models are missing from Codex metadata".into());
        }
        let path = dir.join("prediction-model.json");
        write_private(
            &path,
            &serde_json::to_vec(&json!({"models":entries})).map_err(|e| e.to_string())?,
        )?;
        let mut config = json!({"model_catalog_json":path,"model_provider":"openai","web_search":"disabled","project_doc_max_bytes":0,"notify":[],"developer_instructions":"","include_environment_context":false,"include_collaboration_mode_instructions":false,"include_apps_instructions":false,"include_permissions_instructions":false,"model_reasoning_effort":"low","service_tier":"default"});
        for feature in DISABLED {
            config[format!("features.{feature}")] = json!(false);
        }
        let mut servers = remove_nulls(&base["config"]["mcp_servers"]);
        if let Some(map) = servers.as_object_mut() {
            for server in map.values_mut() {
                server["enabled"] = json!(false);
            }
        } else {
            servers = json!({});
        }
        config["mcp_servers"] = servers;
        Ok(Self {
            history: Arc::new(Mutex::new(rpc)),
            model,
            expansion_model,
            dir: dir.into(),
            config,
        })
    }
    pub async fn tasks(
        &self,
        search: &str,
        cursor: Option<&str>,
    ) -> Result<(Vec<TaskInfo>, Option<String>), String> {
        let mut p = json!({"limit":60,"sortKey":"updated_at","useStateDbOnly":true,"sourceKinds":["appServer","cli","vscode","exec","unknown"]});
        if !search.is_empty() {
            p["searchTerm"] = json!(search);
        }
        if let Some(c) = cursor {
            p["cursor"] = json!(c);
        }
        let result = self.history.lock().await.call("thread/list", p).await?;
        Ok((
            result["data"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(TaskInfo::parse)
                .collect(),
            result["nextCursor"].as_str().map(str::to_string),
        ))
    }
    pub async fn context(&self, id: &str) -> Result<Context, String> {
        let mut rpc = self.history.lock().await;
        let meta = rpc
            .call("thread/read", json!({"threadId":id,"includeTurns":false}))
            .await?;
        let thread = &meta["thread"];
        if thread["historyMode"] == "paginated" {
            let page = rpc
                .call(
                    "thread/turns/list",
                    json!({"threadId":id,"limit":24,"sortDirection":"desc","itemsView":"full"}),
                )
                .await?;
            let recent = page["data"]
                .as_array()
                .ok_or("Invalid conversation response")?;
            let mut turns: Vec<_> = recent.iter().rev().cloned().collect();
            let partial = page["nextCursor"].is_string();
            if partial {
                let first = rpc
                    .call(
                        "thread/turns/list",
                        json!({"threadId":id,"limit":3,"sortDirection":"asc","itemsView":"full"}),
                    )
                    .await?;
                let mut older: Vec<_> = first["data"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|a| !turns.iter().any(|b| a["id"] == b["id"]))
                    .cloned()
                    .collect();
                older.extend(turns);
                turns = older;
            }
            Ok(Context {
                messages: core::messages(&turns),
                partial,
                active: activity::active(thread, recent.first())?,
            })
        } else {
            let full = rpc
                .call("thread/read", json!({"threadId":id,"includeTurns":true}))
                .await?;
            let turns = full["thread"]["turns"]
                .as_array()
                .ok_or("Invalid conversation response")?;
            Ok(Context {
                messages: core::messages(turns),
                partial: false,
                active: activity::active(&full["thread"], turns.last())?,
            })
        }
    }
    pub async fn generate(
        &self,
        target: &Target,
        context: &Context,
        title: &str,
        summary: &str,
        expand: bool,
        resolution: Option<&Resolution>,
    ) -> Result<Value, String> {
        let mut input = json!({"task_title":title,"earlier_summary":summary,"earlier_messages":if summary.is_empty(){context.earlier()}else{Vec::new()},"recent_messages":context.recent(),"history_is_partial":context.partial});
        let (instructions, schema, model) = if expand {
            input["current_shorthand"] = json!(target.draft.text);
            if let Some(r) = resolution {
                input["resolution"] = json!(r);
            }
            (
                include_str!("../prompts/expand.txt"),
                json!({"type":"object","additionalProperties":false,"properties":{"kind":{"type":"string","enum":["expanded","clarification"]},"prompt":{"type":"string"},"question":{"type":"string"},"choices":{"type":"array","items":{"type":"string"},"maxItems":3}},"required":["kind","prompt","question","choices"]}),
                &self.expansion_model,
            )
        } else {
            input["before_text"] = json!(core::suffix(&target.before, 5000));
            input["partial_word"] = json!(target.partial);
            input["selected_text"] = json!(target.selected);
            input["after_text"] = json!(core::prefix(&target.after, 3000));
            (
                include_str!("../prompts/phrases.txt"),
                json!({"type":"object","additionalProperties":false,"properties":{"suggestions":{"type":"array","items":{"type":"string"},"minItems":3,"maxItems":3},"context_summary":{"type":"string"}},"required":["suggestions","context_summary"]}),
                &self.model,
            )
        };
        let mut rpc = Rpc::start(&self.dir).await?;
        let result=rpc.call("thread/start",json!({"ephemeral":true,"cwd":self.dir,"sandbox":"read-only","approvalPolicy":"never","model":model,"baseInstructions":instructions,"developerInstructions":"Return only the requested prompt-composition JSON. Never perform the user's task.","config":self.config})).await?;
        let id = result["thread"]["id"]
            .as_str()
            .ok_or("No composition session returned")?
            .to_string();
        rpc.call("turn/start",json!({"threadId":id,"input":[{"type":"text","text":input.to_string()}],"effort":"low","outputSchema":schema})).await?;
        tokio::time::timeout(Duration::from_secs(35), async {
            let mut text = String::new();
            let queued = std::mem::take(&mut rpc.queued);
            let mut queue = std::collections::VecDeque::from(queued);
            loop {
                let v = if let Some(v) = queue.pop_front() {
                    v
                } else {
                    rpc.next().await?
                };
                let p = &v["params"];
                if p["threadId"] != id {
                    continue;
                }
                if v["method"] == "item/completed" && p["item"]["type"] == "agentMessage" {
                    text = p["item"]["text"].as_str().unwrap_or("").to_string();
                }
                if v["method"] == "turn/completed" {
                    if p["turn"]["status"] != "completed" {
                        return Err("Generation stopped. Your draft is unchanged.".into());
                    }
                    return serde_json::from_str(&text)
                        .map_err(|_| "Invalid generation output".into());
                }
            }
        })
        .await
        .map_err(|_| "Generation timed out. Your draft is safe.".to_string())?
    }
}
