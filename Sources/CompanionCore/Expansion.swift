import Foundation

public struct ExpansionClarification: Equatable {
    public let question: String
    public let choices: [String]
    public init(question: String, choices: [String]) { self.question = question; self.choices = choices }
}

public struct ExpansionResolution: Codable, Equatable {
    public let question: String
    public let choice: String
    public init(question: String, choice: String) { self.question = question; self.choice = choice }
}

public enum ExpansionResult: Equatable {
    case expanded(String)
    case needsClarification(ExpansionClarification)
}

public enum ExpansionOutput {
    public static func parse(_ text: String, alreadyClarified: Bool) throws -> ExpansionResult {
        struct Response: Decodable {
            let kind: String
            let prompt: String
            let question: String
            let choices: [String]
        }
        let response = try JSONDecoder().decode(Response.self, from: Data(text.utf8))
        func clean(_ text: String) -> String { text.trimmingCharacters(in: .whitespacesAndNewlines) }
        if response.kind == "expanded" {
            let prompt = clean(response.prompt)
            guard !prompt.isEmpty, prompt.count <= 2000,
                  prompt.split(whereSeparator: \.isWhitespace).count <= 180,
                  clean(response.question).isEmpty, response.choices.isEmpty else { throw ExpansionError.invalidOutput }
            return .expanded(prompt)
        }
        if response.kind == "clarification", !alreadyClarified {
            let question = clean(response.question)
            let choices = response.choices.map(clean)
            guard clean(response.prompt).isEmpty, !question.isEmpty, question.count <= 140,
                  (2...3).contains(choices.count),
                  choices.allSatisfy({ !$0.isEmpty && $0.count <= 70 && !$0.contains("\n") }),
                  Set(choices.map { $0.lowercased() }).count == choices.count else { throw ExpansionError.invalidOutput }
            return .needsClarification(ExpansionClarification(question: question, choices: choices))
        }
        throw ExpansionError.invalidOutput
    }
}

public enum ExpansionError: LocalizedError {
    case invalidOutput
    public var errorDescription: String? { "Couldn’t make a usable expansion. Your original words are safe; try Expand again." }
}
