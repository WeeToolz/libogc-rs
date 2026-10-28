# Digital stick projection

`ogc_controller::DigitalStickMapper` converts normalized analog X and Y values
to strict digital directions. The caller supplies a real `ControllerId` and a
threshold for each sample. The mapper does not infer a source, read hardware,
or select a calibration value.

Values at the threshold remain neutral. Values below the negative threshold
map left or down. Values above the threshold map right or up. The caller can
use `8192` for the common normalized dead zone or `9599` when preserving the
legacy compact-axis 600/2048 dead zone. This keeps device calibration and app
policy outside the shared transition engine.

The mapper delegates physical edge tracking and the held-at-connect gate to
`InputTransitions`. `held`, `down`, and `up` report physical direction state.
The separate `action_held` and `action_down` fields stay empty for each new
identity until its stick reports neutral once. Use those action fields when a
continuous app action must not start from a stick already held at connection.
Use `action_down`, `action_held`, `action_up`, and reset `action_released` as a
complete app-facing edge contract. The physical fields and reset `released`
remain available for apps that need raw direction state.
The mapper matches slots by full source and generation, so sample order can
change between calls. A removed or replaced ID produces a separate release
observation before the new generation begins its connection gate.
Input count, source identity, threshold, and duplicate-source checks run before
the mapper changes state. Transition state is staged and commits only after
every operation succeeds, so generation exhaustion cannot consume a release
or discard an older identity. The API uses fixed arrays and does not allocate.

No corresponding application, application, or other app-level stick-to-action source
was found in the configured `C-sources` tree. The direction rules match the
existing Rust application utility's strict comparisons. This shared engine only
proves portable state and edge behavior; it does not verify hardware scans or
application-specific controls.
