["provided" "construct" "const"] @keyword

["Func" "Hom" "End" "Array"] @lsp.type.operator  

(conditional_expression
  ["if" "else"] @keyword.conditional)

(gen_modifier) @keyword

["+" "-" "*" "/" "@" "=" "==" "!=" "<" "<=" ">" ">=" "×" "x" "|->"] @operator

["(" ")" "[" "]"] @punctuation.bracket
["," "."] @punctuation.delimiter

(comment) @comment
(directive) @keyword.directive
(number) @number
(string_content) @string
(placeholder ["${" "}"] @punctuation.special)
(placeholder name: (identifier) @variable)
(placeholder field: (identifier) @property)
(unit_type) @type.builtin

(generic_type
  ["{" "}"] @punctuation.bracket
  name: (identifier) @type)

(name_template_slot
  ["{" "}"] @punctuation.bracket
  name: (template_slot_content) @lsp.type.parameter)

(category_identifier) @constant.builtin

(type_identifier) @type

(input_declaration
  name: (identifier) @lsp.type.parameter)

(product_type_declaration
  category: (category_type (category_identifier) @constant.builtin)
  name: (identifier) @type)

(product_field_list
  name: (identifier) @property)

(product_field_list
  ["<" ">"] @punctuation.bracket)

(bracket_literal
  ["[" "]"] @punctuation.bracket)

(named_argument
  name: (identifier) @lsp.type.parameter)

(field_access_expression
  field: (identifier) @lsp.type.enumMember)

(binding_declaration
  type: [
    (end_type)
    (function_type)
    (hom_type)
    (array_type)
  ]
  name: (identifier) @function)

(binding_declaration
  type: [
    (type_identifier)
    (array_type)
    (product_type)
    (parenthesized_type)
  ]
  name: (identifier) @variable)

(inferred_binding_declaration
  name: (identifier) @variable)

(closure_expression
  parameters: (identifier) @lsp.type.parameter)

(closure_parameter_list
  parameter: (identifier) @lsp.type.parameter)

(call_expression
  function: (identifier) @constructor
  (#match? @constructor "^[A-Z]"))

(call_expression
  function: (identifier) @function.builtin
  (#any-of? @function.builtin "size" "concat"))

(call_expression
  function: (identifier) @function)
