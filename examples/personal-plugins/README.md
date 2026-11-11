# Personal plugins for UI testing

These three small Node.js plugins run without network access, packages or model
credentials. They exercise the same import, review and enable flow as a user's
plugins. The builder supplies `rah_plugin.mjs` when importing them.

| Plugin | What to test |
| --- | --- |
| `text-lab` | Tool-only plugin; counts characters and words. Its optional `plugin.label` configuration changes the returned label. |
| `scratch-note` | Declares `storage`; reads and replaces a note in its own storage namespace. Writes still need the policy's permission. |
| `tally-board` | Contributes a page with increment/reset buttons. The count belongs to its process and resets on restart. |

From the repository root, create a bundle at a new path:

```powershell
node examples/personal-plugins/bundle.mjs "$env:TEMP\rah-ui-test-plugins.json"
```

An existing output file is refused. In **Settings > Plugins**, open **My plugins**
under **Plugin pages**, choose **Import**, use the bundle's absolute path, and inspect
the imported files. Each plugin starts disabled. Select its name, confirm that
you read its files, and enable it to test it. No configuration file edits are
needed. The `plugin-builder` feature must be enabled first if My plugins is absent.

Select an enabled plugin in the shared plugin list to edit its process settings.
For `text-lab`, enable the `plugin` field and enter
`{"label":"My text test"}`. This is plugin-specific configuration passed by the
process host, separate from its program, arguments and working directory.

Open **Tally board** under **Plugin pages** to try its buttons. Use **My plugins**
to disable or delete the examples afterward; deleting takes a backup first.
Automated browser tests import these exact sources into temporary directories,
so they never install the examples into your personal RAH data home.
