# LWP workspace allocator public setter in Dolphin

Dolphin 2609a ran `runtime_probe` with 917 passes and zero failures. The runner
used a private NAND copy and removed it after the run. The probe DOL SHA-256 is
`50163cbd2d447854ad63aa08b70b44080bf25ea3b3570ab5f7acdd0723697d14`. The run
was built from revision `772e67f61895323b19305fb7f706f62ea76c83f2` with a dirty
worktree; `suite.json` records the run configuration.

The LWP lifecycle probe now calls both public setters. It installs a wrapper
through `ogc_lwp_driver::set_workspace_allocator`, creates and joins a thread
whose stack uses that allocator, then restores the prior pair through
`ogc_lwp::sys::set_workspace_allocator`. It checks that the same stack pointer
is freed. It also checks that an allocation failure rejects thread creation,
does not leave a live thread handle, and permits a later successful create and
join. The two named checks passed:

- `LWP tracked workspace frees the joined thread stack`
- `LWP failed stack allocation releases its slot and recovers`

The failure occurs before the allocator returns a stack. The probe does not
test cleanup after a later thread-creation failure. This is Dolphin evidence;
physical Wii behavior remains open.

The run evidence includes `probe.log`, the captured transport, compressed raw
Dolphin log, and `suite.json`.
