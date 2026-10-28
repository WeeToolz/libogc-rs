# Letter Flip

A Wii learning game for normal and horizontally mirrored English letters and digits. Use a GameCube controller in port 1 or a Wii Remote. The game uses the real controller, audio, and GameCube memory-card services.

## Play and practice

The title menu has **Play**, **How to Play**, and **Settings**. All game options are in Settings.

- **Practice** is the default. A round has 20 cards by default. A mistake does not remove a chance. Set the round length to 10–100 cards, in steps of 10.
- **Challenge** uses a **chance limit**, instead of lives. The default is 10 chances. Set the limit to 1–20. A mistake uses one chance. The round ends when no chances remain.
- Choose all letters, uppercase letters, lowercase letters, or numbers.
- A correct answer adds one point. Each card can be answered only once.
- After a mistake, the game shows both orientations, marks the correct one, and reads the answer. The minimum review time is adjustable from 0.5–5 seconds. Spoken feedback finishes before the next card appears.
- Correct answers use brief encouragement. Mistakes use gentle correction. Both modes end with **Round Complete**.
- Each deck shuffles its symbols before repeating them. Consecutive identical symbols are prevented at deck boundaries.
- **All Shapes** includes symmetric symbols. These are always normal. **Mirror Shapes** uses only symbols with a visible mirror difference and shuffles a balanced set of normal and mirrored orientations.
- Number cards show the digit and its readable number word. Only the digit is mirrored.

## Settings

Settings has three groups. Move through the rows with Up/Down. Left/Right changes a value. Select changes a value or runs an action. Back returns to the title.

| Group | Options |
| --- | --- |
| Game | Symbols, mode, chance limit, practice card count, symbol pool, review time |
| Sound and Display | Voice on/off, voice volume, effects on/off, effect volume, screen flash on/off |
| Controls and Save | Wii Remote position, preview sound, retry save, back |

Voice starts at 50 percent. Effects start at 35 percent. Screen flashes start off. Effects are short, finite tones, with a gradual attack and release. Game options apply to the next round. Sound and control options apply immediately.

The voice reads menu choices, settings names and values, help, controller instructions, and feedback. Use the repeat button to hear the current card or screen again. Turning Voice off also turns off menu narration.

## Controls

| Action | GameCube pad | Wii Remote upright | Wii Remote sideways |
| --- | --- | --- | --- |
| Normal answer / Select | A | A | 2 |
| Backwards answer / Back | B | B | 1 |
| Pause | START | + | + |
| Repeat voice | Z | − | − |
| Move through menus | D-pad | D-pad | Rotated D-pad |

For sideways use, hold the remote with its D-pad on the left. Set this position in Settings before playing. The default position is upright.

One controller owns the round. Other controllers cannot submit answers or move its menus. Pause offers Resume, Restart, Change Controller, and Menu. If the active controller disconnects, the round pauses. Connect a controller and select Resume. Pause preserves the card and the remaining feedback delay.

## Jamie (Premium) MP3 pack

The text source is [`voice-lines.tsv`](voice-lines.tsv). The generator is [`generate_voice.py`](generate_voice.py). Run this command from the repository root:

```sh
python3 examples/letter-flip/generate_voice.py
```

The script requires macOS, the installed **Jamie (Premium)** English UK voice, FFmpeg, and FFprobe. It does not select another voice if Jamie is missing. It writes the finished files to `examples/letter-flip/mp3/`, which is the build input directory.

The pack contains:

- `a.mp3` through `z.mp3`. Uppercase and lowercase use the same letter-name clip. The pack uses British English **zed** for Z.
- `zero.mp3` through `nine.mp3`.
- Encouragement such as `good_job.mp3`, `well_done.mp3`, and `continue.mp3`.
- Correction, help, menu, control, and save messages.
- Separate clips for setting names and numeric values. These can be queued in order, such as “Voice volume. Fifty. Percent.”

Edit the TSV text to change a phrase. Run the generator again and rebuild the game. Known unchanged clips are verified and reused. Changed source text is regenerated. The script refuses to replace an existing file without a generation record, or a file changed by hand, unless `--force` is given.

