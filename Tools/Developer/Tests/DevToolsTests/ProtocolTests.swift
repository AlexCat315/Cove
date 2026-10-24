import Foundation
import Testing
@testable import DevTools

@Test func protocolNumbersDoNotAcceptBooleanValues() throws {
    let object = try JSON.parseObject(Data(#"{"flag":true,"count":1,"ratio":0.25}"#.utf8))
    #expect(JSON.int(object["flag"]) == nil)
    #expect(JSON.number(object["flag"]) == nil)
    #expect(JSON.boolean(object["count"]) == nil)
    #expect(JSON.boolean(object["flag"]) == true)
    #expect(JSON.int(object["count"]) == 1)
    #expect(JSON.number(object["ratio"]) == 0.25)
    #expect(throws: (any Error).self) { try JSON.parseObject(Data("[]".utf8)) }
    #expect(throws: (any Error).self) { try JSON.parseObject(Data()) }
}

@Test func commandBoundaryReturnsAsynchronousResultsAndErrors() throws {
    let result = try BlockingRequest.run { await Task.yield(); return 42 }
    #expect(result == 42)
    enum Failure: Error { case disconnected }
    #expect(throws: Failure.disconnected) {
        try BlockingRequest.run { () async throws -> Int in throw Failure.disconnected }
    }
}

@Test func compactNodeResultsKeepPagingMetadata() {
    let result: [String: Any] = ["data": ["items": [["id": "node-1", "label": "Focus"]],
                                                       "total": 2, "nextOffset": 1]]
    let text = QueryUI.render(result, compact: true)
    #expect(text.contains("node-1") && text.contains("next offset 1"))
}
