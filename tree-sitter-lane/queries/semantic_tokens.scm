(comment) @comment
(number) @number
(string) @string

(directive_token) @directive

(directive
  (identifier) @namespace)

["provided" "construct" "const"] @keyword

(conditional_expression
  ["if" "else"] @keyword)

(gen_modifier) @keyword

["+" "-" "*" "/" "@" "^" "=" "==" "!=" "<" "<=" ">" ">=" "×" "x" "|->" "and" "or" "not"] @operator

(generic_type
  name: (identifier) @typeParameter)


(category_identifier) @category

(type_identifier) @type

(provided_category_declaration
  name: (identifier) @type)

(provided_category_declaration
  name: (identifier) @type)

(unit_type) @type

(input_declaration
  name: (identifier) @variable.declaration)

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
    (hom_type)
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
  parameter: (identifier) @parameter.declaration)

(hom_type
  ["Hom" "Func"] @functor)

(arrow_function_declaration
  "->" @functor)

[(end_type "End") (array_type "Array")] @functor

(call_expression
  function: (identifier) @function)