```sh
# Verify every MP3, its source record, hash, format, duration, and decoded samples.
python3 examples/letter-flip/generate_voice.py --check

# List all names and spoken text.
python3 examples/letter-flip/generate_voice.py --list

# Deliberately replace all clips named in the TSV file.
python3 examples/letter-flip/generate_voice.py --force
```

Each MP3 is mono, 22,050 Hz, at 64 kbit/s. The generator uses a speech rate of 155 words per minute and a conservative loudness target. It prepares each file in a temporary directory, encodes it, decodes it fully, checks that it contains sound, and then replaces the final file atomically. Failures return a nonzero exit code. Previous completed files are preserved if preparation fails.

The complete `mp3/` directory is Git ignored, including generated audio, temporary files, and generation records. The Python script and TSV source remain available to Git. Generate the local pack before building if you want the narration clips embedded. The build also accepts an absent directory; missing clips are reported on screen at runtime.

The build embeds recognized clips from this directory. A missing clip or playback failure is shown on screen. A new card or screen cancels obsolete speech. Message sequences play in order. Submission to the MP3 decoder is not treated as completed playback.

## Settings and score storage

The game uses GameCube memory-card slots A, then B. It does not format cards. Use **Settings → Retry Save** after inserting or changing a card.

New state uses the separate files `LFSTATE0` and `LFSTATE1`, with game code `LFLP` and company code `RS`. Each file has its own card allocation. Writes alternate between the files. Each write is closed, read back, and checked before the committed in-memory state changes. Settings and records have version, bounds, reserved-byte, and CRC checks.

Records are separate for category, mode, symbol pool, and practice card count or challenge chance limit. The old `LETTERFLIP` file is read only for migration. Its score is preserved as **Previous Game Record**, because its original category and mode are unknown. It is never replaced automatically.

Saves occur on the title, settings, or completed-round screen. The display distinguishes pending, saving, saved, and failed states. After a failed write, the game keeps the pending data and stops automatic writes. Use Retry Save to remount, load the stored records, preserve the higher scores, and retry with the current settings.

If one state file is damaged, a valid other file can be loaded. The damaged file is preserved, and further writes are blocked. Unknown or malformed files are never replaced automatically. Keep the files for diagnosis or recovery. Use another supported card to save new state.

Without a card, settings and records last for the current session. Wii models without GameCube card slots cannot use this storage path. No firmware or NAND writes are required.

## Build

From the repository root:

```sh
python3 examples/letter-flip/generate_voice.py
cargo xtask build-dol examples --example letter-flip
```

Output: `target/xtask/custom-dols/examples-letter-flip.dol`.

This is a Wii PowerPC application. It is not a native GameCube console or desktop game. Use the repository's target toolchain.

## Verification

The host checks use the same game, session, narration, speech, save-journal, tone, and rendering code as production:

```sh
rustc --edition=2021 --test examples/letter-flip/letter-flip.rs -o /tmp/letter-flip-tests
/tmp/letter-flip-tests
python3 -m unittest discover -s examples/letter-flip -p test_generate_voice.py -v
python3 examples/letter-flip/generate_voice.py --check
```

Tests cover practice and challenge limits, score separation, deck coverage, controller ownership, disconnect and resume, elapsed-time feedback, spoken feedback completion, obsolete speech cancellation, decoder failures, save write/read/verification failures, reload from serialized data, fixed CRC bytes, all settings narration, percent rendering, and screen bounds. Generator tests check malformed catalogs, actual tool failure, and a real silent MP3.

A real-hardware probe is in [`hardware_probe.rs`](hardware_probe.rs). Build it separately:

```sh
cargo xtask build-dol examples --example letter-flip-hardware-probe --probe-features letter-flip-hardware-probe
```

Run that DOL on a Wii with a supported memory card and controller. The probe waits for deliberate confirmation before writing **LFPROBE0** and **LFPROBE1**. It uses the production storage and audio paths. It tests two writes, read-back, explicit unmount, remount, exact settings and score reload, representative symbol speech, encouragement, and finite effects. It leaves the probe files on the card. It does not change normal game saves. A host run is not a hardware pass.

Real-Wii checks remain necessary. Test both remote positions, GameCube input, audio audibility, television margins, supported video modes, card removal and reinsertion, and restart persistence. Do not remove power to test interrupted writes on a valuable card. Use controlled failure tests and a suitable test card first.
