use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
fn marker(bytes: &[u8]) -> Option<bool> {
    let v: Value = serde_json::from_slice(bytes).ok()?;
    if v["type"] != "event_msg" {
        return None;
    }
    match v["payload"]["type"].as_str() {
        Some("task_started") => Some(true),
        Some("task_complete" | "turn_aborted") => Some(false),
        _ => None,
    }
}
pub fn from_rollout(path: &Path) -> Result<Option<bool>, String> {
    let mut f = File::open(path).map_err(|_| "Cannot read task activity. Generation is paused.")?;
    let mut offset = f.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    let mut remainder = Vec::new();
    while offset > 0 {
        let len = offset.min(65536);
        offset -= len;
        f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
        let mut chunk = vec![0; len as usize];
        f.read_exact(&mut chunk).map_err(|e| e.to_string())?;
        chunk.extend(remainder);
        let lines: Vec<_> = chunk.split(|b| *b == b'\n').collect();
        for line in lines.iter().skip(1).rev() {
            if let Some(active) = marker(line) {
                return Ok(Some(active));
            }
        }
        remainder = lines[0].to_vec();
    }
    Ok(marker(&remainder))
}
pub fn active(thread: &Value, latest: Option<&Value>) -> Result<bool, String> {
    if thread["status"]["type"] == "active" || latest.is_some_and(|t| t["status"] == "inProgress") {
        return Ok(true);
    }
    match thread["path"].as_str() {
        Some(path) => Ok(from_rollout(Path::new(path))?.unwrap_or(false)),
        None => Ok(false),
    }
}
