# TODO
- [ ] Editor
  - [ ] Fonts (including bold font)
  - [ ] Full CommonMark support
  - [ ] Insert spaces on tab
  - [x] Scrolling

- [ ] Google Drive integration
    - [ ] Allow opening public docs without auth
    - [ ] Proper error when trying to open unsupported file format
    - [ ] Support for opening google docs (with convert)
    - [ ] Side panel with your google drive
    - [ ] Autosave
    - [ ] Proper "Save to" dialog
      - [ ] At least support path/with/slashes.md
      - [ ] Windows like save window?
    - [ ] Concurrent editing
      - [ ] Warn about it
      - [ ] Try three-way merge. On conflict generate git-like markers
        - [ ] In conflict-resolving mode the save button should be replaced with "Resolve and Save"

- [ ] Internal
  - [ ] Small size build
  - [ ] Check CPU usage
  - [ ] `layouter` memoization
  - [ ] Code blocks performance

- [ ] Bugs
  - [ ] Scroll
    - Make big scrollable text
    - Place caret at the end
    - Scroll up
    - Click top panel
    - Observe flickering / canvas begin shifted
  - [x] Big spaces before heading