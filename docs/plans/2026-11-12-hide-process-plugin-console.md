# Hide process plugin console windows

## Scope

Launch Windows process plugins with CREATE_NO_WINDOW in the shared spawn path,
covering manifest discovery and active plugin execution. Preserve stdio pipes,
scope ownership and non-Windows behavior. No service or protocol changes.

## Validation

Add a Windows fixture check that the plugin has no console during discovery
and mounting. Run process host tests, formatting and diff checks.
