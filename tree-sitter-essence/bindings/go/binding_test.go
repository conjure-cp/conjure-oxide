package tree_sitter_essence_tester_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_essence_tester "github.com/tree-sitter/tree-sitter-essence_tester/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_essence_tester.Language())
	if language == nil {
		t.Errorf("Error loading EssenceTester grammar")
	}
}
