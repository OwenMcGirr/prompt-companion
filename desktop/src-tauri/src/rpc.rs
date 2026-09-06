use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
};
pub const DISABLED: &[&str] = &[
    "shell_tool",
    "unified_exec",
    "apps",
    "plugins",
    "computer_use",
    "browser_use",
    "browser_use_external",
    "code_mode",
    "code_mode_host",
    "code_mode_only",
    "multi_agent",
    "multi_agent_v2",
    "image_generation",
    "view_image",
    "goals",
    "hooks",
    "sleep_tool",
    "skill_search",
    "memories",
    "workspace_dependencies",
    "tool_suggest",
    "in_app_browser",
];
pub fn executable() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    #[cfg(target_os = "macos")]
    {
        candidates.extend([
            PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex"),
            PathBuf::from("/Applications/Codex.app/Contents/Resources/codex"),
        ]);
        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join("Applications/Codex.app/Contents/Resources/codex"));
        }
        candidates.extend([
            PathBuf::from("/opt/homebrew/bin/codex"),
            PathBuf::from("/usr/local/bin/codex"),
        ]);
    }
    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|p| p.join(name)));
    }
    #[cfg(windows)]
    {
        // npm installs a .cmd launcher; launch the packaged native binary directly,
        // never a shell or a command assembled from user text.
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let root = PathBuf::from(appdata).join("npm/node_modules/@openai");
            for folder in ["codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe","codex/vendor/x86_64-pc-windows-msvc/codex/codex.exe","codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe"]{candidates.push(root.join(folder));}
        }
    }
    candidates.into_iter().find(|p|p.is_file()).ok_or("Codex could not be found. Install the native Codex CLI on PATH, or the Codex Mac app, and reopen the preview.".into())
}
pub struct Rpc {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
    serial: u64,
    pub home: PathBuf,
    pub queued: Vec<Value>,
}
impl Drop for Rpc {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
impl Rpc {
    pub async fn start(dir: &Path) -> Result<Self, String> {
        let mut cmd = Command::new(executable()?);
        cmd.args(["app-server", "--listen", "stdio://"]);
        for feature in DISABLED {
            cmd.args(["-c", &format!("features.{feature}=false")]);
        }
        cmd.args([
            "-c",
            "notify=[]",
            "-c",
            "web_search=\"disabled\"",
            "-c",
            "project_doc_max_bytes=0",
        ]);
        cmd.current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Cannot start Codex: {e}"))?;
        let input = child.stdin.take().ok_or("Codex input unavailable")?;
        let output = BufReader::new(child.stdout.take().ok_or("Codex output unavailable")?).lines();
        let mut rpc = Self {
            child,
            input,
            output,
            serial: 0,
            home: PathBuf::new(),
            queued: Vec::new(),
        };
        let init=rpc.call("initialize",json!({"clientInfo":{"name":"prompt_companion_preview","title":"Prompt Companion Preview","version":"0.2.0"},"capabilities":{"experimentalApi":true}})).await?;
        rpc.home = init["codexHome"]
            .as_str()
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|p| p.join(".codex")))
            .ok_or("Codex home unavailable")?;
        rpc.send(json!({"method":"initialized","params":{}}))
            .await?;
        Ok(rpc)
    }
    async fn send(&mut self, v: Value) -> Result<(), String> {
        let mut data = serde_json::to_vec(&v).map_err(|e| e.to_string())?;
        data.push(b'\n');
        self.input
            .write_all(&data)
            .await
            .map_err(|_| "Codex disconnected".to_string())
    }
    pub async fn next(&mut self) -> Result<Value, String> {
        loop {
            let line = self
                .output
                .next_line()
                .await
                .map_err(|e| e.to_string())?
                .ok_or("Codex connection closed")?;
            let v: Value =
                serde_json::from_str(&line).map_err(|_| "Invalid app-server response")?;
            if v.get("method").is_some() && v.get("id").is_some() {
                self.send(json!({"id":v["id"],"error":{"code":-32601,"message":"Prompt composition does not permit tools or interactive requests."}})).await?;
                continue;
            }
            return Ok(v);
        }
    }
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.serial += 1;
        let id = self.serial;
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        tokio::time::timeout(Duration::from_secs(25), async {
            loop {
                let v = self.next().await?;
                if v["id"] == id {
                    if let Some(e) = v.get("error") {
                        return Err(e["message"]
                            .as_str()
                            .unwrap_or("Codex request failed")
                            .to_string());
                    }
                    return Ok(v["result"].clone());
                }
                if v.get("method").is_some() {
                    self.queued.push(v)
                }
            }
        })
        .await
        .map_err(|_| "Codex took too long. Your draft is safe.".to_string())?
    }
}
