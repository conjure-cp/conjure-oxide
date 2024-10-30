import XCTest
import SwiftTreeSitter
import TreeSitterEssenceTester

final class TreeSitterEssenceTesterTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_essence_tester())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading EssenceTester grammar")
    }
}
