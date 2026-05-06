["provided" "construct" "const" "Func" "Hom" "End" "Array"] @keyword

(conditional_expression
  ["if" "else"] @keyword.conditional)

(gen_modifier) @keyword

["+" "-" "*" "/" "@" "=" "==" "!=" "<" "<=" ">" ">=" "×" "x" "->"] @operator

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

((type_identifier) @constant.builtin
  (#any-of? @constant.builtin "Ab" "Mon" "Grp" "Ring" "DivRing" "VectR" "RAlg" "Set")
  (#set! priority 110))

(type_identifier) @type
(type_identifier) @type.builtin

(input_declaration
  name: (identifier) @variable.parameter)

(product_type_declaration
  category: (category_type (type_identifier) @constant.builtin)
  name: (identifier) @type)

(product_field_list
  name: (identifier) @property)

(product_field_list
  ["<" ">"] @punctuation.bracket)

(bracket_literal
  ["[" "]"] @punctuation.bracket)

(named_argument
  name: (identifier) @property)

(field_access_expression
  field: (identifier) @property)

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
  parameters: (identifier) @variable.parameter)

(closure_parameter_list
  parameter: (identifier) @variable.parameter)

(call_expression
  function: (identifier) @constructor
  (#match? @constructor "^[A-Z]"))

(call_expression
  function: (identifier) @function.builtin
  (#any-of? @function.builtin "size" "concat"))

(call_expression
  function: (identifier) @function)
