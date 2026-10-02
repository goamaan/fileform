# Breadth-first parity plan

Updated October 1, 2026 after the user's explicit change in priority. This replaces
native-domain-first sequencing. The goal still includes full original parity and
a verified downloadable macOS/Windows release; nothing is removed from the ledger.

## Stable checkpoint

The PDF/HEIC detail work is closed at `073d44d`, with its relevant Windows native
checks passing. `916cc59` then added a complete packaged native runtime and passed
Mac packaging; its Windows packaging exposed an architecture-name mismatch, fixed
in the shared-intake increment below. Do not expand domain fixture corpora now.

The next stable increment connects 13 main actions through one file-first intake
and action-first chooser, with a shared inspector/task planner, source binding,
cancellation, native save dialogs and open/reveal results. Real native integration
covers every action across tables/images/PDF/audio/video/text; representative
packaged Mac GUI checks cover action-first PDF text extraction, PDF splitting,
table conversion, media intake/audio extraction and both themes. This is broad exposure, not complete
parity: no claim is made for untested drop handling, all GUI variants, automatic
languages, playback/editing, batches, URL downloads, setups or persistence.

## Execution order

1. **Make the shared application foundation usable.** Bundle the reviewed native
   tools/models/libraries for development and Windows builds, with notices and
   explicit production gates. Replace category intake with one file picker/drop
   surface and relevant actions, plus action-first entry. Establish one shared
   source/job/result model with source binding, progress, cancellation, clear errors,
   save/reveal/open and filename-conflict handling. Keep the engine outside the UI.
2. **Connect each main file family end-to-end.** Reuse the verified native routes
   in the table below. Finish a real input → action/options → result flow for one
   family, verify it, and move to the next family. Do not deepen a finished family's
   codec/metadata edge cases while another family has no usable desktop route.
3. **Close broad missing behavior.** Implement direct media URLs and automatic
   multilingual OCR, finish mixed batches, basic PDF/media editing, saved setups,
   workspace/recent-result persistence and practical settings. These are core gaps,
   not optional polishing. Their original scope stays required; a restricted
   substitute must remain labelled partial.
4. **Run the whole-app parity pass.** Check every required main action from both
   file-first and action-first entry where relevant. Run representative Mac GUI
   workflows and packaged Windows runtime/build checks, retain originals and reopen
   actual outputs. Report unavailable manual Windows GUI checks explicitly.
5. **Deepen only after broad coverage.** Revisit the recorded metadata/color policy,
   camera formats, language/orientation quality, uncommon documents, resource/race
   hardening and performance cases. Finish final visual/copy polish, signed packages,
   install/update verification and website/download accuracy. Essential safety and
   truthful limitations apply throughout, not only in this last pass.

## Main-action coverage

| Family | Main actions to connect | Current foundation / broad gap |
| --- | --- | --- |
| Tables | CSV/TSV/flat JSON conversion | Native and shared table intake/conversion exist; batches/persistence still missing |
| Images | PNG/JPEG/TIFF/HEIC input, conversion, crop, resize, compress/fit, preview | Native common routes and shared app route exist; required tools are bundled |
| PDFs | Combine PDF/images; reorder/rotate/remove/insert; split; page images; embedded images; compress/fit | Native routes and basic shared app routes exist; page editing still missing |
| Audio | Convert/extract, MP3, size fit, exact/fast trim, track selection, waveform/playback | Native and shared conversion/trim routes exist; waveform/playback/editor still missing |
| Video | Convert/remux, resize/fit, exact/fast trim, audio selection/mute, poster/playback | Native routes and basic shared app routes exist; poster/playback/editor still missing; preserve original CFR/SDR limits |
| Text/OCR | Embedded PDF text, image/scanned-PDF recognition | Native explicit-English routes exist; explicit-English app route exists; automatic multilingual behavior remains required |
| Direct URLs | Inspect/save direct media, then convert/trim through the normal media flow | Portable backend and desktop flow are missing |
| Shared UX | Mixed batches, task search, setups, persistence, progress/retry, output actions, settings | Most remain incomplete; they are part of parity, not later feature breadth |

Keep per-family evidence distinct: native implementation, desktop route, bundled
application, Mac end-to-end test and Windows runtime/build result. A checkbox must
not mean only that an engine function exists. Use REQUIREMENTS.json for full scope
and PARITY_STATUS.md for current evidence; add the relevant app test as each route
is exposed. Do not copy a list of backend capabilities into the website as if the
application already provides them.

## Work deliberately stopped at this checkpoint

- Further PDF graph/image-object corner cases and additional compression knobs.
- Further camera-specific HEIC/gain-map/depth corpus expansion.
- Language-by-language OCR accuracy refinements before the broad OCR route exists.
- Unadvertised formats, new PDF editing breadth, media joining, AI tools or commerce.

Existing implementations and fixtures are retained. The BT.709/ImageIO color
policy difference remains recorded in HEIC_RESEARCH.md; no equivalence is claimed.
Do not suppress known failures or preservation limits to make broad coverage green.

## Completion gate

Parity is complete only when every release-and-parity main action has a usable,
verified app path with working shared UX, and the original requirements/limits are
accounted for. Release completion additionally needs actual bundled, signed downloads
and secure updates. Keep the active goal open until those outcomes are achieved.
