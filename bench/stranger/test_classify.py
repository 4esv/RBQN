import os, sys, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import classify


class Classify(unittest.TestCase):
    def test_lenient(self):
        self.assertEqual(classify("ERR\n", "3\n3"), "lenient")

    def test_strict(self):
        self.assertEqual(classify("3", "ERR\n3"), "strict")

    def test_wrong(self):
        self.assertEqual(classify("3", "4"), "wrong")


if __name__ == "__main__":
    unittest.main()
