import os
import unittest
from pathlib import Path

from dotenv import load_dotenv

from stats.essence_file import EssenceFile
from test_data.constants import KNAPSACK_AST

ENV_PATH = Path("../.env").resolve()
load_dotenv(dotenv_path=ENV_PATH)

CONJURE_DIR = os.getenv("CONJURE_DIR")
CONJURE_BIN = Path(CONJURE_DIR) / "conjure"


class TestEssenceFile(unittest.TestCase):
    def test_instantiate(self):
        file = EssenceFile("test_data/knapsack.essence", CONJURE_BIN)
        self.assertIsInstance(file, EssenceFile)

    def test_path(self):
        file = EssenceFile("test_data/knapsack.essence", CONJURE_BIN)
        path = Path("test_data/knapsack.essence").resolve()
        self.assertEqual(file.path, path)

    def test_path_trimmed(self):
        file = EssenceFile("test_data/knapsack.essence", CONJURE_BIN)
        self.assertEqual(file.get_str_path(depth=1), "knapsack.essence")

    def test_ast(self):
        file = EssenceFile("test_data/knapsack.essence", CONJURE_BIN)
        self.assertEqual(file.ast, KNAPSACK_AST)


if __name__ == "__main__":
    unittest.main()
