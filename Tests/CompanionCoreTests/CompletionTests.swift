import XCTest
@testable import CompanionCore

final class CompletionTests: XCTestCase {
    func testPartialWordPreservesTypedCase() throws {
        let target = CompletionTarget(Draft(text: "Please Fi", cursor: 9))
        XCTAssertEqual(target.partial, "Fi")
        XCTAssertEqual(target.before, "Please ")
        XCTAssertEqual(target.inserting("fix the login error")?.text, "Please Fix the login error ")
        XCTAssertNil(target.inserting("change the layout"))
    }

    func testCursorInsideTokenDoesNotDuplicateSuffix() {
        let target = CompletionTarget(Draft(text: "fix logn tomorrow", cursor: 6))
        XCTAssertEqual(target.partial, "lo")
        XCTAssertEqual(target.inserting("login")?.text, "fix login tomorrow")
    }

    func testSelectionReplacementPreservesFollowingText() {
        let target = CompletionTarget(Draft(text: "Fix login, please.", cursor: 4, selectionLength: 5))
        XCTAssertEqual(target.partial, "")
        XCTAssertEqual(target.inserting("the layout")?.text, "Fix the layout, please.")
    }

    func testEmojiUTF16CursorAndInsertion() {
        let input = "🙂 fix "
        let result = CompletionTarget(Draft(text: input, cursor: (input as NSString).length)).inserting("the error")
        XCTAssertEqual(result?.text, "🙂 fix the error ")
        XCTAssertEqual(result?.cursor, ("🙂 fix the error " as NSString).length)
    }

    func testEmptyDraftAndPunctuation() {
        XCTAssertEqual(CompletionTarget(Draft()).inserting("Show me the options")?.text, "Show me the options ")
        XCTAssertEqual(CompletionTarget(Draft(text: "Yes,", cursor: 4)).inserting("please explain")?.text, "Yes, please explain ")
    }

    func testCombiningAccentRemainsPartOfToken() {
        let input = "cafe\u{301}"
        let target = CompletionTarget(Draft(text: input, cursor: (input as NSString).length))
        XCTAssertEqual(target.partial, input)
        XCTAssertEqual(target.inserting(input + " recommendations")?.text, input + " recommendations ")
    }

    func testPredictionValidationRejectsDuplicatesUnrelatedAndMultiline() throws {
        let target = CompletionTarget(Draft(text: "fix", cursor: 3))
        let json = #"{"suggestions":["fix login","FIX LOGIN","write docs","fix\nlogin","fix the layout"]}"#
        XCTAssertEqual(try PredictionOutput.phrases(from: json, target: target), ["fix login", "fix the layout"])
        XCTAssertThrowsError(try PredictionOutput.phrases(from: #"{"suggestions":["unrelated"]}"#, target: target))
    }

    func testRevisionAndSelectionBothInvalidateResults() {
        let draft = Draft(text: "fix", cursor: 3)
        let batch = SuggestionBatch(revision: 4, target: CompletionTarget(draft), phrases: ["fix login"])
        XCTAssertTrue(batch.valid(for: draft, revision: 4))
        XCTAssertFalse(batch.valid(for: draft, revision: 5))
        XCTAssertFalse(batch.valid(for: Draft(text: "fix", cursor: 0), revision: 4))
    }

    func testContextExcludesToolsReasoningAndImages() {
        let turns: [[String: Any]] = [["items": [
            ["type": "userMessage", "content": [["type": "text", "text": "Fix the error"], ["type": "image", "url": "private"]]],
            ["type": "reasoning", "text": "hidden"],
            ["type": "commandExecution", "aggregatedOutput": "secret"],
            ["type": "agentMessage", "text": "The login handler is failing"]
        ]]]
        XCTAssertEqual(ContextBuilder.messages(from: turns), [
            ConversationMessage(role: "user", text: "Fix the error"),
            ConversationMessage(role: "assistant", text: "The login handler is failing")
        ])
    }

    func testContextBudgetPrefersRecentMessages() {
        let messages = [ConversationMessage(role: "user", text: String(repeating: "a", count: 200)), ConversationMessage(role: "assistant", text: "Recent correction")]
        let result = ContextBuilder.bounded(messages, budget: 40)
        XCTAssertLessThanOrEqual(result.reduce(0) { $0 + $1.text.count }, 40)
        XCTAssertEqual(result.last?.text, "Recent correction")
    }
}
