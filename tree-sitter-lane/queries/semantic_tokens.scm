(comment) @comment
(number) @number
(string_content) @string

(directive_token) @directive
(directive (identifier) @namespace)

["provided" "construct" "const"] @keyword

(conditional_expression
  ["if" "else"] @keyword)

(gen_modifier) @keyword

["+" "-" "*" "/" "@" "=" "==" "!=" "<" "<=" ">" ">=" "×" "x" "|->"] @operator

(generic_type
  name: (identifier) @typeParameter)

(name_template_slot
  name: (template_slot_content) @typeParameter)

(category_identifier) @type
(type_identifier) @type
(unit_type) @type

(input_declaration
  name: (identifier) @parameter.declaration)

(product_type_declaration
  name: (identifier) @type.declaration)

(product_field_list
  name: (identifier) @property)

(named_argument
  name: (identifier) @parameter)

(field_access_expression
  field: (identifier) @property)

(binding_declaration
  type: [
    (end_type)
    (function_type)
    (hom_type)
    (array_type)
  ]
  name: (identifier) @function.declaration)

(binding_declaration
  type: [
    (type_identifier)
    (array_type)
    (product_type)
    (parenthesized_type)
    (generic_type)
    (unit_type)
  ]
  name: (identifier) @variable.declaration)

(inferred_binding_declaration
  name: (identifier) @variable.declaration)

(closure_expression
  parameters: (identifier) @parameter.declaration)

(closure_parameter_list
  parameter: (identifier) @parameter.declaration)

(function_type
  "Func" @functor)

(hom_type
  "Hom" @functor)

(function_arrow_type
  "->" @functor)

[(end_type "End") (array_type "Array")] @type

(call_expression
  function: (identifier) @function)
