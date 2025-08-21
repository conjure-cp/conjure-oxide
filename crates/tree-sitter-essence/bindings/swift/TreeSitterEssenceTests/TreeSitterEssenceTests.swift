import XCTest
import SwiftTreeSitter
import TreeSitterEssence

final class TreeSitterEssenceTests: XCTestCase {
    func testCanLoadGrammar() throws {
        let parser = Parser()
        let language = Language(language: tree_sitter_essence())
        XCTAssertNoThrow(try parser.setLanguage(language),
                         "Error loading Essence grammar")
    }
}
