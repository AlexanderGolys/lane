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
  if root then
    local query = read_file(root .. "/queries/lane/highlights.scm")
    if query then
      vim.treesitter.query.set("lane", "highlights", query)
    end
  end

  local parser_path = opts.parser_path
  if not parser_path and root then
    parser_path = root .. "/parser.so"
  end

  if parser_path and vim.fn.filereadable(parser_path) == 1 then
    vim.treesitter.language.add("lane", { path = parser_path })
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
