"""Updater regression checks without downloads, host installs or profile changes."""

from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib
import unittest

SCRIPT = Path(__file__).with_name("update.ps1").resolve()
SOURCE = SCRIPT.read_text(encoding="utf-8-sig")
ENABLE = re.search(r"\$enableLsp = @'\r?\n(.*?)\r?\n'@", SOURCE, re.S).group(1)


class ProfileTests(unittest.TestCase):
    def run_enable(self, home, config):
        return subprocess.run([sys.executable, "-I", "-", str(config), str(home)], input=ENABLE,
                              text=True, capture_output=True)

    def test_preserves_settings_comments_and_is_idempotent(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            config = home / "custom.toml"
            original = b'# my model\r\nmodel = "mock"\r\npermissions = "read-only"\r\nfeatures = ["format"]\r\n'
            config.write_bytes(original)
            result = self.run_enable(home, config)
            self.assertEqual(result.returncode, 0, result.stderr)
            updated = config.read_bytes()
            self.assertTrue(updated.startswith(original))
            parsed = tomllib.loads(updated.decode())
            self.assertEqual(parsed["permissions"], "read-only")
            self.assertEqual(parsed["features"], ["format"])
            self.assertEqual(parsed["plugin"], [{"use": "lsp-diagnostics"}])
            backups = list((home / "config-backups").rglob("*.before"))
            self.assertEqual(len(backups), 1)
            self.assertEqual(backups[0].read_bytes(), original)
            self.assertEqual(self.run_enable(home, config).returncode, 0)
            self.assertEqual(config.read_bytes(), updated)
            self.assertEqual(list((home / "config-backups").rglob("*.before")), backups)

    def test_existing_feature_or_custom_mount_is_untouched(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            config = home / "rah.toml"
            for original in [b'features = ["lsp", "shell"]\n',
                             b'[[plugin]]\nuse = "lsp-diagnostics"\nid = "custom-lsp"\nconfig = { defaults = false }\n']:
                config.write_bytes(original)
                result = self.run_enable(home, config)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(config.read_bytes(), original)
                self.assertFalse((home / "config-backups").exists())

    def test_invalid_config_is_not_replaced(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp)
            config = home / "rah.toml"
            for original in [b'features = [', b'features = "lsp"', b'plugin = {}']:
                config.write_bytes(original)
                self.assertNotEqual(self.run_enable(home, config).returncode, 0)
                self.assertEqual(config.read_bytes(), original)

    def test_fresh_profile_does_not_choose_a_model(self):
        with tempfile.TemporaryDirectory() as tmp:
            home = Path(tmp) / "profile"
            config = home / "rah.toml"
            result = self.run_enable(home, config)
            self.assertEqual(result.returncode, 0, result.stderr)
            parsed = tomllib.loads(config.read_text())
            self.assertNotIn("model", parsed)
            self.assertEqual(parsed["permissions"], "ask")


@unittest.skipUnless(sys.platform == "win32", "Windows updater")
class RuntimeTests(unittest.TestCase):
    def test_release_without_both_runtimes_fails_before_installing(self):
        with tempfile.TemporaryDirectory() as tmp:
            fixture = Path(tmp) / "release.ps1"
            fixture.write_text(r'''
param($Script, $Root)
$ErrorActionPreference = 'Stop'
function Invoke-RestMethod { return @{ tag_name = 'v1.0.0'; assets = @(@{ name = 'rah-1.0.0-portable.zip' }, @{ name = 'SHA256SUMS.txt' }) } }
function Invoke-WebRequest { throw 'Unexpected download' }
$install = Join-Path $Root 'install'
$refused = $false
try { & $Script -Source release -InstallDir $install -DataDir (Join-Path $Root 'profile') -NoShortcuts } catch {
    if ($_.Exception.Message -notlike '*graphify-bundled-lsp-bundled-portable.zip*') { throw }
    $refused = $true
}
if (-not $refused -or (Test-Path -LiteralPath $install)) { throw 'Incomplete release was accepted' }
''', encoding="utf-8")
            result = subprocess.run(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                                     str(fixture), str(SCRIPT), tmp], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_replacement_removes_stale_files_and_rejects_missing_or_overlapping_sources(self):
        with tempfile.TemporaryDirectory() as tmp:
            # Load only helper definitions from the AST, never the updater body.
            code = r"""
param($Script, $Root)
$ErrorActionPreference = 'Stop'
$tokens = $null; $errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile($Script, [ref]$tokens, [ref]$errors)
if ($errors.Count) { throw 'Updater syntax error' }
$ast.FindAll({param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -in @('Find-UpdateRuntime', 'Install-UpdateRuntime', 'Move-UpdateDirectory')}, $true) | ForEach-Object { Invoke-Expression $_.Extent.Text }
$source = Join-Path $Root 'source'
$install = Join-Path $Root 'install'
$existing = Join-Path $install 'lsp-runtime'
New-Item -ItemType Directory -Path $source, $existing | Out-Null
Set-Content -LiteralPath (Join-Path $source 'runtime.json') -Value '{}'
Set-Content -LiteralPath (Join-Path $source 'new.txt') -Value 'new'
Set-Content -LiteralPath (Join-Path $existing 'stale.txt') -Value 'stale'
$selected = Find-UpdateRuntime '' @((Join-Path $Root 'missing'), $source) 'Lsp'
if ($selected -ne $source) { throw 'Runtime discovery failed' }
Install-UpdateRuntime $source $install 'lsp-runtime'
if (Test-Path -LiteralPath (Join-Path $existing 'stale.txt')) { throw 'Stale runtime file retained' }
if (-not (Test-Path -LiteralPath (Join-Path $existing 'new.txt'))) { throw 'New runtime missing' }
$refused = $false
try { Install-UpdateRuntime $existing $install 'lsp-runtime' } catch { $refused = $true }
if (-not $refused) { throw 'Overlapping runtime was accepted' }
$refused = $false
try { Find-UpdateRuntime (Join-Path $Root 'missing') @($source) 'Lsp' } catch { $refused = $true }
if (-not $refused) { throw 'Missing explicit source silently fell back' }
if ((Get-ChildItem -LiteralPath $install).Count -ne 1) { throw 'Temporary runtime directories leaked' }
"""
            fixture = Path(tmp) / "test.ps1"
            fixture.write_text(code, encoding="utf-8")
            result = subprocess.run(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                                     str(fixture), str(SCRIPT), tmp], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
