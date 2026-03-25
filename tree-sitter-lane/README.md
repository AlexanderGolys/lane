# tree-sitter-lane

Tree-sitter grammar for the Lane DSL.

The grammar includes highlight queries for both the tree-sitter CLI and Neovim:

- `queries/highlights.scm` is used by the tree-sitter CLI.
- `queries/lane/highlights.scm` is used by Neovim's runtime query loader.

Generate the parser with:

```sh
tree-sitter generate
```

Run the corpus tests with:

```sh
tree-sitter test
```
