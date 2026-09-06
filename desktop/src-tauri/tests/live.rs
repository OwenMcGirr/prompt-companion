use companion::{
    core::{expansion, Context, Draft, Expansion, Message, Resolution, Target},
    engine::Engine,
};
fn enabled() {
    assert_eq!(
        std::env::var("PROMPT_COMPANION_LIVE_TEST").as_deref(),
        Ok("1"),
        "Opt-in test uses Codex allowance. Set PROMPT_COMPANION_LIVE_TEST=1."
    );
}
#[tokio::test]
#[ignore = "uses existing Codex ChatGPT account and allowance"]
async fn live_prediction() {
    enabled();
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::connect(dir.path()).await.unwrap();
    let (tasks, _) = e.tasks("", None).await.unwrap();
    assert!(!tasks.is_empty());
    if let Ok(id) = std::env::var("PROMPT_COMPANION_TEST_TASK") {
        let _ = e.context(&id).await.unwrap();
    }
    let c=Context{messages:vec![Message{role:"user".into(),text:"The login screen crashes after entering a password.".into()},Message{role:"assistant".into(),text:"The login handler passes a missing user to the profile screen. We can add a guard and a regression test.".into()}],..Context::default()};
    let t = Target::new(&Draft::at_end("fix"));
    let raw = e
        .generate(&t, &c, "Login crash", "", false, None)
        .await
        .unwrap();
    let p = t.phrases(&raw).unwrap();
    assert_eq!(p.len(), 3);
    assert!(p.iter().all(|s| s.starts_with("fix")));
    assert!(p.iter().any(|s| s.contains("login") || s.contains("crash")));
    println!("Synthetic prediction: {p:?}");
}
#[tokio::test]
#[ignore = "uses existing Codex ChatGPT account and allowance"]
async fn live_expansion() {
    enabled();
    let dir = tempfile::tempdir().unwrap();
    let e = Engine::connect(dir.path()).await.unwrap();
    let c=Context{messages:vec![Message{role:"user".into(),text:"Prompt Companion has three phrase buttons arranged vertically, an editable draft, Undo and Copy Prompt. Use ordinary left click. For the previous change, commit and push when done.".into()},Message{role:"assistant".into(),text:"That previous change is complete. Copy Prompt copies the draft; Undo can restore cleared text. The two current concerns are phrase buttons that feel too small and phrase suggestions that arrive slowly. Neither issue has been prioritized. The cause of the delay has not been identified.".into()}],..Context::default()};
    for s in [
        "bigger buttons same layout",
        "why slow",
        "fix it",
        "copy clear but no push",
    ] {
        let t = Target::new(&Draft::at_end(s));
        let raw = e
            .generate(&t, &c, "Prompt Companion", "", true, None)
            .await
            .unwrap();
        let result = expansion(&raw, false).unwrap();
        println!("Synthetic expansion {s}: {result:?}");
        if s == "fix it" {
            let Expansion::Clarification(choices) = result else {
                panic!("Two unresolved issues need clarification")
            };
            let r = Resolution {
                question: choices.question,
                choice: choices.choices[0].clone(),
            };
            let raw = e
                .generate(&t, &c, "Prompt Companion", "", true, Some(&r))
                .await
                .unwrap();
            println!("Chosen interpretation: {raw}");
            assert!(matches!(expansion(&raw, true), Ok(Expansion::Expanded(_))));
        } else {
            let Expansion::Expanded(prompt) = result else {
                panic!("Clear shorthand needs direct expansion")
            };
            if s == "why slow" {
                assert!(prompt.contains('?'));
            }
            if s != "copy clear but no push" {
                assert!(!prompt.to_lowercase().contains("push"));
                assert!(!prompt.to_lowercase().contains("commit"));
            }
        }
    }
}
