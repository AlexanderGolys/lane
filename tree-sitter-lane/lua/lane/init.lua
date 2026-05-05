local M = {}

local function plugin_root()
  local source = debug.getinfo(1, "S").source
  if source:sub(1, 1) == "@" then
    source = source:sub(2)
  end
  return source:match("(.+)/lua/lane/init%.lua$")
end

local function read_file(path)
  local file = io.open(path, "r")
  if not file then
    return nil
  end
  local content = file:read("*a")
  file:close()
  return content
end

local function parser_is_current(root, parser_path)
  local parser_time = vim.fn.getftime(parser_path)
  if parser_time < 0 then
    return false
  end

  local generated_parser = root .. "/src/parser.c"
  local generated_time = vim.fn.getftime(generated_parser)
  return generated_time < 0 or parser_time >= generated_time
end

local function parser_symbols(lang)
  local ok, info = pcall(vim.treesitter.language.inspect, lang)
  if not ok or not info then
    return nil
  end
  return info.symbols
end

local function has_symbol(symbols, name)
  return symbols and symbols[name] ~= nil
end

local function remove_line(query, line)
  return query:gsub(vim.pesc(line) .. "\n?", "")
end

local function query_for_parser(query, symbols)
  if not symbols then
    return query
  end

  if not has_symbol(symbols, "conditional_expression") or not has_symbol(symbols, '"if"') then
    query = query:gsub(
      '\n?%(conditional_expression\n  %[%"if%" %"else%"%] @keyword%.conditional\n%)\n?',
      "\n"
    )
  end

  if not has_symbol(symbols, '"=="') then
    query = remove_line(query, '["+" "-" "*" "/" "@" "=" "==" "!=" "<" "<=" ">" ">=" "×" "x"] @operator')
    query = query:gsub(
      '(%(gen_modifier%) @keyword\n)',
      '%1\n(binary_expression operator: _ @operator)\n(unary_expression operator: _ @operator)\n'
    )
  end

  if not has_symbol(symbols, '"."') then
    query = query:gsub('%["," "%."%] @punctuation%.delimiter', '[","] @punctuation.delimiter')
  end

  if not has_symbol(symbols, "field_access_expression") then
    query = query:gsub('\n%(field_access_expression\n  field: %(identifier%) @property%)\n', "\n")
  end

  return query
end

local function register_nvim_treesitter_parser(root)
  local ok, parsers = pcall(require, "nvim-treesitter.parsers")
  if not ok or not root then
    return
  end

  parsers.lane = {
    install_info = {
      path = root,
      queries = "queries/lane",
    },
    tier = 3,
  }
end

local function register_language(root, opts)
  local parser_path = opts.parser_path
  local parser_path_is_explicit = parser_path ~= nil
  if not parser_path and root then
    parser_path = root .. "/parser.so"
  end

  if parser_path
      and vim.fn.filereadable(parser_path) == 1
      and (parser_path_is_explicit or parser_is_current(root, parser_path)) then
    vim.treesitter.language.add("lane", { path = parser_path })
  end
end

local function start_highlighting(bufnr)
  if vim.bo[bufnr].filetype ~= "lane" then
    return
  end

  pcall(vim.treesitter.start, bufnr, "lane")
end

function M.setup(opts)
  opts = opts or {}

  vim.filetype.add({
    extension = {
      lane = "lane",
    },
  })
  vim.api.nvim_create_autocmd({ "BufRead", "BufNewFile" }, {
    group = vim.api.nvim_create_augroup("lane_filetype", { clear = true }),
    pattern = "*.lane",
    callback = function(event)
      vim.bo[event.buf].filetype = "lane"
      start_highlighting(event.buf)
    end,
  })

  vim.api.nvim_create_autocmd("FileType", {
    group = vim.api.nvim_create_augroup("lane_treesitter_highlight", { clear = true }),
    pattern = "lane",
    callback = function(event)
      start_highlighting(event.buf)
    end,
  })

  local root = opts.root or plugin_root()
  register_language(root, opts)

  if root then
    local query = read_file(root .. "/queries/lane/highlights.scm")
    if query then
      vim.treesitter.query.set("lane", "highlights", query_for_parser(query, parser_symbols("lane")))
    end
  end

  if root then
    register_nvim_treesitter_parser(root)
    vim.api.nvim_create_autocmd("User", {
      group = vim.api.nvim_create_augroup("lane_nvim_treesitter", { clear = true }),
      pattern = "TSUpdate",
      callback = function()
        register_nvim_treesitter_parser(root)
      end,
    })
  end
end

return M
