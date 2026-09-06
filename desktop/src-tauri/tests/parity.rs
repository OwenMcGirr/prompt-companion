use companion::{
    activity,
    core::*,
    engine::TaskInfo,
    model::Model,
    storage::{Saved, Store},
};
use serde_json::json;
fn task(id: &str) -> TaskInfo {
    TaskInfo {
        id: id.into(),
        title: id.into(),
    }
}
fn model() -> Model {
    let mut m = Model::new(Saved::default(), None);
    m.view.connected = true;
    m.view.connecting = false;
    m.view.settings.automatic = false;
    m.select(task("A"));
    m.update_context(Context {
        messages: vec![Message {
            role: "user".into(),
            text: "Make a text composer".into(),
        }],
        ..Context::default()
    });
    m
}
fn expanded(s: &str) -> serde_json::Value {
    json!({"kind":"expanded","prompt":s,"question":"","choices":[]})
}
fn choices() -> serde_json::Value {
    json!({"kind":"clarification","prompt":"","question":"Which issue?","choices":["Slow suggestions","Small buttons"]})
}
fn phrases() -> serde_json::Value {
    json!({"suggestions":["Fix login","Add tests","Explain error"],"context_summary":""})
}
#[test]
fn partial_word_preserves_case() {
    let t = Target::new(&Draft::at_end("Please Fi"));
    assert_eq!(t.partial, "Fi");
    assert_eq!(
        t.insert("fix the login error").unwrap().text,
        "Please Fix the login error "
    );
    assert!(t.insert("change layout").is_none());
}
#[test]
fn cursor_inside_token() {
    let t = Target::new(&Draft::new("fix logn tomorrow", 6, 0));
    assert_eq!(t.partial, "lo");
    assert_eq!(t.insert("login").unwrap().text, "fix login tomorrow");
}
#[test]
fn selection_preserves_suffix() {
    let t = Target::new(&Draft::new("Fix login, please.", 4, 5));
    assert_eq!(t.partial, "");
    assert_eq!(
        t.insert("the layout").unwrap().text,
        "Fix the layout, please."
    );
}
#[test]
fn emoji_offsets() {
    let t = Target::new(&Draft::at_end("🙂 fix "));
    let d = t.insert("the error").unwrap();
    assert_eq!(d.text, "🙂 fix the error ");
    assert_eq!(d.cursor, utf16(&d.text));
}
#[test]
fn empty_and_punctuation() {
    assert_eq!(
        Target::new(&Draft::default())
            .insert("Show options")
            .unwrap()
            .text,
        "Show options "
    );
    assert_eq!(
        Target::new(&Draft::at_end("Yes,"))
            .insert("please explain")
            .unwrap()
            .text,
        "Yes, please explain "
    );
}
#[test]
fn combining_accent() {
    let d = Draft::at_end("cafe\u{301}");
    let t = Target::new(&d);
    assert_eq!(t.partial, d.text);
    assert_eq!(
        t.insert("cafe\u{301} recommendations").unwrap().text,
        "cafe\u{301} recommendations "
    );
}
#[test]
fn invalid_utf16_offsets_are_clamped() {
    assert_eq!(Draft::new("🙂a", 1, 1), Draft::new("🙂a", 0, 2));
    assert_eq!(Draft::new("abc", 999, 999), Draft::at_end("abc"));
}
#[test]
fn phrase_validation() {
    let t = Target::new(&Draft::at_end("fix"));
    assert_eq!(
        t.phrases(
            &json!({"suggestions":["fix login","FIX LOGIN","write docs","fix\nlogin","fix layout"]})
        )
        .unwrap(),
        vec!["fix login", "fix layout"]
    );
    assert!(t.phrases(&json!({"suggestions":["unrelated"]})).is_err());
}
#[test]
fn context_excludes_tools_and_images() {
    let turns = vec![
        json!({"items":[{"type":"userMessage","content":[{"type":"text","text":"Fix error"},{"type":"image","url":"private"}]},{"type":"reasoning","text":"hidden"},{"type":"commandExecution","aggregatedOutput":"secret"},{"type":"agentMessage","text":"Login fails"}]}),
    ];
    let m = messages(&turns);
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].text, "Fix error");
    assert_eq!(m[1].text, "Login fails");
}
#[test]
fn context_budget_preserves_recent() {
    let m = vec![
        Message {
            role: "user".into(),
            text: "a".repeat(200),
        },
        Message {
            role: "assistant".into(),
            text: "Recent correction".into(),
        },
    ];
    let b = bounded(&m, 40);
    assert!(b.iter().map(|m| count(&m.text)).sum::<usize>() <= 40);
    assert_eq!(b.last().unwrap().text, "Recent correction");
}
#[test]
fn opening_goal_survives() {
    let goal = Message {
        role: "user".into(),
        text: "Keep buttons large".into(),
    };
    let mut messages = vec![goal.clone()];
    messages.extend((0..60).map(|_| Message {
        role: "assistant".into(),
        text: "x".repeat(3000),
    }));
    let c = Context {
        messages,
        partial: true,
        active: false,
    };
    assert_eq!(c.earlier()[0], goal);
    assert!(c.earlier().iter().map(|m| count(&m.text)).sum::<usize>() <= 8000);
}
#[test]
fn expansion_validation() {
    assert_eq!(
        expansion(&expanded("Larger buttons, same layout."), false).unwrap(),
        Expansion::Expanded("Larger buttons, same layout.".into())
    );
    assert!(matches!(
        expansion(&choices(), false),
        Ok(Expansion::Clarification(_))
    ));
    assert!(expansion(&choices(), true).is_err());
    for v in [
        expanded(""),
        expanded(&"word ".repeat(181)),
        json!({"kind":"expanded","prompt":"Change","question":"Which?","choices":[]}),
        json!({"kind":"clarification","prompt":"","question":"Which?","choices":["same","SAME"]}),
        json!({"kind":"clarification","prompt":"","question":"Which?","choices":["only"]}),
        json!({}),
    ] {
        assert!(expansion(&v, false).is_err());
    }
}
#[test]
fn late_prediction_after_edit() {
    let mut m = model();
    let g = m.begin(false, None).unwrap();
    m.edit(Draft::at_end("No"));
    m.accept(&g, Ok(phrases()), 0.1);
    m.insert(0, g.revision);
    assert_eq!(m.view.draft.text, "No");
    assert!(!m.view.can_insert);
}
#[test]
fn selection_change_invalidates_result() {
    let mut m = model();
    m.edit(Draft::at_end("fix"));
    let g = m.begin(false, None).unwrap();
    m.edit(Draft::new("fix", 0, 0));
    m.accept(&g, Ok(json!({"suggestions":["fix login"]})), 0.1);
    assert!(!m.view.can_insert);
}
#[test]
fn hover_holds_results() {
    let mut m = model();
    m.hover(true);
    let g = m.begin(false, None).unwrap();
    m.accept(&g, Ok(phrases()), 0.1);
    assert!(m.view.phrases.is_empty());
    m.hover(false);
    assert_eq!(m.view.phrases[0], "Fix login");
    assert!(m.view.can_insert);
    m.insert(0, m.view.revision);
    assert_eq!(m.view.draft.text, "Fix login ");
    assert!(m.view.focus > 0);
    m.undo();
    assert_eq!(m.view.draft.text, "");
}
#[test]
fn partial_word_reuse() {
    let mut m = model();
    m.edit(Draft::at_end("f"));
    let g = m.begin(false, None).unwrap();
    m.accept(
        &g,
        Ok(json!({"suggestions":["fix login","fix errors","fix layout"]})),
        0.1,
    );
    m.edit(Draft::at_end("fi"));
    assert!(m.view.can_insert);
    assert!(m.view.phrases.iter().all(|s| s.starts_with("fi")));
}
#[test]
fn task_switch_isolates_drafts_and_undo() {
    let mut m = model();
    m.edit(Draft::at_end("A draft"));
    let g = m.begin(true, None).unwrap();
    m.select(task("B"));
    m.update_context(Context::default());
    m.edit(Draft::at_end("B draft"));
    m.accept(&g, Ok(expanded("stale")), 0.1);
    assert_eq!(m.view.draft.text, "B draft");
    m.select(task("A"));
    assert_eq!(m.view.draft.text, "A draft");
    m.undo();
    assert_eq!(m.view.draft.text, "");
}
#[test]
fn clear_undo_selection() {
    let mut m = model();
    let original = Draft::new("hello world", 6, 5);
    m.edit(original.clone());
    m.clear();
    assert_eq!(m.view.draft, Draft::default());
    m.undo();
    assert_eq!(m.view.draft, original);
}
#[test]
fn successful_copy_clears_and_undo_restores() {
    let mut m = model();
    let d = Draft::at_end("hello");
    m.edit(d.clone());
    m.copy_result(true);
    assert!(m.view.copied);
    assert_eq!(m.view.draft, Draft::default());
    m.undo();
    assert_eq!(m.view.draft, d);
}
#[test]
fn copy_failure_keeps_draft() {
    let mut m = model();
    m.edit(Draft::at_end("hello"));
    m.copy_result(false);
    assert_eq!(m.view.draft.text, "hello");
    assert!(!m.view.copied);
    assert!(m.view.problem.is_some());
}
#[test]
fn empty_copy_does_nothing() {
    let mut m = model();
    let revision = m.view.revision;
    m.copy_result(true);
    assert_eq!(m.view.revision, revision);
    assert!(!m.view.copied);
}
#[test]
fn expansion_is_one_undoable_edit() {
    let mut m = model();
    let original = Draft::new("bigger buttons same layout", 7, 7);
    m.edit(original.clone());
    let focus = m.view.focus;
    let g = m.begin(true, None).unwrap();
    assert_eq!(m.view.draft, original);
    m.accept(
        &g,
        Ok(expanded("Make the buttons larger. Keep their arrangement.")),
        0.1,
    );
    assert!(m.view.focus > focus);
    assert_eq!(m.view.draft.cursor, utf16(&m.view.draft.text));
    m.undo();
    assert_eq!(m.view.draft, original);
}
#[test]
fn clarification_preserves_draft_and_pauses_phrases() {
    let mut m = model();
    m.edit(Draft::at_end("fix it"));
    let g = m.begin(true, None).unwrap();
    m.accept(&g, Ok(choices()), 0.1);
    assert_eq!(m.view.draft.text, "fix it");
    assert!(!m.view.can_insert);
    assert!(m.begin(false, None).is_none());
    let r = m.resolution(1).unwrap();
    assert_eq!(r.choice, "Small buttons");
    let next = m.begin(true, Some(r)).unwrap();
    assert!(m.begin(true, None).is_none());
    m.accept(&next, Ok(expanded("Make the buttons larger.")), 0.1);
    assert_eq!(m.view.draft.text, "Make the buttons larger.");
}
#[test]
fn hover_holds_clarification() {
    let mut m = model();
    m.edit(Draft::at_end("fix it"));
    m.hover(true);
    let g = m.begin(true, None).unwrap();
    m.accept(&g, Ok(choices()), 0.1);
    assert!(m.view.clarification.is_none());
    m.hover(false);
    assert!(m.view.clarification.is_some());
}
#[test]
fn keep_original_rejects_late_expansion() {
    let mut m = model();
    m.edit(Draft::at_end("bigger"));
    let g = m.begin(true, None).unwrap();
    m.invalidate();
    m.accept(&g, Ok(expanded("stale")), 0.1);
    assert_eq!(m.view.draft.text, "bigger");
    assert!(m.begin(false, None).is_some());
}
#[test]
fn edit_switch_clear_copy_cancel_expansion() {
    for action in ["edit", "switch", "clear", "copy"] {
        let mut m = model();
        m.edit(Draft::at_end("bigger"));
        let g = m.begin(true, None).unwrap();
        match action {
            "edit" => m.edit(Draft::at_end("why slow")),
            "switch" => m.select(task("B")),
            "clear" => m.clear(),
            _ => m.copy_result(true),
        }
        let expected = m.view.draft.clone();
        m.accept(&g, Ok(expanded("stale")), 0.1);
        assert_eq!(m.view.draft, expected);
    }
}
#[test]
fn context_change_cancels_expansion() {
    let mut m = model();
    m.edit(Draft::at_end("bigger"));
    let g = m.begin(true, None).unwrap();
    m.update_context(Context {
        messages: vec![Message {
            role: "user".into(),
            text: "new correction".into(),
        }],
        ..Context::default()
    });
    m.accept(&g, Ok(expanded("stale")), 0.1);
    assert_eq!(m.view.draft.text, "bigger");
    assert!(m.view.status.contains("Conversation changed"));
}
#[test]
fn failure_allows_retry() {
    let mut m = model();
    m.edit(Draft::at_end("bigger"));
    let g = m.begin(true, None).unwrap();
    m.accept(&g, Err("Offline".into()), 0.1);
    assert_eq!(m.view.draft.text, "bigger");
    assert!(m.view.can_expand);
    assert!(m.view.problem.is_some());
    assert!(m.begin(true, None).is_some());
}
#[test]
fn no_second_clarification() {
    let mut m = model();
    m.edit(Draft::at_end("fix it"));
    let g = m.begin(true, None).unwrap();
    m.accept(&g, Ok(choices()), 0.1);
    let r = m.resolution(0).unwrap();
    let g = m.begin(true, Some(r)).unwrap();
    m.accept(&g, Ok(choices()), 0.1);
    assert_eq!(m.view.draft.text, "fix it");
    assert!(m.view.problem.is_some());
    assert_eq!(m.view.phase, "idle");
}
#[test]
fn expanded_copy_clear_undo_chain() {
    let mut m = model();
    m.edit(Draft::at_end("bigger"));
    let g = m.begin(true, None).unwrap();
    m.accept(&g, Ok(expanded("Larger buttons.")), 0.1);
    m.copy_result(true);
    m.undo();
    assert_eq!(m.view.draft.text, "Larger buttons.");
    m.undo();
    assert_eq!(m.view.draft.text, "bigger");
}
#[test]
fn old_phrase_cannot_interrupt_expansion() {
    let mut m = model();
    m.edit(Draft::at_end("bigger"));
    let old = m.begin(false, None).unwrap();
    m.invalidate();
    let next = m.begin(true, None).unwrap();
    m.accept(&old, Ok(phrases()), 0.1);
    assert_eq!(m.view.phase, "expanding");
    m.accept(&next, Ok(expanded("Larger buttons.")), 0.1);
    assert_eq!(m.view.draft.text, "Larger buttons.");
}
#[test]
fn active_task_blocks_and_keeps_draft() {
    let mut m = model();
    m.edit(Draft::at_end("keep this"));
    let mut ctx = m.context.clone().unwrap();
    ctx.active = true;
    m.update_context(ctx.clone());
    assert!(!m.view.can_expand);
    assert!(m.begin(false, None).is_none());
    assert!(m.begin(true, None).is_none());
    assert_eq!(m.view.draft.text, "keep this");
    ctx.active = false;
    m.update_context(ctx);
    assert!(m.view.can_expand);
    assert!(!m.view.settings.automatic);
}
#[test]
fn activity_cancels_pending_and_late() {
    let mut m = model();
    let g = m.begin(false, None).unwrap();
    m.hover(true);
    m.accept(&g, Ok(phrases()), 0.1);
    let mut ctx = m.context.clone().unwrap();
    ctx.active = true;
    m.update_context(ctx.clone());
    m.hover(false);
    assert!(!m.view.can_insert);
    assert!(m.view.phrases.is_empty());
    ctx.active = false;
    m.update_context(ctx.clone());
    let g = m.begin(false, None).unwrap();
    ctx.active = true;
    m.update_context(ctx);
    m.accept(&g, Ok(phrases()), 0.1);
    assert!(!m.view.can_insert);
}
#[test]
fn active_cancels_expansion() {
    let mut m = model();
    m.edit(Draft::at_end("bigger"));
    let g = m.begin(true, None).unwrap();
    let mut ctx = m.context.clone().unwrap();
    ctx.active = true;
    m.update_context(ctx);
    m.accept(&g, Ok(expanded("late")), 0.1);
    assert_eq!(m.view.draft.text, "bigger");
    assert!(!m.view.can_expand);
}
#[test]
fn failed_activity_check_blocks_generation() {
    let mut m = model();
    m.context_failed("unavailable".into());
    assert!(m.begin(false, None).is_none());
    assert!(!m.view.can_insert);
}
#[test]
fn activity_metadata() {
    assert!(activity::active(&json!({"status":{"type":"active"}}), None).unwrap());
    assert!(activity::active(
        &json!({"status":{"type":"notLoaded"}}),
        Some(&json!({"status":"inProgress"}))
    )
    .unwrap());
    for s in ["completed", "interrupted", "failed"] {
        assert!(!activity::active(
            &json!({"status":{"type":"notLoaded"}}),
            Some(&json!({"status":s}))
        )
        .unwrap());
    }
}
#[test]
fn lifecycle_reverse_chunks() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("history");
    let marker = |s| format!("{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"{s}\"}}}}\n");
    let unrelated = format!(
        "{{\"type\":\"response_item\",\"text\":\"{}\"}}\n",
        "x".repeat(140000)
    );
    for terminal in ["task_complete", "turn_aborted"] {
        std::fs::write(
            &p,
            format!(
                "{}{}{}",
                marker("task_started"),
                unrelated,
                marker(terminal)
            ),
        )
        .unwrap();
        assert_eq!(activity::from_rollout(&p).unwrap(), Some(false));
    }
    std::fs::write(
        &p,
        format!("{}{}{{\"type\":", marker("task_started"), unrelated),
    )
    .unwrap();
    assert_eq!(activity::from_rollout(&p).unwrap(), Some(true));
    std::fs::write(&p, unrelated).unwrap();
    assert_eq!(activity::from_rollout(&p).unwrap(), None);
}
#[test]
fn missing_history_is_error() {
    assert!(activity::from_rollout(std::path::Path::new("/no-such-history-file")).is_err());
}
#[test]
fn persistence_and_import_once() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = dir.path().join("old.json");
    let raw=json!({"selectedTaskID":"A","drafts":{"A":{"text":"🙂 draft","cursor":999,"selectionLength":999}},"fontSize":99,"buttonHeight":86,"automatic":false,"floating":true}).to_string();
    std::fs::write(&legacy, &raw).unwrap();
    let (store, saved, error) = Store::load(dir.path().join("preview"), Some(&legacy));
    assert!(error.is_none(), "{error:?}");
    assert_eq!(saved.selected_task_id.as_deref(), Some("A"));
    assert_eq!(saved.settings.font_size, 32.);
    assert_eq!(saved.drafts["A"], Draft::at_end("🙂 draft"));
    assert_eq!(std::fs::read_to_string(&legacy).unwrap(), raw);
    assert_eq!(
        std::fs::read_to_string(store.dir.join("swift-drafts-backup.json")).unwrap(),
        raw
    );
    std::fs::write(&legacy, "broken").unwrap();
    let (_, restored, error) = Store::load(store.dir.clone(), Some(&legacy));
    assert!(error.is_none());
    assert_eq!(restored.drafts, saved.drafts);
}
#[test]
fn corrupt_or_future_storage_is_never_overwritten() {
    for raw in [
        "broken".into(),
        json!({"version":9,"selectedTaskId":null,"drafts":{},"settings":{}}).to_string(),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("state.json");
        std::fs::write(&p, &raw).unwrap();
        let (store, saved, error) = Store::load(dir.path().into(), None);
        assert!(error.is_some());
        assert!(store.save(&saved).is_err());
        assert_eq!(std::fs::read_to_string(p).unwrap(), raw);
    }
}
#[test]
fn store_failure_preserves_memory() {
    let dir = tempfile::tempdir().unwrap();
    let (store, _, _) = Store::load(dir.path().join("data"), None);
    std::fs::remove_dir_all(&store.dir).unwrap();
    std::fs::write(&store.dir, "not a directory").unwrap();
    let mut m = model();
    m.edit(Draft::at_end("keep me"));
    assert!(store.save(&m.saved).is_err());
    assert_eq!(m.view.draft.text, "keep me");
}
#[cfg(unix)]
#[test]
fn storage_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let (store, _, _) = Store::load(dir.path().join("data"), None);
    assert_eq!(
        std::fs::metadata(store.dir.join("state.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn edits_during_startup_stay_with_the_saved_task() {
    let mut saved = Saved {
        selected_task_id: Some("A".into()),
        ..Saved::default()
    };
    saved.drafts.insert("A".into(), Draft::at_end("old words"));
    let mut m = Model::new(saved, None);
    m.edit(Draft::at_end("new words before connecting"));
    assert_eq!(m.saved.drafts["A"].text, "new words before connecting");
    m.select(task("A"));
    assert_eq!(m.view.draft.text, "new words before connecting");
    assert!(!m.saved.drafts.contains_key("__unassigned__"));
}
#[test]
fn successful_context_refresh_clears_transient_failure() {
    let mut m = model();
    let context = m.context.clone().unwrap();
    m.context_failed("Temporary read failure".into());
    assert!(m.view.problem.is_some());
    m.update_context(context);
    assert!(m.view.problem.is_none());
    m.edit(Draft::at_end("explain this"));
    assert!(m.view.can_expand);
}
