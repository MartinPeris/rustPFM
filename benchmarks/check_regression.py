#!/usr/bin/env python3
"""Paired Linux file-I/O regression gate; Python standard library only."""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(args, cwd=None, env=None, timeout=300):
    return subprocess.check_output(args, cwd=cwd, env=env, text=True, timeout=timeout)


def reject_nonfinite(value):
    raise ValueError(f'nonfinite JSON number: {value}')


def validate_policy(policy):
    if not re.fullmatch(r'[0-9a-f]{40}', policy['baseline_revision']):
        raise ValueError('baseline_revision must be a full commit SHA')
    for key in ('slowdown_percent', 'minimum_increase_ms'):
        if not math.isfinite(policy[key]) or policy[key] <= 0:
            raise ValueError(f'{key} must be finite and positive')
    for key in ('trials', 'repeats', 'warmup'):
        if type(policy[key]) is not int or policy[key] < 1:
            raise ValueError(f'{key} must be a positive integer')
    if policy['trials'] < 3 or policy['repeats'] < 5:
        raise ValueError('at least three trials and five samples are required')
    if not policy['sizes'] or any(type(n) is not int or n < 1 for n in policy['sizes']):
        raise ValueError('sizes must be positive integers')
    if not policy['channels'] or any(n not in (1, 3) for n in policy['channels']):
        raise ValueError('channels must be 1 or 3')


def samples(result, repeats):
    values = result['samples_seconds']
    if len(values) != repeats or any(
        type(value) not in (int, float) or not math.isfinite(value) or value <= 0
        for value in values
    ):
        raise ValueError('worker returned missing, nonfinite or nonpositive timings')
    return values


def assess(trials, policy):
    if len(trials) != policy['trials']:
        raise ValueError('incomplete paired trials')
    ratios, increases, exceeded = [], [], []
    for trial in trials:
        baseline = statistics.median(samples(trial['baseline'], policy['repeats']))
        candidate = statistics.median(samples(trial['candidate'], policy['repeats']))
        ratios.append(candidate / baseline)
        increases.append((candidate - baseline) * 1000)
        exceeded.append(candidate > baseline * (1 + policy['slowdown_percent'] / 100)
                        and increases[-1] > policy['minimum_increase_ms'])
    return {'status': 'regression' if all(exceeded) else 'warning' if any(exceeded) else 'pass',
            'ratios': ratios, 'increases_ms': increases, 'exceeded': exceeded}


