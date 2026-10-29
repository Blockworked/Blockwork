#!/usr/bin/env python3
"""Install and run Blockwork fixture checks on one explicitly named Android device."""
from pathlib import Path
import argparse
import datetime
import os
import re
import shutil
import shlex
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--serial', required=True, help='Explicit adb serial of a dedicated, unlocked test device')
    parser.add_argument('--abi', choices=['arm64-v8a', 'x86_64'])
    parser.add_argument('--install-only', action='store_true', help='Install the three APKs, then enable accessibility manually')
    parser.add_argument('--skip-install', action='store_true')
    parser.add_argument('--include-revocation', action='store_true', help='Temporarily remove and restore this service from enabled accessibility services')
    parser.add_argument('--class', dest='test_class', help='Optional JUnit class or class#method filter')
    parser.add_argument('--results', type=Path)
    args = parser.parse_args()
    if args.install_only and args.skip_install:
        parser.error('--install-only and --skip-install cannot be combined')
    if args.test_class and not re.fullmatch(r'[A-Za-z0-9_.$]+(?:#[A-Za-z0-9_$]+)?(?:,[A-Za-z0-9_.$]+(?:#[A-Za-z0-9_$]+)?)*', args.test_class):
        parser.error('--class must contain JUnit package/class names and optional #method names')
    root = Path(__file__).resolve().parents[1]
    sdk = os.environ.get('ANDROID_SDK_ROOT') or os.environ.get('ANDROID_HOME')
    adb = str(Path(sdk) / 'platform-tools/adb') if sdk else shutil.which('adb')
    if not adb or not Path(adb).is_file():
        parser.error('Set ANDROID_SDK_ROOT or put adb on PATH')
    prefix = [adb, '-s', args.serial]

    def checked(*command):
        result = subprocess.run([*prefix, *command], text=True, capture_output=True, timeout=120)
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip() or f'adb failed: {command[0]}')
        return result.stdout.strip()

    if checked('get-state') != 'device':
        raise RuntimeError('The selected device is not connected and authorized')
    abi = args.abi or checked('shell', 'getprop', 'ro.product.cpu.abi')
    supported = checked('shell', 'getprop', 'ro.product.cpu.abilist').split(',')
    if abi not in ('arm64-v8a', 'x86_64') or abi not in supported:
        raise RuntimeError(f'Unsupported/mismatched ABI {abi}; device supports {supported}')
    if not args.skip_install:
        artifacts = [root / 'dist/android' / name for name in [f'blockwork-{abi}-debug.apk', 'blockwork-fixture-debug.apk', f'blockwork-{abi}-androidTest.apk']]
        for artifact in artifacts:
            if not artifact.is_file():
                raise RuntimeError(f'Missing {artifact.name}; build the main and validation APKs first')
        for artifact in artifacts:
            options = ['-r', '-t'] if artifact.name.endswith('-androidTest.apk') else ['-r']
            print(checked('install', *options, str(artifact)))
    if args.install_only:
        print('Installed. Open Blockwork and enable Blockwork automation in Accessibility settings on this dedicated device.')
        print('Then rerun with --skip-install. No app storage was cleared or accessibility grant changed.')
        return 0
    destination = args.results or root / 'target/android/device-validation' / re.sub(r'[^a-zA-Z0-9_.-]', '_', args.serial) / datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    destination.mkdir(parents=True, exist_ok=True)
    metadata = {'serial': args.serial, 'abi': abi, 'sdk': checked('shell', 'getprop', 'ro.build.version.sdk'),
                'fingerprint': checked('shell', 'getprop', 'ro.build.fingerprint'), 'page_size': checked('shell', 'getconf', 'PAGE_SIZE'),
                'include_revocation': args.include_revocation, 'class_filter': args.test_class}
    original_services = None
    if args.include_revocation:
        original_services = checked('shell', 'settings', 'get', 'secure', 'enabled_accessibility_services')
        metadata['original_enabled_accessibility_services'] = original_services
    import json
    (destination / 'device.json').write_text(json.dumps(metadata, indent=2) + '\n')
    command = ['am', 'instrument', '-w', '-r']
    if args.include_revocation:
        command += ['-e', 'includeRevocation', 'true']
    command += ['-e', 'class', args.test_class or 'com.blockworked.blockwork.FixtureDeviceTest',
                'com.blockworked.blockwork.test/androidx.test.runner.AndroidJUnitRunner']
    print('Running real device checks. The secure-capture check intentionally fails if Android returns masked pixels.')
    try:
        result = subprocess.run([*prefix, 'shell', shlex.join(command)], text=True, capture_output=True, timeout=600)
    except subprocess.TimeoutExpired as error:
        def decode(value):
            return value.decode(errors='replace') if isinstance(value, bytes) else value or ''
        (destination / 'instrumentation.txt').write_text(decode(error.stdout) + decode(error.stderr))
        raise RuntimeError(f'Instrumentation timed out; results in {destination}. Check the device for a still-running test.') from error
    finally:
        if original_services is not None:
            restore = ['settings', 'delete', 'secure', 'enabled_accessibility_services'] if original_services == 'null' else ['settings', 'put', 'secure', 'enabled_accessibility_services', original_services]
            try:
                checked('shell', shlex.join(restore))
            except (RuntimeError, subprocess.SubprocessError) as error:
                raise RuntimeError(f'Could not restore accessibility services; original value is recorded in {destination / "device.json"}: {error}') from error
    output = result.stdout + result.stderr
    (destination / 'instrumentation.txt').write_text(output)
    print(output)
    print(f'Device evidence: {destination}')
    passed = result.returncode == 0 and re.search(r'^INSTRUMENTATION_CODE: -1\s*$', output, re.M) and re.search(r'OK \([1-9][0-9]* tests?\)', output)
    failed = re.search(r'INSTRUMENTATION_STATUS_CODE: -(?:1|2)\b|INSTRUMENTATION_FAILED|FAILURES!!!', output)
    return 0 if passed and not failed else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (RuntimeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
