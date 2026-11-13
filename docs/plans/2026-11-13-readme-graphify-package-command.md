# Document the Graphify packaging option

Add explicit Windows packaging commands with -GraphifyRuntime and -SkipBuild
to the repository and bundled package READMEs. Explain runtime preparation,
lightweight versus bundled output, and installation-time Graphify activation.
Refresh the standalone distribution README and verify README equality and Git
diff formatting. No runtime or contract behavior changes.

The prior staging directory, ZIP and installer are no longer present in dist.
An attempted artifact refresh could not run without them. Only the standalone
distribution README was updated; the next packaging run includes the new text.
