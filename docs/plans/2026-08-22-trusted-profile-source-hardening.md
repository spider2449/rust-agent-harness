# Task 030 plan: trusted profile source hardening

## Scope

Harden the explicit static profile file boundary before the existing Task 028
parser and capability constructors run. The loader will reject non-absolute,
linked/reparse, non-regular, oversized, and non-UTF-8 sources, then parse only
bounded bytes read from the same opened file handle.

## Design

1. Keep the public `TrustedStaticProfile::load` entrypoint and add a private
   source reader in `rah-tools`.
2. Validate the supplied absolute path topology before opening it: every
   existing component must be a directory except the final regular file, and
   links/reparse points fail closed. Windows accepts only normal drive-rooted
   paths and rejects UNC, verbatim/device prefixes, and ADS syntax.
3. Open once, validate the opened object as a regular file, recheck the source
   topology, and read a fixed maximum plus one byte from that handle. Unix also
   uses `O_NOFOLLOW` and compares the opened object with the post-open path
   identity. Decode only strict UTF-8 before handing bytes to the existing JSON
   parser.
4. Keep failures redacted and return no `TrustedStaticProfile` until source,
   parse, resource, and capability construction all succeed.
5. Document the boundary and its non-guarantees: it validates object/type/link
   properties but cannot prove exclusive ACL ownership or eliminate every
   external replacement race on every filesystem.

## Validation

Run focused `rah-tools` and CLI tests, representative Windows CLI cases, then
the workspace format, check, test, clippy, diff-check, and metadata commands.
