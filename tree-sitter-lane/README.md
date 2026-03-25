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

To use the parser and the new `lane-lsp` binary together in Neovim, register the filetype and built-in LSP config:

```lua
vim.filetype.add({ extension = { lane = "lane" } })

vim.lsp.config("lane_lsp", {
    cmd = { "cargo", "run", "--bin", "lane-lsp" },
    filetypes = { "lane" },
    root_markers = { "Cargo.toml", ".git" },
})

vim.lsp.enable("lane_lsp")
```
