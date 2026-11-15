# Hush agent skills

Procedures for keeping the codebase small, deep and easy to change. Claude
Code reads the same files through the symlinks in `.claude/skills`.

- `untangle-codebase`: a whole-codebase pass that removes hidden links,
  needless links and surplus. Run it when a change starts touching many files.
- `codebase-design`: the shared vocabulary (module, interface, depth, seam,
  adapter, leverage, locality) and the deletion test.
- `improve-codebase-architecture`: finds shallow modules and proposes
  deepening refactors as a visual report.

`codebase-design` and `improve-codebase-architecture` come from
[`mattpocock/skills`](https://github.com/mattpocock/skills) (MIT, see
`mattpocock-skills.LICENSE`). Domain terms live in `CONTEXT.md` at the repo
root.
