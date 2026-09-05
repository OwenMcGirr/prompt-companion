import Foundation

public struct Draft: Codable, Equatable {
    public var text: String
    public var cursor: Int
    public var selectionLength: Int
    public init(text: String = "", cursor: Int = 0, selectionLength: Int = 0) {
        self.text = text
        self.cursor = min(max(0, cursor), (text as NSString).length)
        self.selectionLength = min(max(0, selectionLength), (text as NSString).length - self.cursor)
    }
    public var selection: NSRange { NSRange(location: cursor, length: selectionLength) }
}

public struct CompletionTarget: Equatable {
    public let draft: Draft
    public let range: NSRange
    public let partial: String
    public let before: String
    public let after: String

    public init(_ draft: Draft) {
        self.draft = draft
        let ns = draft.text as NSString
        var start = draft.cursor
        var end = draft.cursor + draft.selectionLength
        func isWord(_ c: unichar) -> Bool {
            guard let scalar = UnicodeScalar(c) else { return false }
            return CharacterSet.alphanumerics.contains(scalar) || CharacterSet.nonBaseCharacters.contains(scalar)
                || scalar == "_" || scalar == "'" || scalar == "’"
        }
        if draft.selectionLength == 0 {
            while start > 0 && isWord(ns.character(at: start - 1)) { start -= 1 }
            // Complete the token around the caret, including its existing suffix.
            if start < draft.cursor {
                while end < ns.length && isWord(ns.character(at: end)) { end += 1 }
            }
        }
        range = NSRange(location: start, length: end - start)
        partial = draft.selectionLength > 0 ? "" : ns.substring(with: NSRange(location: start, length: draft.cursor - start))
        before = ns.substring(to: start)
        after = ns.substring(from: end)
    }

    public func normalized(_ candidate: String) -> String? {
        var value = candidate.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty, value.count <= 160, !value.contains("\n"), !value.contains("\r") else { return nil }
        if !partial.isEmpty {
            guard value.lowercased().hasPrefix(partial.lowercased()) else { return nil }
            // Never change the capitalization the user already typed.
            value = partial + value.dropFirst(partial.count)
        }
        guard value != partial else { return nil }
        return value
    }

    public func inserting(_ candidate: String) -> Draft? {
        guard var value = normalized(candidate) else { return nil }
        if let first = value.first, let last = before.last,
           first.isLetter, ",;:!?".contains(last) { value = " " + value }
        if after.isEmpty || (after.first.map { $0.isLetter || $0.isNumber } ?? false) { value += " " }
        let text = (draft.text as NSString).replacingCharacters(in: range, with: value)
        return Draft(text: text, cursor: range.location + (value as NSString).length)
    }
}

public struct SuggestionBatch {
    public let revision: Int
    public let target: CompletionTarget
    public let phrases: [String]
    public init(revision: Int, target: CompletionTarget, phrases: [String]) {
        self.revision = revision; self.target = target; self.phrases = phrases
    }
    public func valid(for draft: Draft, revision: Int) -> Bool {
        self.revision == revision && target.draft == draft
    }
}

public enum PredictionOutput {
    public static func phrases(from text: String, target: CompletionTarget) throws -> [String] {
        struct Response: Decodable { let suggestions: [String] }
        let response = try JSONDecoder().decode(Response.self, from: Data(text.utf8))
        var seen = Set<String>()
        let values = response.suggestions.compactMap { target.normalized($0) }
            .filter { seen.insert($0.lowercased()).inserted }
        guard !values.isEmpty else { throw CoreError.invalidPredictions }
        return Array(values.prefix(3))
    }
}

public enum CoreError: LocalizedError {
    case invalidPredictions
    public var errorDescription: String? { "No usable phrases arrived. Try a few more letters or click Refresh phrases." }
}

public struct ConversationMessage: Codable, Equatable {
    public let role: String
    public let text: String
    public init(role: String, text: String) { self.role = role; self.text = text }
}

public enum ContextBuilder {
    // Keep user/assistant text only. Tool output, hidden reasoning, and attachments
    // are deliberately excluded from the prediction input.
    public static func messages(from turns: [[String: Any]]) -> [ConversationMessage] {
        turns.flatMap { turn -> [ConversationMessage] in
            (turn["items"] as? [[String: Any]] ?? []).compactMap { item in
                switch item["type"] as? String {
                case "userMessage":
                    let text = (item["content"] as? [[String: Any]] ?? [])
                        .filter { $0["type"] as? String == "text" }
                        .compactMap { $0["text"] as? String }.joined(separator: "\n")
                    return text.isEmpty ? nil : ConversationMessage(role: "user", text: text)
                case "agentMessage":
                    guard let text = item["text"] as? String, !text.isEmpty else { return nil }
                    return ConversationMessage(role: "assistant", text: text)
                default: return nil
                }
            }
        }
    }

    public static func bounded(_ messages: [ConversationMessage], budget: Int = 18000) -> [ConversationMessage] {
        var remaining = budget
        var recent: [ConversationMessage] = []
        for message in messages.reversed() {
            guard remaining > 0 else { break }
            let limit = min(remaining, message.role == "user" ? 3000 : 4500)
            let text = String(message.text.prefix(limit))
            recent.append(ConversationMessage(role: message.role, text: text))
            remaining -= text.count
        }
        return recent.reversed()
    }
}
