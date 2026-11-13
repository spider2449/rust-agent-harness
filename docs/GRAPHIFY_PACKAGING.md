# Graphify packages without uv

There are two package variants. The normal/lightweight package prepares Graphify
through uv when the user explicitly runs `rah graphify install`. A bundled
package contains Node, a portable Python interpreter and Graphify dependencies.
Its target computer needs no uv, separate Python/Node installation, pip or
network access. Both retain explicit read policy and scoped process supervision.

## Prepare a runtime

On the build computer, supply a complete portable Python 3.10+ distribution for
the target platform/architecture, Node.js 20+ and Node's matching LICENSE file.
Keep Python's license in its distribution root. A copied venv is refused because
it is not a portable interpreter. Windows embeddable Python is also supported
when dependencies are supplied separately; its relative import-path file is
configured in the generated bundle. No system interpreter or environment is
modified.

For a portable Python distribution that includes pip:

```powershell
& 'C:\build\python\python.exe' scripts/prepare-graphify-runtime.py `
  --python-root 'C:\build\python' `
  --node 'C:\build\node\node.exe' `
  --node-license 'C:\build\node\LICENSE' `
  --out '.rah\graphify-runtime'

.\scripts\package.ps1 -GraphifyRuntime '.rah\graphify-runtime'
```

The preparation step installs graphifyy[mcp]==0.9.107 with pip into the bundle.
Use `--wheelhouse DIR --offline` for a predownloaded wheelhouse, or
`--site-packages DIR --offline` to copy already prepared dependencies for the same
Python ABI/platform without pip or downloads. All package metadata/licenses are
preserved. Virtualenv bootstrap hooks are removed from the generated copy.
Existing output directories and overlapping source/output paths are refused.

The manifest records platform, architecture, Python/Node versions and installed
package versions. Validation launches the copied programs, checks the pinned
Graphify version and verifies that Python's prefix/import paths are contained
in the bundle. The runtime and RAH executable must target the same platform.
Use the matching Node distribution's LICENSE, not npm's separate license.

## Package variants

Windows `package.ps1 -GraphifyRuntime DIR` produces both:

- dist/rah-VERSION-graphify-bundled-portable.zip
- dist/rah-VERSION-graphify-bundled-setup.exe

The installer includes the runtime directory. Normal package names remain
unchanged and coexist with these artifacts. `-SkipBuild` requires a current RAH
binary supporting bundled runtime discovery. The script validates the runtime
before staging and only replaces its own generated staging directory.

On Linux, prepare a native portable runtime with the same Python script and run:

```sh
./scripts/package.sh --graphify-runtime /opt/build/graphify-runtime
```

It creates the separate graphify-bundled tarball. Its install.sh copies the
runtime beside the installed binaries. Linux runtime portability depends on the
supplied Python/Node distribution and system ABI; validate on your target OS.
Windows runtime validation does not prove Linux compatibility.

## Use the package

Install/unpack the complete bundle, then run `rah graphify install` and reopen
the matching RAH Desktop binary. The runtime is detected beside RAH, including
when PATH has no uv, Python or Node. It is checked and used directly with no
downloads. An existing copied configuration is relocated and backed up.

For an explicit bundle location:

```powershell
rah graphify install --runtime-dir 'D:\RAH\graphify-runtime' --offline
```

For an existing prepared Python environment, use `--python` and optionally
`--node` instead. Install the pinned Graphify package into that environment
yourself; RAH never silently changes it. Re-run installation after moving a
portable folder so stored program paths follow the new location.

References: [Python Windows embedding](https://docs.python.org/3/using/windows.html#the-embeddable-package)
and [pip target installation](https://pip.pypa.io/en/stable/cli/pip_install/#cmdoption-target).
