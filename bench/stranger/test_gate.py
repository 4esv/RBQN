import os, sys, tempfile, unittest
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gate

HEAD = "exprs=10 wrong=0 strict=1 lenient=1 timeouts=[] panics=0 rbqn_stopped_early=0\n"
CELL = "--- %s\n  cbqn: 'a'\n  rbqn: 'b'\n"


class Gate(unittest.TestCase):
    def test_docs(self):
        self.assertEqual(gate.docs_failures("compared 5 expressions; 0 differ\n"), [])
        self.assertTrue(gate.docs_failures("compared 5 expressions; 1 differ\n--- a.md: ⌽5\n    cbqn: 'x'\n"))
        self.assertTrue(gate.docs_failures("garbage"))
        self.assertEqual(gate.docs_failures("compared 5 expressions; 1 differ\n--- a.md: ⌽5\n    cbqn: 'x'\n", {"a.md: ⌽5"}), [])

    def test_grid_known_passes_unknown_fails(self):
        txt = HEAD + "=== strict (1)\n" + CELL % "@ ¬ 1" + CELL % "⌽ 5"
        bad = gate.grid_failures(txt, {"@ ¬ 1"})
        self.assertEqual(bad[0], "--- ⌽ 5")
        self.assertEqual(gate.grid_failures(HEAD + CELL % "@ ¬ 1", {"@ ¬ 1"}), [])

    def test_timeouts_fail(self):
        txt = "exprs=10 wrong=0 strict=0 lenient=0 timeouts=[('rbqn', 0)] panics=0 rbqn_stopped_early=0\n"
        self.assertTrue(gate.grid_failures(txt, set()))

    def test_known_file(self):
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
            f.write("# c\n@ ¬ 1 | why\n")
        self.assertEqual(gate.known(f.name), {"@ ¬ 1"})
        with tempfile.NamedTemporaryFile("w", suffix=".txt", delete=False) as f:
            f.write("# c\n@ ¬ 1 | why | docs/spec/primitive.md#not\n⌽ 5 | old two-field\n\n")
        self.assertEqual(gate.known(f.name), {"@ ¬ 1", "⌽ 5"})
        # NOTE: the shipped file may be empty once every divergence is fixed; it must still parse.
        self.assertIsInstance(gate.known(os.path.join(os.path.dirname(__file__), "known_divergences.txt")), set)


if __name__ == "__main__":
    unittest.main()
