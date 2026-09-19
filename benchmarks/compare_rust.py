#!/usr/bin/env python3
"""Linux file-read comparison with pinned zune-ppm; Python standard library only."""
import argparse
from array import array
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / 'benchmarks/rust-codecs'


def command(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def git_info(*args):
    result = subprocess.run(["git", *args], cwd=ROOT, text=True, capture_output=True)
    return result.stdout.strip() if result.returncode == 0 else None


def positive(value):
    value = int(value)
    if value <= 0:
        raise argparse.ArgumentTypeError('must be positive')
    return value


def fixture(path, size, channels):
    # Independent little-endian, bottom-first writer, bounded to one row.
    with path.open('wb') as stream:
        magic = 'Pf' if channels == 1 else 'PF'
        stream.write(f'{magic}\n{size} {size}\n-1.0\n'.encode())
        row_samples = size * channels
        for row in reversed(range(size)):
            values = array('f', ((i % 257) - 128 for i in range(row * row_samples, (row + 1) * row_samples)))
            assert values.itemsize == 4
            if sys.byteorder != 'little':
                values.byteswap()
            values.tofile(stream)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sizes', nargs='+', type=positive, default=[1024, 2048])
    parser.add_argument('--repeats', type=positive, default=5)
    parser.add_argument('--warmup', type=positive, default=2)
    parser.add_argument('--temp-dir', type=Path, default=Path(tempfile.gettempdir()))
    parser.add_argument('--output', type=Path, default=ROOT / 'benchmarks/results/rust-local.json')
    parser.add_argument('--row-order', choices=['top', 'bottom'], default='top')
    parser.add_argument('--reverse-order', action='store_true')
    args = parser.parse_args()
    if sys.platform != 'linux':
        parser.error('Linux is required for consistent RSS reporting')
    if any(size % 2 for size in args.sizes):
        parser.error('this comparison covers even heights only (zune-ppm 0.5.1 row-flip limitation)')
    manifest = str(PACKAGE / 'Cargo.toml')
    subprocess.run(['cargo', 'build', '--locked', '--release', '--manifest-path', manifest], cwd=ROOT, check=True)
    metadata = json.loads(command('cargo', 'metadata', '--locked', '--manifest-path', manifest, '--format-version', '1'))
    binary = Path(metadata['target_directory']) / 'release/rustpfm-codec-comparison'
    files = list((ROOT / 'src').rglob('*.rs')) + list((PACKAGE / 'src').rglob('*.rs'))
    files += [ROOT / 'Cargo.toml', PACKAGE / 'Cargo.toml', PACKAGE / 'Cargo.lock', Path(__file__).resolve()]
    environment = dict(
        utc_timestamp=datetime.now(timezone.utc).isoformat(),
        git_revision=git_info('rev-parse', 'HEAD'), git_status=git_info('status', '--short'),
        source_sha256={str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)},
        binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
        dependencies=[dict(name=p['name'], version=p['version'], source=p['source']) for p in metadata['packages']],
        rustc=command('rustc', '-Vv'), cargo=command('cargo', '-V'), python=sys.version,
        os=platform.platform(), cpu_model=next(line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),
        cpu_affinity=sorted(os.sched_getaffinity(0)), load_average_before=os.getloadavg(),
        storage=command('findmnt', '-T', str(args.temp_dir), '-o', 'SOURCE,FSTYPE,OPTIONS', '-n'),
        transparent_hugepage_policy=Path('/sys/kernel/mm/transparent_hugepage/enabled').read_text().strip() if Path('/sys/kernel/mm/transparent_hugepage/enabled').exists() else None,
        rustpfm_row_order=args.row_order, rustpfm_features='default',
        temp_dir=str(args.temp_dir.resolve()), reverse_order=args.reverse_order,
        timing_policy='Serial fresh workers, alternating order; one validation call, warmups, then samples. Includes file open/close, buffering, decoding, allocation, and output extraction. Excludes fixture generation, startup, validation and result disposal. Default release profile; no target-cpu override.',
        semantics='Scale 1 only, little endian, even square grayscale/RGB; contiguous output; Rust physical row order is recorded separately, zune output is top-first. zune-ppm does not encode PFM and ignores scale magnitude. Both default APIs are measured: rustPFM also checks exact payload size and trailing bytes, zune-ppm does not provide identical validation guarantees.',
        cache_policy='Warm/cache-eligible, no eviction or fsync; not durable disk throughput.',
        memory_policy='Linux process-lifetime peak RSS KiB, including runtime, decoded buffers and all calls; excludes OS page cache. Not per-call allocation measurement.',
    )
    report = dict(schema_version=1, environment=environment, cases=[])
    with tempfile.TemporaryDirectory(prefix='rustpfm-rust-comparison-', dir=args.temp_dir) as temporary:
        path = Path(temporary) / 'image.pfm'
        for size in args.sizes:
            for channels in [1, 3]:
                order = ['rustpfm', 'zune-ppm']
                if (len(report['cases']) % 2 == 1) != args.reverse_order:
                    order.reverse()
                case = dict(size=size, channels=channels, operation='read', scale=1.0, repeats=args.repeats, warmup=args.warmup, worker_order=order, payload_bytes=size * size * channels * 4)
                for library in order:
                    fixture(path, size, channels)
                    result = json.loads(command(str(binary), library, str(size), str(channels), str(args.repeats), str(args.warmup), str(path), args.row_order))
                    assert len(result['samples_seconds']) == args.repeats
                    result['median_seconds'] = statistics.median(result['samples_seconds'])
                    result['payload_mib_per_second'] = case['payload_bytes'] / 1024**2 / result['median_seconds']
                    case[library] = result
                report['cases'].append(case)
                print(f"{size} {channels}ch: " + ', '.join(f"{lib} {case[lib]['median_seconds'] * 1000:.3f} ms" for lib in order), file=sys.stderr)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(args.output)


if __name__ == '__main__':
    main()
