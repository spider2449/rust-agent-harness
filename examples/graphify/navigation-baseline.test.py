"""Offline checks for fixture locations and relation scoring."""
import runpy
import unittest
import json
from pathlib import Path

BASELINE = runpy.run_path(str(Path(__file__).with_name("navigation-baseline.py")))


class NavigationBaselineTests(unittest.TestCase):
    def setUp(self):
        self.sites, _, self.files = BASELINE["source_cases"](BASELINE["FIXTURE"], False)

    def test_fixture_locations(self):
        self.assertEqual(self.files, 3)
        callbacks = {row["line"]: row for row in self.sites if row["kind"] == "callback"}
        self.assertEqual(sorted(callbacks), [33, 36, 37, 50, 78])
        self.assertEqual(callbacks[36]["expected"], callbacks[37]["expected"])
        self.assertNotEqual(callbacks[36]["signal"], callbacks[37]["signal"])
        self.assertEqual(callbacks[78]["classification"], "event")
        self.assertEqual([row["line"] for row in self.sites if not row["expected"]], [21, 65])

    def test_scores_positionless_edges_separately_from_call_sites(self):
        callback = dict(file="window.py", source="window.Window.wire_triggered",
                        relation="CALL_REFERENCE", target="window.Window.menuHelp_triggered",
                        expression="menuHelp_triggered", line=None)
        decoy = dict(file="window.py", source="window.Window.run_alias", relation="CALLS",
                     target="decoy.Decoy.ik2fk", expression="self.ik2fk", line=18)
        score = BASELINE["score"](self.sites, [callback, decoy])
        self.assertEqual(score["sites"], {"hit": 2, "wrong": 1, "unscored": 2, "miss": 7})
        self.assertEqual(score["target_pairs"], {"hit": 1, "wrong": 1, "miss": 7})
        self.assertEqual(score["position_hits"], 0)
        self.assertEqual(score["merged_candidates"][0]["positions"], [(36, 8), (37, 8)])

        callback["line"] = 37
        score = BASELINE["score"](self.sites, [callback, decoy])
        self.assertEqual(score["position_hits"], 1)
        self.assertEqual(score["merged_candidates"], [])
        self.assertEqual(score["sites"]["hit"], 1)

        wrong_callback = dict(callback, target="decoy.Decoy.menuHelp_triggered")
        score = BASELINE["score"](self.sites, [callback, decoy, wrong_callback])
        self.assertEqual(score["sites"]["hit_with_wrong"], 1)
        self.assertEqual(score["target_pairs"]["hit_with_wrong"], 1)

    def test_multiline_ripgrep_match_counts_the_connect_token_line(self):
        result = dict(type="match", data=dict(path={"text": "window.py"},
                      lines={"text": "訊號.\n  connect(\n"}, line_number=36,
                      submatches=[{"start": len("訊號".encode())}]))
        self.assertEqual(BASELINE["source_match_lines"](json.dumps(result)), {("window.py", 37)})


if __name__ == "__main__":
    unittest.main()
