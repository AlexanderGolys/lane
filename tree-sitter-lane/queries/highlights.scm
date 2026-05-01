["provided" "generate" "gen" "construct" "const" "Func" "Hom" "End"] @keyword

(gen_modifier) @keyword.modifier

["+" "-" "*" "/" "@" "=" "×"] @operator

["(" ")"] @punctuation.bracket
[","] @punctuation.delimiter

(comment) @comment
(number) @number

(type_identifier) @type
(type_identifier) @type.builtin

(input_declaration
  name: (identifier) @variable.parameter)

(named_argument
  name: (identifier) @property)

(binding_declaration
  type: [
    (end_type)
    (function_type)
    (hom_type)
  ]
  name: (identifier) @function)

(binding_declaration
  type: [
    (type_identifier)
    (product_type)
    (parenthesized_type)
  ]
  name: (identifier) @variable)

(call_expression
  function: (identifier) @constructor
  (#match? @constructor "^[A-Z]"))

(call_expression
  function: (identifier) @function.call)
