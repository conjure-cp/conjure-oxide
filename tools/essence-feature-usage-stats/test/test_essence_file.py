import os
import unittest
from pathlib import Path

from dotenv import load_dotenv
from test_data.constants import KNAPSACK_AST

from stats.essence_file import EssenceFile

ENV_PATH = Path("../.env").resolve()
load_dotenv(dotenv_path=ENV_PATH)

CONJURE_DIR = os.getenv("CONJURE_DIR")
CONJURE_BIN = Path(CONJURE_DIR) / "conjure"


class TestEssenceFile(unittest.TestCase):
    """Tests for EssenceFile class."""

    def test_instantiate(self):
        """Test that an EssenceFile can be instantiated."""
        file = EssenceFile("test_data/knapsack.essence", CONJURE_BIN)
        self.assertIsInstance(file, EssenceFile)

    def test_path(self):
        """Test that an EssenceFile object has the correct path."""
        file = EssenceFile("test_data/knapsack.essence", CONJURE_BIN)
        path = Path("test_data/knapsack.essence").resolve()
        self.assertEqual(file.path, path)

    def test_path_trimmed(self):
        """Test that an EssenceFile path is trimmed correctly."""
        file = EssenceFile("test_data/knapsack.essence", CONJURE_BIN)
        self.assertEqual(file.get_str_path(depth=1), "knapsack.essence")

    def test_ast(self):
        """Test that an AST is generated correctly."""
        file = EssenceFile("test_data/knapsack.essence", CONJURE_BIN)
        self.assertEqual(file.ast, KNAPSACK_AST)


if __name__ == "__main__":
    unittest.main()
