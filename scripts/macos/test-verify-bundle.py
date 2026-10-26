#!/usr/bin/env python3
"""Exercise the bundle validator against real, signed native Mach-O fixtures."""
import pathlib
import plistlib
import shutil
import subprocess
import sys
import tempfile

validator = pathlib.Path(__file__).with_name('verify-bundle.py')


def run(*args):
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f'{args!r} failed ({result.returncode}):\n{result.stdout}{result.stderr}')
    return result


with tempfile.TemporaryDirectory(prefix='blockwork-validator-') as temporary:
    root = pathlib.Path(temporary)
    bundle = root / 'Blockwork.app'
    binaries = bundle / 'Contents/MacOS'
    frameworks = bundle / 'Contents/Frameworks'
    binaries.mkdir(parents=True)
    frameworks.mkdir()
    (bundle / 'Contents/Info.plist').write_bytes(plistlib.dumps({
        'CFBundleExecutable': 'Blockwork', 'CFBundleIdentifier': 'com.blockworked.ValidatorFixture',
        'CFBundlePackageType': 'APPL', 'CFBundleName': 'Blockwork',
        'CFBundleVersion': '1.0', 'CFBundleShortVersionString': '1.0',
        'LSMinimumSystemVersion': '13.0'}))
    main = root / 'main.c'
    main.write_text('int main(void) { return 0; }\n')
    run('clang', str(main), '-o', str(binaries / 'Blockwork'))
    shutil.copy2(binaries / 'Blockwork', binaries / 'blockwork-daemon')
    renderer = binaries / 'Blockwork Helper (Renderer)'
    shutil.copy2(binaries / 'Blockwork', renderer)
    library = frameworks / 'libprobe.dylib'
    source = root / 'library.c'
    source.write_text('int blockwork_probe(void) { return 42; }\n')
    run('clang', '-dynamiclib', str(source), '-Wl,-headerpad_max_install_names',
        '-Wl,-install_name,./libprobe.dylib', '-o', str(library))

    def sign():
        for image in [binaries / 'Blockwork', binaries / 'blockwork-daemon', renderer, library]:
            run('codesign', '--force', '--sign', '-', str(image))
        run('codesign', '--force', '--sign', '-', str(bundle))

    sign()
    valid = run(sys.executable, str(validator), str(bundle))
    assert 'Validated 4 Mach-O images' in valid.stdout, valid.stdout
    # A genuine load dependency must still be rejected. Re-sign both levels
    # so the failure cannot be explained by the prior signature check.
    host_only = str(root / 'missing-host-library.dylib')
    run('install_name_tool', '-change', '/usr/lib/libSystem.B.dylib', host_only, str(library))
    sign()
    run('codesign', '--verify', '--deep', '--strict', str(bundle))
    invalid = subprocess.run([sys.executable, str(validator), str(bundle)], capture_output=True, text=True)
    assert invalid.returncode != 0 and 'references a host library: ' + host_only in invalid.stderr, invalid.stderr
    print('Passed: parenthesized helper and relative install ID accepted; actual host load dependency rejected.')
