#!/usr/bin/env python3
"""Validate a macOS app's signatures, architectures and bundled Mach-O dependencies."""
import pathlib
import plistlib
import re
import subprocess
import sys

MAGIC = {b'\xfe\xed\xfa\xce', b'\xce\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xcf\xfa\xed\xfe',
         b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca', b'\xca\xfe\xba\xbf', b'\xbf\xba\xfe\xca'}


def run(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def main():
    bundle = pathlib.Path(sys.argv[1]).resolve(strict=True)
    with (bundle / 'Contents/Info.plist').open('rb') as stream:
        info = plistlib.load(stream)
    executable = bundle / 'Contents/MacOS' / info['CFBundleExecutable']
    daemon = bundle / 'Contents/MacOS/blockwork-daemon'
    if not executable.is_file() or not daemon.is_file():
        raise RuntimeError('the editor and its sibling daemon must both be bundled')
    run('codesign', '--verify', '--deep', '--strict', str(bundle))
    required_arches = set(run('lipo', '-archs', str(executable)).split())
    images = set()
    for path in bundle.rglob('*'):
        if path.is_symlink() and bundle not in path.resolve().parents:
            raise RuntimeError(f'symlink escapes the bundle: {path}')
        if path.is_file():
            with path.open('rb') as stream:
                if stream.read(4) in MAGIC:
                    images.add(path.resolve())
    rpaths = {}
    for image in images:
        arches = set(run('lipo', '-archs', str(image)).split())
        if not required_arches.issubset(arches):
            raise RuntimeError(f'{image} lacks required architecture(s): {required_arches - arches}')
        commands = run('otool', '-l', str(image))
        rpaths[image] = re.findall(r'cmd LC_RPATH\s+cmdsize \d+\s+path (.*?) \(offset', commands)
        for path in rpaths[image]:
            if path.startswith('/') and not path.startswith(('/System/Library/', '/usr/lib/')):
                raise RuntimeError(f'{image} has a host-specific search path: {path}')
    def expand(path, image):
        return pathlib.Path(path.replace('@executable_path', str(executable.parent))
                            .replace('@loader_path', str(image.parent))).resolve()
    for image in images:
        for line in run('otool', '-L', str(image)).splitlines():
            if not line.startswith('\t'):
                continue
            dependency = line.strip().split(' (compatibility version', 1)[0]
            if dependency.startswith(('/System/Library/', '/usr/lib/')):
                continue
            if dependency.startswith('@rpath/'):
                suffix = dependency[len('@rpath/'):]
                candidates = [expand(path, image) / suffix for path in rpaths[image]]
                candidates += [expand(path, executable) / suffix for path in rpaths[executable.resolve()]]
            elif dependency.startswith(('@loader_path/', '@executable_path/')):
                candidates = [expand(dependency, image)]
            else:
                raise RuntimeError(f'{image} references a host library: {dependency}')
            if not any(path.is_file() and bundle in path.resolve().parents for path in candidates):
                raise RuntimeError(f'{image} has an unresolved bundled dependency: {dependency}')
    print(f'Validated {len(images)} Mach-O images for {", ".join(sorted(required_arches))}: signatures and dependencies are self-contained.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'Bundle verification failed: {error}', file=sys.stderr)
        sys.exit(1)
