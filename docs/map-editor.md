# Map editor: scope and plan

Status: planning (issue #5). Nothing here is built yet; this note fixes the
scope of a first version so that it can be built in small steps.

## Why

Most devices have no map in the library. Today a map is written by hand in a
text editor from the device's manual, then opened in Modbench to find the
mistakes. Modbench already has most of the parts a built-in editor needs:

| Part | Where it is today |
|---|---|
| Parse and validate `.mbmap.json`, with the spec's codes and JSON Pointers | `mbmap::parse` (Rust), shown when a map is opened |
| Write a map in the spec's layout (one line per register) | `save_map` command, `mbmap::to_string` |
| Add a register to an open map, with checks | `CustomRegisterForm`, `custom-registers.ts` |
| Find which addresses of a device answer | Tools › Find addresses |
| See raw registers as every type and byte order | Tools › Data decoder |
| Try a map against a simulated device | Tools › Simulator |

## First version (minimum workflow)

1. **New map** (Library page): vendor, model, connection defaults (protocol,
   unit ID, byte and word order, address base). Starts with no registers.
2. **Edit registers** in a table: name, group, area, address, type, length,
   bit/bits, scale, offset, unit, access, enum, description. One row per
   register, as the spec lays files out. Add, duplicate, delete, reorder.
3. **Validate as you type**: each change is checked with the same Rust
   validator the library uses (`mbmap::parse` on the map being edited), and
   errors and warnings show on their row and field, with the spec's code.
   Overlaps (`overlap`) and duplicate names show as warnings.
4. **Save** as `.mbmap.json` (format version 2 only), refused while there are
   errors; optionally add it to the library at once.
5. **Edit an existing map**: open any map from the library or a file into the
   same editor. This is cheap once 1–4 exist, so it belongs in the first
   version; a map's `revision` is raised by one when a register changes
   (spec 3.1), and `revised` set to today.

Out of the first version: SunSpec models, `scale_factor` and `sentinels`
editing (kept as they are when a map is edited), firmware ranges per
register, importing spreadsheets.

## Helpers that make it fast (second step)

- **From Find addresses**: "New map from these ranges" creates one `uint16`
  register per address that answered, to be renamed and retyped.
- **From the Data decoder**: pick the type and byte order that make sense
  for a register, and apply them to the row.
- **Promote custom registers and computed values**: registers added to an
  open map on this computer can be written into a copy of its file.
- **Test with the simulator**: "Simulate this map" from the editor.

## Compatibility with the format

- The editor writes version 2 only, through `mbmap::to_string`, so files keep
  the spec's layout and diff well.
- Keys the editor does not show (`x_*`, `notes`, `sources`, `comment`,
  unknown keys) are kept unchanged when an existing map is saved: the editor
  changes registers and the fields it shows, nothing else.
- What the validator accepts is what the editor accepts: there is one
  implementation of the rules (Rust), shared with the library's CI through
  the fixtures in `vendor/mbmap-library/tests/`.

## Implementation outline

- Rust: a `validate_map(map) -> Vec<Diagnostic>` command that serialises the
  edited map with `mbmap::to_string` and parses it again, so diagnostics carry
  the JSON Pointers of the saved file; `save_map` as today.
- Frontend: a `MapEditor` page reached from the Library page and from an open
  map; the table reuses the register form's fields and checks; diagnostics
  are mapped from `/registers/N/field` to row N, field `field`.
- Tests: editing round trips (open, change one register, save: only that
  line changes), validation messages by pointer, refusal to save with errors.

## Priority

After the open feature issues. Steps 1–4 are one pull request; editing an
existing map (5) the next; the helpers after that.
