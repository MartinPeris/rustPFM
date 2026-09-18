#!/usr/bin/env python3
"""Opt-in Linux comparison: release-built rustPFM and installed justPFM."""
import argparse
import hashlib
import importlib.metadata
import json
import os
import platform
import resource
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path

import numpy as np
import justpfm.justpfm as implementation
from justpfm import read_pfm, write_pfm

ROOT = Path(__file__).resolve().parents[1]


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def pixels(case):
    shape = (case['size'], case['size'], case['channels'])
    values = np.arange(np.prod(shape), dtype=np.float32).reshape(shape)
    values %= 257
    values -= 128
    return values.astype('<f4', copy=False)


def fixture(path, case):
    values = pixels(case)
    magic = 'Pf' if case['channels'] == 1 else 'PF'
    with path.open('wb') as stream:
        stream.write(f"{magic}\n{case['size']} {case['size']}\n-{case['scale']}\n".encode())
        values[::-1].tofile(stream)


def validate_write(path, values, case):
    with path.open('rb') as stream:
        assert stream.readline().strip() == (b'Pf' if case['channels'] == 1 else b'PF')
        assert stream.readline().strip() == f"{case['size']} {case['size']}".encode()
        assert float(stream.readline()) == -case['scale']
        stored = np.fromfile(stream, dtype='<f4')
    np.testing.assert_array_equal(stored, values[::-1].ravel())


def python_worker(case, path):
    values = pixels(case)
    samples = []
    for iteration in range(1 + case['warmup'] + case['repeats']):
        start = time.perf_counter()
        if case['operation'] == 'read':
            result = read_pfm(path)
        else:
            result = write_pfm(path, values, scale=case['scale'])
        elapsed = time.perf_counter() - start
        if iteration > case['warmup']:
            samples.append(elapsed)
        if case['operation'] == 'read':
            np.testing.assert_array_equal(result, values * case['scale'])
        else:
            validate_write(path, values, case)
        del result
    return dict(samples_seconds=samples,
                process_peak_rss_kib=resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)


def positive(value):
    value = int(value)
    if value <= 0:
        raise argparse.ArgumentTypeError('must be positive')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sizes', nargs='+', type=positive, default=[1024, 2048])
    parser.add_argument('--repeats', type=positive, default=5)
    parser.add_argument('--warmup', type=positive, default=2)
    parser.add_argument('--temp-dir', type=Path, default=Path(tempfile.gettempdir()))
    parser.add_argument('--output', type=Path, default=ROOT / 'benchmarks/results/local.json')
    parser.add_argument('--reverse-order', action='store_true', help='reverse alternating library order for a second trial')
    parser.add_argument('--worker', help=argparse.SUPPRESS)
    parser.add_argument('--path', type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if sys.platform != 'linux':
        parser.error('Linux is required for consistent RSS reporting')
    if args.worker:
        print(json.dumps(python_worker(json.loads(args.worker), args.path)))
        return
    if importlib.metadata.version('justpfm') != '1.2.1':
        parser.error('install justpfm==1.2.1 in the benchmark environment')
    metadata = json.loads(command('cargo', 'metadata', '--no-deps', '--format-version', '1'))
    binary = Path(metadata['target_directory']) / 'release/examples/benchmark'
    subprocess.run(['cargo', 'build', '--locked', '--release', '--example', 'benchmark'], cwd=ROOT, check=True)
    source = Path(implementation.__file__)
    cpu = next(line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name'))
    files = list((ROOT / 'src').rglob('*.rs')) + [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock', ROOT / 'examples/benchmark.rs', Path(__file__).resolve()]
    environment = dict(
        utc_timestamp=datetime.now(timezone.utc).isoformat(),
        git_revision=command('git', 'rev-parse', 'HEAD'),
        git_status=command('git', 'status', '--short'),
        source_sha256={str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(files)},
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        rustc=command('rustc', '-Vv'), cargo=command('cargo', '-V'),
        python=sys.version, numpy=np.__version__, justpfm=importlib.metadata.version('justpfm'),
        justpfm_source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
        os=platform.platform(), cpu_model=cpu, cpu_count=os.cpu_count(),
        cpu_affinity=sorted(os.sched_getaffinity(0)), host_byte_order=sys.byteorder,
        load_average_before=os.getloadavg(), reverse_order=args.reverse_order,
        storage=command('findmnt', '-T', str(args.temp_dir), '-o', 'SOURCE,FSTYPE,OPTIONS', '-n'),
        temp_dir=str(args.temp_dir.resolve()),
        timing_policy='Serial isolated workers; alternating library order per case. One validation call plus requested warmups. Timers include library call, allocation and scaling, exclude imports, process startup, fixture generation, validation and result disposal. Release Rust default optimization; no target-cpu override.',
        cache_policy='Warm/cache-eligible, no cache eviction or fsync. Existing files are atomically replaced. This is not durable storage throughput.',
        memory_policy='Linux process-lifetime peak RSS in KiB per isolated worker, includes runtime, input, output, validation buffers and all calls; excludes OS page cache. Not per-call allocations. Different runtimes and validation implementations make absolute RSS non-equivalent. No allocator instrumentation.',
        layout_policy='Both receive contiguous top-first float32 input; little-endian files; grayscale and interleaved RGB. Rust returns contiguous top-first owned pixels; justPFM returns a writable top-first negative-row-stride array. No extra Python contiguity conversion is timed.',
    )
    report = dict(schema_version=1, environment=environment, cases=[])
    with tempfile.TemporaryDirectory(prefix='rustpfm-comparison-', dir=args.temp_dir) as temporary:
        path = Path(temporary) / 'image.pfm'
        for size in args.sizes:
            for channels in [1, 3]:
                for operation in ['read', 'write']:
                    for scale in [1.0, 2.0]:
                        case = dict(size=size, channels=channels, operation=operation, scale=scale, repeats=args.repeats, warmup=args.warmup)
                        order = ['rustpfm', 'justpfm'] if len(report['cases']) % 2 == 0 else ['justpfm', 'rustpfm']
                        if args.reverse_order:
                            order.reverse()
                        measured = dict(case, worker_order=order, payload_bytes=size * size * channels * 4)
                        for library in order:
                            fixture(path, case)
                            if library == 'rustpfm':
                                cmd = [str(binary), operation, str(size), str(channels), str(scale), str(args.repeats), str(args.warmup), str(path)]
                            else:
                                cmd = [sys.executable, str(Path(__file__).resolve()), '--worker', json.dumps(case), '--path', str(path)]
                            result = json.loads(subprocess.check_output(cmd, text=True))
                            result['median_seconds'] = statistics.median(result['samples_seconds'])
                            result['payload_mib_per_second'] = measured['payload_bytes'] / 1024**2 / result['median_seconds']
                            measured[library] = result
                        report['cases'].append(measured)
                        print(f"{size} {channels}ch {operation} scale={scale:g}: " + ', '.join(f"{lib} {measured[lib]['median_seconds'] * 1000:.3f} ms" for lib in order), file=sys.stderr)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(args.output)


if __name__ == '__main__':
    main()
