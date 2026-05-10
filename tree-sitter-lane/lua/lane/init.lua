local M = {}

local function notify(message, level)
  vim.schedule(function()
    vim.notify(message, level or vim.log.levels.INFO, { title = "tree-sitter-lane" })
  end)
end

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
    query = remove_line(query, '["+" "-" "*" "/" "@" "=" "==" "!=" "<" "<=" ">" ">=" "×" "x" "|->"] @operator')
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

  if not has_symbol(symbols, "bracket_literal") then
    query = query:gsub('\n%(bracket_literal\n  %["%[" "%]"%] @punctuation%.bracket%)\n', "\n")
  end

  if not has_symbol(symbols, "generic_type") then
    query = query:gsub(
      "\n?" .. vim.pesc('(generic_type\n  ["{" "}"] @punctuation.bracket\n  name: (identifier) @type)') .. "\n?",
      "\n"
    )
  end

  if not has_symbol(symbols, "name_template_slot") then
    query = query:gsub(
      "\n?" .. vim.pesc('(name_template_slot\n  ["{" "}"] @punctuation.bracket\n  name: (template_slot_content) @variable.parameter)') .. "\n?",
      "\n"
    )
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
    return true
  end

  if parser_path and vim.fn.filereadable(parser_path) == 1 and not parser_path_is_explicit then
    notify(
      "parser.so is older than src/parser.c; rebuild it with `cc -fPIC -shared -I src src/parser.c -o parser.so`",
      vim.log.levels.WARN
    )
  end

  return false
end

local function start_highlighting(bufnr)
  if vim.bo[bufnr].filetype ~= "lane" then
    return
  end

  pcall(vim.treesitter.start, bufnr, "lane")
end

local function restart_highlighting()
  for _, bufnr in ipairs(vim.api.nvim_list_bufs()) do
    if vim.api.nvim_buf_is_loaded(bufnr) and vim.bo[bufnr].filetype == "lane" then
      pcall(vim.treesitter.stop, bufnr, "lane")
      start_highlighting(bufnr)
    end
  end
end

local function project_root(root)
  if not root then
    return nil
  end
  return root:gsub("/tree%-sitter%-lane$", "")
end

local function register_lsp(root, opts)
  if opts.lsp == false or not vim.lsp then
    return
  end

  local lsp_opts = type(opts.lsp) == "table" and opts.lsp or {}
  local cwd = lsp_opts.cwd or project_root(root)
  local cmd = lsp_opts.cmd
  if not cmd and cwd then
    cmd = { "cargo", "run", "--manifest-path", cwd .. "/Cargo.toml", "-p", "lane-lsp" }
  elseif not cmd then
    cmd = { "lane-lsp" }
  end
  local config = {
    cmd = cmd,
    filetypes = { "lane" },
    root_dir = cwd,
    root_markers = { "Cargo.toml", ".git" },
  }

  if vim.lsp.config and vim.lsp.enable then
    vim.lsp.config("lane_lsp", vim.tbl_extend("force", config, lsp_opts.config or {}))
    vim.lsp.enable("lane_lsp")
    return
  end

  vim.api.nvim_create_autocmd("FileType", {
    group = vim.api.nvim_create_augroup("lane_lsp", { clear = true }),
    pattern = "lane",
    callback = function(event)
      local start_config = vim.tbl_extend("force", config, lsp_opts.config or {})
      local markers = vim.fs.find({ "Cargo.toml", ".git" }, {
        upward = true,
        path = vim.api.nvim_buf_get_name(event.buf),
      })
      start_config.name = start_config.name or "lane_lsp"
      start_config.root_dir = start_config.root_dir or (markers[1] and vim.fs.dirname(markers[1]))
      start_config.bufnr = event.buf
      vim.lsp.start(start_config)
    end,
  })
end

local function load_highlight_query(root)
  if not root then
    return false
  end

  local query = read_file(root .. "/queries/lane/highlights.scm")
  if not query then
    return false
  end

  local ok, err = pcall(function()
    vim.treesitter.query.set("lane", "highlights", query_for_parser(query, parser_symbols("lane")))
  end)
  if not ok then
    notify("failed to load highlight query: " .. err, vim.log.levels.ERROR)
    return false
  end

  return true
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
  load_highlight_query(root)

  if root then
    register_nvim_treesitter_parser(root)
    vim.api.nvim_create_autocmd("User", {
      group = vim.api.nvim_create_augroup("lane_nvim_treesitter", { clear = true }),
      pattern = "TSUpdate",
      callback = function()
        register_nvim_treesitter_parser(root)
      end,
    })

    vim.api.nvim_create_user_command("LaneTSReload", function()
      register_language(root, opts)
      load_highlight_query(root)
      restart_highlighting()
    end, {
      desc = "Reload Lane Tree-sitter parser and highlight query",
    })
  end

  register_lsp(root, opts)
end

return M
