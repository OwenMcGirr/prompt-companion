import XCTest
@testable import PromptCompanion
import CompanionCore

final class LiveIntegrationTests: XCTestCase {
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
