pub mod activity;
pub mod core;
pub mod engine;
#[cfg(any(target_os = "macos", feature = "insertion-prototype"))]
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
    app: tauri::AppHandle,
    service: tauri::State<'_, runtime::Service>,
) -> Result<(), String> {
    #[cfg(any(target_os = "macos", feature = "insertion-prototype"))]
    if !matches!(request.action, model::Action::Hover { .. }) {
        app.run_on_main_thread(insertion::cancel_pending)
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "macos", feature = "insertion-prototype")))]
    let _ = app;
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
    #[cfg(any(target_os = "macos", feature = "insertion-prototype"))]
    {
        let request: insertion::Request =
            serde_json::from_value(request).map_err(|e| e.to_string())?;
        #[cfg(not(feature = "insertion-prototype"))]
        if !matches!(
            request,
            insertion::Request::Status
                | insertion::Request::Cancel
                | insertion::Request::ArmPaste { .. }
        ) {
            return Err("This insertion method is unavailable.".into());
        }
        let (tx, rx) = tokio::sync::oneshot::channel();
        let probe_app = app.clone();
        app.run_on_main_thread(move || {
            let _ = tx.send(
                serde_json::to_value(insertion::execute(request, probe_app))
                    .map_err(|e| e.to_string()),
            );
        })
        .map_err(|e| e.to_string())?;
        rx.await
            .map_err(|_| "Insertion experiment stopped".to_string())?
    }
    #[cfg(not(any(target_os = "macos", feature = "insertion-prototype")))]
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
            let service = runtime::start(app.handle().clone(), dir);
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
