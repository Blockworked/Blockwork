"""Black-box host checks for the device runner; no Android device is accessed."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest


class DeviceRunnerTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='blockwork-runner-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        scripts = self.root / 'scripts'
        scripts.mkdir()
        self.runner = scripts / 'test-android-device.py'
        shutil.copyfile(Path(__file__).resolve().parents[1] / self.runner.name, self.runner)
        adb = self.root / 'sdk/platform-tools/adb'
        adb.parent.mkdir(parents=True)
        self.record = self.root / 'adb.jsonl'
        adb.write_text(textwrap.dedent('''\
            #!/usr/bin/env python3
            import json, os, pathlib, shlex, sys
            args = sys.argv[1:]
            with open(os.environ['FIXTURE_ADB_RECORD'], 'a') as log: log.write(json.dumps(args)+'\\n')
            assert args[:2] == ['-s', 'dedicated-device'], args
            command = args[2:]
            scenario = os.environ.get('FIXTURE_ADB_SCENARIO', 'pass')
            if command == ['get-state']:
                if scenario == 'missing': print('error: device not found', file=sys.stderr); sys.exit(1)
                print('device')
            elif command[:1] == ['install']: print('Success')
            elif command[:1] == ['shell']:
                remote = shlex.split(command[1]) if len(command) == 2 else command[1:]
                if remote[:2] == ['am', 'instrument']:
                    if scenario == 'fail': print('INSTRUMENTATION_STATUS_CODE: -2\\nFAILURES!!!\\nINSTRUMENTATION_CODE: -1')
                    else: print('OK (1 test)\\nINSTRUMENTATION_CODE: -1')
                elif remote[:2] == ['getprop', 'ro.product.cpu.abi']: print('x86_64')
                elif remote[:2] == ['getprop', 'ro.product.cpu.abilist']: print('x86_64,x86')
                elif remote[:2] == ['getprop', 'ro.build.version.sdk']: print('30')
                elif remote[:2] == ['getprop', 'ro.build.fingerprint']: print('fixture/device')
                elif remote[:2] == ['getconf', 'PAGE_SIZE']: print('16384')
                elif remote[:2] == ['settings', 'get']: print('com.example/.Other:com.blockworked.blockwork/.AutomationService')
                elif remote[:2] in [['settings', 'put'], ['settings', 'delete']]: pass
                else: raise AssertionError(remote)
            else: raise AssertionError(command)
            '''))
        adb.chmod(0o755)
        self.environment = {**os.environ, 'ANDROID_SDK_ROOT': str(self.root / 'sdk'), 'FIXTURE_ADB_RECORD': str(self.record)}
        self.results = self.root / 'results'

    def run_cli(self, *options, scenario='pass'):
        return subprocess.run([sys.executable, str(self.runner), '--serial', 'dedicated-device', '--results', str(self.results), *options],
                              capture_output=True, text=True, env={**self.environment, 'FIXTURE_ADB_SCENARIO': scenario}, timeout=10)

    def calls(self):
        return [json.loads(line)[2:] for line in self.record.read_text().splitlines()] if self.record.exists() else []

    def test_success_records_device_and_instrumentation_evidence(self):
        result = self.run_cli('--skip-install', '--class', 'com.example.Test$Inner#method')
        self.assertEqual(result.returncode, 0, result.stderr)
        metadata = json.loads((self.results / 'device.json').read_text())
        self.assertEqual(metadata['page_size'], '16384')
        self.assertEqual(metadata['class_filter'], 'com.example.Test$Inner#method')
        self.assertIn('OK (1 test)', (self.results / 'instrumentation.txt').read_text())
        self.assertFalse(any(call[0] == 'install' for call in self.calls()))
        import shlex
        remote = next(shlex.split(call[1]) for call in self.calls() if call[0] == 'shell' and len(call) == 2 and 'instrument' in call[1])
        self.assertEqual(remote[remote.index('class')+1], 'com.example.Test$Inner#method')

    def test_adb_exit_zero_does_not_hide_junit_failure(self):
        result = self.run_cli('--skip-install', scenario='fail')
        self.assertEqual(result.returncode, 1)
        self.assertIn('FAILURES!!!', (self.results / 'instrumentation.txt').read_text())

    def test_revocation_setting_is_preserved_and_restored_after_failure(self):
        import shlex
        result = self.run_cli('--skip-install', '--include-revocation', scenario='fail')
        self.assertEqual(result.returncode, 1)
        original = json.loads((self.results / 'device.json').read_text())['original_enabled_accessibility_services']
        restores = [shlex.split(call[1]) for call in self.calls() if call[0] == 'shell' and len(call) == 2 and 'settings put' in call[1]]
        self.assertEqual(restores, [['settings', 'put', 'secure', 'enabled_accessibility_services', original]])

    def test_missing_device_does_not_install_or_change_settings(self):
        result = self.run_cli('--skip-install', scenario='missing')
        self.assertEqual(result.returncode, 1)
        self.assertEqual(self.calls(), [['get-state']])
        self.assertFalse(self.results.exists())

    def test_invalid_class_filter_is_rejected_before_adb(self):
        result = self.run_cli('--skip-install', '--class', 'com.example.Test;id')
        self.assertEqual(result.returncode, 2)
        self.assertEqual(self.calls(), [])

    def test_install_only_allows_test_apk_and_does_not_enable_accessibility(self):
        artifacts = self.root / 'dist/android'
        artifacts.mkdir(parents=True)
        for file in ['blockwork-x86_64-debug.apk', 'blockwork-fixture-debug.apk', 'blockwork-x86_64-androidTest.apk']:
            (artifacts / file).write_bytes(b'fake APK for command verification')
        result = self.run_cli('--install-only')
        self.assertEqual(result.returncode, 0, result.stderr)
        installs = [call for call in self.calls() if call[0] == 'install']
        self.assertEqual(len(installs), 3)
        self.assertEqual(installs[-1][1:3], ['-r', '-t'])
        self.assertFalse(any('instrument' in ' '.join(call) or 'settings' in ' '.join(call) for call in self.calls()))
        self.assertFalse(self.results.exists())


if __name__ == '__main__':
    unittest.main()
