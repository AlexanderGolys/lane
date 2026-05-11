["provided" "construct" "const"] @keyword

["Func" "Hom" "End" "Array" "->"] @lsp.type.operator

["if" "else"] @keyword.conditional

["+" "-" "*" "/" "@" "^" "=" "==" "!=" "<" "<=" ">" ">=" "×" "x" "|->"
 "and" "or" "not"] @operator

["(" ")" "[" "]" "{" "}" "<" ">"] @punctuation.bracket

["," "." ":"] @punctuation.delimiter

(comment) @comment

(directive) @keyword.directive

(number) @number

(raw_code
    [(string) "\""] @string)

(placeholder 
  ["${" "}"] @punctuation.special)

(placeholder name: (identifier) @variable.parameter)

(placeholder field: (identifier) @property)

(unit_type) @type.builtin

(generic_type
  name: (identifier) @type)

(category_identifier) @constant.builtin

(type_identifier) @type

(input_declaration
  name: (identifier) @variable)

(product_type_declaration
  category: (category_identifier) @constant.builtin
  name: (identifier) @type)

(product_field_list
  name: (identifier) @property)

(named_argument
  name: (identifier) @lsp.type.parameter)

(field_access_expression
  field: (identifier) @lsp.type.enumMember)

(binding_declaration
  type: [
    (end_type)
    (hom_type)
  ]
  name: (identifier) @function)

(binding_declaration
  type: [
    (type_identifier)
    (array_type)
    (product_type)
    (parenthesized_type)
    (generic_type)
    (unit_type)
  ]
  name: (identifier) @variable)

(inferred_binding_declaration
  name: (identifier) @variable)

(closure_expression
  parameter: (identifier) @lsp.type.parameter)

(call_expression
  function: (identifier) @function)

(arrow_function_declaration
  dom: (_) @type)

(arrow_function_declaration
  codom: (_) @type)

(arrow_function_declaration
  name: (identifier) @function)


(input_declaration
  type: (hom_type)
  name: (identifier) @function)

(call_expression
  function: (identifier) @constructor
  (#match? @constructor "^[A-Z]"))
