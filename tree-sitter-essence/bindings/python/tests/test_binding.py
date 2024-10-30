from unittest import TestCase

import tree_sitter, tree_sitter_essence_tester


class TestLanguage(TestCase):
    def test_can_load_grammar(self):
        try:
            tree_sitter.Language(tree_sitter_essence_tester.language())
        except Exception:
            self.fail("Error loading EssenceTester grammar")
