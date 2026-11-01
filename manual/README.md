# Modbench user manual

The user manual, in LaTeX, with the app's look: its colours, the Inter font,
cards, buttons and tabs drawn as in the app.

## Build

Needs a TeX distribution with LuaLaTeX (MiKTeX or TeX Live) and the `inter`
font package.

```sh
cd manual
latexmk modbench-manual.tex
```

The PDF is written to `build/modbench-manual.pdf` and copied to
`modbench-manual.pdf`, the copy kept in the repository (the app's Settings
link to it).

The cover's version is read from `../package.json` by `version.lua`, and its date
is the month of the build, so a release only needs the manual rebuilt.

## Layout

- `modbench-manual.tex`: the book: cover, contents, chapters.
- `modbench.sty`: the style (colours, type, headings, boxes, interface
  elements, screenshot placeholders).
- `chapters/`: one file per chapter.
- `screenshots/`: the pictures (see its README).

## Writing

| Macro | Shows |
|---|---|
| `\ui{Stop}` / `\uiaccent{Start}` | A button / the main button |
| `\tab{Chart}` | A tab of a segmented control |
| `\nav{Tools\then Simulator}` | A page or menu path |
| `\field{Unit ID}` | A field label |
| `\key{Esc}` | A key |
| `\val{0x0405}`, `\file{.mbsim}` | Values, addresses, file names |
| `\sent{…}` / `\received{…}` | Bytes sent (blue) / received (green) |
| `note`, `tip`, `warning` | Boxes |
| `steps` | Numbered steps |
| `\screenshot[height]{name}{caption}{what to capture}` | `screenshots/name.png`, or a placeholder until it exists |
