# tree-sitter-lane

Tree-sitter grammar for the Lane DSL.

## Syntax Notes

- Product type field names use angle brackets: `Set Hit = R3 x R <point, distance>`.
- `Array(...)` constructs arrays. Square brackets are reserved for vector and matrix literals.
- Generic placeholder suffixes are part of identifiers, for example `R{n}`, `I{3}`, `eye{3}`, and `e{3}{2}`.

## Neovim

Add this directory as a Neovim plugin so the `.lane` filetype, highlight query,
and nvim-treesitter parser install metadata are registered before parser
installation:

```lua
{
    dir = "/home/flux/sdf-compiler/tree-sitter-lane",
    name = "tree-sitter-lane",
    dependencies = { "nvim-treesitter/nvim-treesitter" },
    config = function()
        require("lane").setup()
    end,
}
```

Then install the parser with nvim-treesitter:

```lua
require("nvim-treesitter").install({ "lane" }):wait()
```

From command mode, `:TSInstall lane` works too after the plugin has loaded.

If you do not use nvim-treesitter, compile the parser and register it directly:

```sh
cc -fPIC -shared -I src src/parser.c -o parser.so
```

```lua
require("lane").setup({
    parser_path = "/home/flux/sdf-compiler/tree-sitter-lane/parser.so",
})
```

By default, `require("lane").setup()` uses `parser.so` from this directory only
when it is at least as new as `src/parser.c`; otherwise it lets
nvim-treesitter's installed parser handle `lane` buffers.

Start highlighting for Lane buffers with Neovim's current tree-sitter API:

```lua
vim.api.nvim_create_autocmd("FileType", {
    pattern = "lane",
    callback = function()
        vim.treesitter.start()
    end,
})
```

The grammar includes highlight queries for both the tree-sitter CLI and Neovim:

- `queries/highlights.scm` is used by the tree-sitter CLI.
- `queries/lane/highlights.scm` is used by Neovim's runtime query loader.

Neovim captures Lane conditionals with `@keyword.conditional`, so `if` and
`else` follow the active colorscheme's conditional keyword style.

After changing grammar, parser, or query files in a running Neovim session, run:

```vim
:LaneTSReload
```

If `parser.so` is older than `src/parser.c`, rebuild it first:

```sh
cc -fPIC -shared -I src src/parser.c -o parser.so
```

Generate the parser with:

```sh
npm run generate
```

Commit the regenerated `src/parser.c` and `src/grammar.json` artifacts whenever `grammar.js` changes so Neovim and other tree-sitter consumers pick up the new syntax.

Run the corpus tests with:

```sh
npm test
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