def source_hashes(tree):
    paths = [tree / 'Cargo.toml', tree / 'Cargo.lock', *sorted((tree / 'src').rglob('*'))]
    return {str(p.relative_to(tree)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in paths if p.is_file()}


def export_baseline(repository, revision, destination):
    paths = run(['git', '-C', str(repository), 'ls-tree', '-r', '--name-only', revision,
                 '--', 'src', 'Cargo.toml', 'Cargo.lock', 'README.md', 'examples/benchmark.rs']).splitlines()
    for name in paths:
        path = Path(name)
        if path.is_absolute() or '..' in path.parts:
            raise ValueError('invalid baseline path')
        output = destination / path
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(subprocess.check_output(
            ['git', '-C', str(repository), 'show', f'{revision}:{name}'], timeout=30))
    # Use one pinned, validating driver for both versions, independent of candidate edits.
    driver = destination / 'examples/benchmark.rs'
    driver.rename(destination / 'examples/perf_regression.rs')


def fixture(path, size, channels, scale):
    row_bytes = size * channels * 4
    count = size * size * channels
    pattern = struct.pack('<257f', *(n - 128 for n in range(257)))
    pixels = (pattern * ((count + 256) // 257))[:count * 4]
    with path.open('wb') as stream:
        stream.write(f"{'Pf' if channels == 1 else 'PF'}\n{size} {size}\n-{scale}\n".encode())
        for y in reversed(range(size)):
            stream.write(pixels[y * row_bytes:(y + 1) * row_bytes])


def markdown(report):
    lines = ['# Performance regression check', '',
             f"Status: **{report['status']}**. Baseline: `{report['policy'].get('baseline_revision', 'unavailable')}`.", '',
             'Each delta is a paired trial median; all trials must exceed both limits to fail.', '',
             '| Case | Candidate slowdown by trial | Result |', '|---|---|---|']
    for case in report['cases']:
        if 'assessment' in case:
            a = case['assessment']
            deltas = ', '.join(f'{(r - 1) * 100:+.1f}%' for r in a['ratios'])
            lines.append(f"| {case['name']} | {deltas} | {a['status']} |")
    if 'error' in report:
        lines.extend(['', 'The check could not complete; see the JSON report and job log.'])
    return '\n'.join(lines) + '\n'


def publish(report, output):
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, allow_nan=False) + '\n')
    summary = markdown(report)
    output.with_suffix('.md').write_text(summary)
    print(summary, flush=True)
    if os.environ.get('GITHUB_STEP_SUMMARY'):
        with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as stream:
            stream.write(summary)
    if os.environ.get('GITHUB_ACTIONS') == 'true':
        if report['status'] in ('regression', 'error'):
            print('::error title=Performance regression check::' +
                  ('Confirmed slowdown exceeds policy; inspect the performance report.'
                   if report['status'] == 'regression' else 'Benchmark failed to complete; inspect the job log.'))
        elif any(c.get('assessment', {}).get('status') == 'warning' for c in report['cases']):
            print('::warning title=Performance variation::An unconfirmed slowdown occurred; inspect the performance report.')


def measure(report, repository):
    policy = report['policy']
    validate_policy(policy)
    if sys.platform != 'linux':
        raise RuntimeError('performance gate currently requires Linux')
    report['environment'] = {
        'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'platform': platform.platform(), 'rustc': run(['rustc', '-Vv']).strip(),
        'cpu': next((line.split(':', 1)[1].strip() for line in
                     Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unknown'),
        'affinity': sorted(os.sched_getaffinity(0)),
        'load_before': os.getloadavg(),
        'hugepage_policy': Path('/sys/kernel/mm/transparent_hugepage/enabled').read_text().strip()
        if Path('/sys/kernel/mm/transparent_hugepage/enabled').exists() else 'unavailable',
        'candidate_source_sha256': source_hashes(ROOT),
        'candidate_repository_head': run(['git', '-C', str(repository), 'rev-parse', 'HEAD']).strip(),
        'candidate_identity': 'source hashes identify the tested tree, including a staged hook snapshot',
        'timing': 'Serial paired workers; warm files; default release features; no fsync; validation outside timer.',
    }
    with tempfile.TemporaryDirectory(prefix='rustpfm-regression-') as scratch:
        scratch = Path(scratch)
        trees = {name: scratch / name for name in ('baseline', 'candidate')}
        for tree in trees.values():
            tree.mkdir()
        export_baseline(repository, policy['baseline_revision'], trees['baseline'])
        candidate = trees['candidate']
        shutil.copytree(ROOT / 'src', candidate / 'src')
        for name in ('Cargo.toml', 'Cargo.lock', 'README.md'):
            shutil.copyfile(ROOT / name, candidate / name)
        (candidate / 'examples').mkdir()
        driver = Path('examples/perf_regression.rs')
        shutil.copyfile(trees['baseline'] / driver, candidate / driver)
        report['environment']['baseline_source_sha256'] = source_hashes(trees['baseline'])
        report['environment']['driver_sha256'] = hashlib.sha256((candidate / driver).read_bytes()).hexdigest()
        binaries = {}
        # Build both before measuring; never run compilation concurrently with trials.
        for name, tree in trees.items():
            env = dict(os.environ, CARGO_TARGET_DIR=str(tree / 'target'), CARGO_INCREMENTAL='0', RUSTFLAGS='')
            env.pop('CARGO_ENCODED_RUSTFLAGS', None)
            env.pop('CARGO_BUILD_TARGET', None)
            run(['cargo', 'build', '--locked', '--release', '--example', 'perf_regression'], cwd=tree, env=env)
            binaries[name] = tree / 'target/release/examples/perf_regression'
        report['environment']['binary_sha256'] = {
            name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()}
        for size in policy['sizes']:
            for channels in policy['channels']:
                for operation, scale in [('read', 1), ('read', 2), ('write', 1)]:
                    case = {'name': f'{size}x{size}/{channels}ch/{operation}/scale{scale}',
                            'size': size, 'channels': channels, 'operation': operation,
                            'scale': scale, 'trials': []}
                    report['cases'].append(case)
                    path = scratch / 'fixture.pfm'
                    fixture(path, size, channels, scale)
                    for trial_index in range(policy['trials']):
                        order = ['baseline', 'candidate']
                        if (trial_index + len(report['cases'])) % 2:
                            order.reverse()
                        trial = {'order': order}
                        for name in order:
                            trial[name] = json.loads(run([
                                str(binaries[name]), operation, str(size), str(channels), str(scale),
                                str(policy['repeats']), str(policy['warmup']), str(path), 'top'], timeout=120))
                            samples(trial[name], policy['repeats'])
                        case['trials'].append(trial)
                    case['assessment'] = assess(case['trials'], policy)
                    print(case['name'], case['assessment'], flush=True)
    report['status'] = 'regression' if any(
        c['assessment']['status'] == 'regression' for c in report['cases']) else 'pass'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', type=Path, default=Path(os.environ.get('RUSTPFM_SOURCE_REPOSITORY', ROOT)))
    parser.add_argument('--output', type=Path, default=Path(os.environ.get('RUSTPFM_PERFORMANCE_OUTPUT', ROOT / 'target/performance/report.json')))
    args = parser.parse_args()
    report = {'schema_version': 1, 'status': 'error', 'cases': [], 'policy': {}}
    try:
        report['policy'] = json.loads((ROOT / 'benchmarks/regression-policy.json').read_text(),
                                      parse_constant=reject_nonfinite)
        measure(report, args.repository.resolve())
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        report['error'] = str(error)
        print(f'Performance check failed: {error}', file=sys.stderr)
    except RuntimeError as error:
        report['error'] = str(error)
        print(str(error), file=sys.stderr)
    publish(report, args.output)
    return 0 if report['status'] == 'pass' else 1


if __name__ == '__main__':
    sys.exit(main())
