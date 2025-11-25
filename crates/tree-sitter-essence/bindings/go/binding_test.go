package tree_sitter_essence_test

import (
	"testing"

	tree_sitter "github.com/tree-sitter/go-tree-sitter"
	tree_sitter_essence "github.com/conjure-cp/conjure-oxide/bindings/go"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_essence.Language())
	if language == nil {
		t.Errorf("Error loading Essence grammar")
	}
}
