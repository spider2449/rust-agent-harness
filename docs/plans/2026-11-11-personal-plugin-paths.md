# Personal plugin paths

Use `plugins` consistently for the plugin-builder relative default and example.
The distribution resolves relative data paths against the data home, normally
`~/.rah`. Do not silently rewrite explicit custom directories.

Add a regression test resolving the public example through the host data-path
resolver and compare it with an omitted directory. Document the relative-path
rule. No kernel or service-contract change.

For the user-authorized local repair, back up rah.toml, copy all four personal
plugins from `~/.rah/.rah/plugins` to `~/.rah/plugins`, verify file hashes, and
update the builder setting plus saved mount paths. Preserve the old directory
for rollback and running instances. Do not restart services or auto-enable
plugins. The new saved configuration applies on restart.

## Outcome

- Updated the plugin fallback, public configuration example and documentation.
- Regression coverage resolves the example and omitted setting to the same
  data-home/plugins directory; explicit custom directories remain supported.
- Local repair copied scratch-note, tally-board, text-lab and thunderbird-mail
  (21 files). Verified copies before rewriting generated mount paths, then
  checked TOML structure, unchanged plugin content and existing entry files.
- Updated the user's builder directory and both saved personal process mounts.
  Preserved all other settings, including policy and interpreter selection.
  No old-root references remain in copied text files.
- Preserved the original plugin directory and backed up the original config
  under the data home's path-repair-backups directory. Running instances were
  not restarted; restart RAH before using the repaired saved configuration.
- Plugin-builder tests: 24 passed. Data-path tests: 6 passed. Formatting,
  plugin-builder all-target Clippy, CLI library Clippy and diff checks passed.

No ADR, kernel, service-contract or dependency changes. Source changes are not
committed. Next step: restart the application and confirm the My plugins panel
uses the new directory before retiring the old copy.
