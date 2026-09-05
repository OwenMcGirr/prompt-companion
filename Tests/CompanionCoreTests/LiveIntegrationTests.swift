import XCTest
@testable import PromptCompanion
import CompanionCore

final class LiveIntegrationTests: XCTestCase {
    @MainActor
    func testContextAwareExpansionExamples() async throws {
        guard ProcessInfo.processInfo.environment["PROMPT_COMPANION_LIVE_TEST"] == "1" else {
            throw XCTSkip("Opt-in live expansion review uses the existing Codex allowance.")
        }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("PromptExpansionLive-" + UUID().uuidString)
        let engine = CompanionEngine(support: directory)
        defer { engine.stop() }
        try await engine.connect()
        let context = ContextSnapshot(messages: [
            .init(role: "user", text: "Prompt Companion has three phrase buttons arranged vertically, an editable draft, Undo and Copy Prompt. Use ordinary left click. For the previous change, commit and push when done."),
            .init(role: "assistant", text: "That previous change is complete. Copy Prompt copies the draft; Undo can restore cleared text. The two current concerns are phrase buttons that feel too small and phrase suggestions that arrive slowly. Neither issue has been prioritized. The cause of the delay has not been identified.")
        ], isPartial: false)
        for shorthand in ["bigger buttons same layout", "why slow", "fix it", "copy clear but no push"] {
            let started = Date()
            let result = try await engine.expand(draft: shorthand, context: context, title: "Prompt Companion", earlierSummary: "", resolution: nil)
            print("EXPANSION EXAMPLE \(shorthand): \(result) (\(String(format: "%.1f", Date().timeIntervalSince(started)))s)")
            if shorthand == "fix it" {
                guard case .needsClarification(let clarification) = result else { XCTFail("Two unresolved issues should offer a choice"); continue }
                let resolved = try await engine.expand(draft: shorthand, context: context, title: "Prompt Companion", earlierSummary: "",
                    resolution: .init(question: clarification.question, choice: clarification.choices[0]))
                print("RESOLVED EXAMPLE: \(resolved)")
                guard case .expanded = resolved else { XCTFail("A chosen interpretation must not ask again"); continue }
            } else {
                guard case .expanded(let prompt) = result else { XCTFail("Clear shorthand should expand directly"); continue }
                if shorthand == "why slow" { XCTAssertTrue(prompt.contains("?")) }
                if shorthand != "copy clear but no push" {
                    XCTAssertFalse(prompt.localizedCaseInsensitiveContains("push"))
                    XCTAssertFalse(prompt.localizedCaseInsensitiveContains("commit"))
                }
            }
        }
    }

    @MainActor
    func testExistingCodexLoginAndContextualPrediction() async throws {
        guard ProcessInfo.processInfo.environment["PROMPT_COMPANION_LIVE_TEST"] == "1" else {
            throw XCTSkip("Opt-in live check uses the existing Codex usage allowance.")
        }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("PromptCompanionLive-" + UUID().uuidString)
        let engine = CompanionEngine(support: directory)
        defer { engine.stop() }
        try await engine.connect()
        let (tasks, _) = try await engine.listTasks()
        XCTAssertFalse(tasks.isEmpty)
        if let id = ProcessInfo.processInfo.environment["PROMPT_COMPANION_TEST_TASK"] {
            let snapshot = try await engine.context(for: id)
            XCTAssertFalse(snapshot.messages.isEmpty)
        }
        let context = ContextSnapshot(messages: [
            ConversationMessage(role: "user", text: "The login screen crashes after entering a password."),
            ConversationMessage(role: "assistant", text: "The login handler passes a missing user to the profile screen. We can add a guard and a regression test.")
        ], isPartial: false)
        let target = CompletionTarget(Draft(text: "fix", cursor: 3))
        let result = try await engine.predict(target: target, context: context, title: "Login crash", earlierSummary: "")
        XCTAssertEqual(result.phrases.count, 3)
        XCTAssertTrue(result.phrases.allSatisfy { $0.hasPrefix("fix") })
        XCTAssertTrue(result.phrases.contains { $0.localizedCaseInsensitiveContains("login") || $0.localizedCaseInsensitiveContains("crash") })
        print("Live prediction: \(result.phrases); \(String(format: "%.1f", result.duration)) seconds")
    }
}
