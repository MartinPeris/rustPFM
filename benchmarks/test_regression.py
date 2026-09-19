"""Deterministic checks of the timing policy and failure reporting."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_regression as gate


class RegressionPolicyTests(unittest.TestCase):
    def setUp(self):
        self.policy = json.loads((gate.ROOT / 'benchmarks/regression-policy.json').read_text())

    def trials(self, ratios, baseline=0.01):
        return [{'baseline': {'samples_seconds': [baseline] * self.policy['repeats']},
                 'candidate': {'samples_seconds': [baseline * ratio] * self.policy['repeats']}}
                for ratio in ratios]

    def test_sustained_regression_fails(self):
        self.assertEqual(gate.assess(self.trials([1.3, 1.4, 1.3]), self.policy)['status'], 'regression')

    def test_single_noisy_trial_warns_without_failing(self):
        self.assertEqual(gate.assess(self.trials([1.5, 1.01, 0.99]), self.policy)['status'], 'warning')

    def test_median_rejects_single_outlier_sample(self):
        trials = self.trials([1, 1, 1])
        for trial in trials:
            trial['candidate']['samples_seconds'][-1] = 10
        self.assertEqual(gate.assess(trials, self.policy)['status'], 'pass')

    def test_absolute_floor_and_boundary(self):
        self.assertEqual(gate.assess(self.trials([1.3] * 3, 0.0001), self.policy)['status'], 'pass')
        self.assertEqual(gate.assess(self.trials([1.2] * 3), self.policy)['status'], 'pass')

    def test_improvement_passes(self):
        self.assertEqual(gate.assess(self.trials([0.6, 0.7, 0.8]), self.policy)['status'], 'pass')

    def test_invalid_or_incomplete_timings_fail_closed(self):
        for value in [0, -1, float('nan'), float('inf'), '0.01']:
            trials = self.trials([1, 1, 1])
            trials[0]['baseline']['samples_seconds'][0] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                gate.assess(trials, self.policy)
        with self.assertRaises(ValueError):
            gate.assess(self.trials([1, 1]), self.policy)
        trials = self.trials([1, 1, 1])
        trials[0]['candidate']['samples_seconds'].pop()
        with self.assertRaises(ValueError):
            gate.assess(trials, self.policy)

    def test_invalid_policy_rejected(self):
        for key, value in [('baseline_revision', 'main'), ('slowdown_percent', 0),
                           ('minimum_increase_ms', float('nan')), ('trials', 1),
                           ('repeats', 2), ('sizes', []), ('channels', [2])]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                gate.validate_policy(dict(self.policy, **{key: value}))

    def test_worker_failure_produces_report_and_nonzero_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'result.json'
            with patch('sys.argv', ['gate', '--output', str(output)]), \
                 patch.object(gate, 'measure', side_effect=RuntimeError('worker failed')), \
                 patch.dict('os.environ', {'GITHUB_ACTIONS': 'false', 'GITHUB_STEP_SUMMARY': ''}), \
                 contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(gate.main(), 1)
            report = json.loads(output.read_text())
            self.assertEqual(report['status'], 'error')
            self.assertIn('worker failed', report['error'])
            self.assertTrue(output.with_suffix('.md').exists())

    def test_invalid_policy_still_writes_failure_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'benchmarks').mkdir()
            (root / 'benchmarks/regression-policy.json').write_text('{"slowdown_percent": NaN}')
            output = root / 'result.json'
            with patch.object(gate, 'ROOT', root), \
                 patch('sys.argv', ['gate', '--output', str(output)]), \
                 patch.dict('os.environ', {'GITHUB_ACTIONS': 'false', 'GITHUB_STEP_SUMMARY': ''}), \
                 contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(gate.main(), 1)
            self.assertEqual(json.loads(output.read_text())['status'], 'error')

    def test_regression_is_nonzero_and_generates_annotation(self):
        def measured(report, repository):
            report['status'] = 'regression'
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'result.json'
            stdout = io.StringIO()
            with patch('sys.argv', ['gate', '--output', str(output)]), \
                 patch.object(gate, 'measure', side_effect=measured), \
                 patch.dict('os.environ', {'GITHUB_ACTIONS': 'true', 'GITHUB_STEP_SUMMARY': str(Path(directory) / 'summary')}), \
                 contextlib.redirect_stdout(stdout):
                self.assertEqual(gate.main(), 1)
            self.assertIn('::error title=Performance regression check::', stdout.getvalue())


if __name__ == '__main__':
    unittest.main()
