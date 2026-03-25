# tree-sitter-lane

Tree-sitter grammar for the Lane DSL.

The grammar includes highlight queries in `queries/highlights.scm` for keywords, declaration names, call sites, named arguments, built-in types, and operators.

Generate the parser with:

```sh
tree-sitter generate
```

Run the corpus tests with:

```sh
tree-sitter test
```
