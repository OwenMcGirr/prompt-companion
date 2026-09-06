pub mod activity;
pub mod core;
pub mod engine;
#[cfg(feature = "insertion-prototype")]
pub mod insertion;
pub mod model;
pub mod rpc;
mod runtime;
pub mod storage;
use tauri::Manager;
#[tauri::command]
fn snapshot(service: tauri::State<'_, runtime::Service>) -> Result<model::View, String> {
    service
        .view
        .lock()
        .map(|v| v.clone())
        .map_err(|_| "State unavailable".into())
}
#[tauri::command]
fn action(
    request: model::Request,
    service: tauri::State<'_, runtime::Service>,
) -> Result<(), String> {
    service
        .tx
        .send(request)
        .map_err(|_| "Composer has stopped".into())
}
#[tauri::command]
async fn insertion_probe(
    app: tauri::AppHandle,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    #[cfg(feature = "insertion-prototype")]
    {
        let request: insertion::Request =
            serde_json::from_value(request).map_err(|e| e.to_string())?;
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.run_on_main_thread(move || {
            let _ = tx
                .send(serde_json::to_value(insertion::execute(request)).map_err(|e| e.to_string()));
        })
        .map_err(|e| e.to_string())?;
        rx.await
            .map_err(|_| "Insertion experiment stopped".to_string())?
    }
    #[cfg(not(feature = "insertion-prototype"))]
    {
        let _ = (app, request);
        Ok(
            serde_json::json!({"available":false,"enabled":false,"destination":null,"message":"Direct insertion is not included in this preview."}),
        )
    }
}
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![action, snapshot, insertion_probe])
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            #[cfg(target_os = "macos")]
            let legacy = dirs::home_dir()
                .map(|p| p.join("Library/Application Support/PromptCompanion/drafts.json"));
            #[cfg(not(target_os = "macos"))]
            let legacy = None;
            let service = runtime::start(app.handle().clone(), dir, legacy);
            app.manage(service);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Unable to run Prompt Companion Preview")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let service = app.state::<runtime::Service>();
                if !service.exiting.load(std::sync::atomic::Ordering::SeqCst) {
                    api.prevent_exit();
                    let _ = service.shutdown.send(());
                }
            }
        });
}
