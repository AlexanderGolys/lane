const PREC = {
    compare: 0,
    add: 1,
    multiply: 2,
    compose: 4,
    power: 3,
    call: 5,
    unary: 6,
    closure: 7,
};

const CATEGORY_NAMES = [
    'Ab',
    'Mon',
    'Grp',
    'Ring',
    'DivRing',
    'RVect',
    'RAlg',
    'RDivAlg',
    'Set',
];

module.exports = grammar({
    name: 'lane',

    extras: ($) => [
        /\s/,
        $.comment,
    ],

    word: ($) => $._identifier_text,

    conflicts: ($) => [
        [$._expression, $.closure_parameter_list],
    ],

    rules: {
        source_file: ($) => seq(repeat($.directive), repeat($._declaration)),

        comment: () => token(seq('//', /.*/)),

        directive_token: () => token(/#[A-Za-z0-9_]+/),

        directive: ($) => prec.right(seq($.directive_token, optional(choice($.number, $.identifier)))),

        _declaration: ($) => choice(
            $.provided_category_declaration,
            $.category_type_declaration,
            $.product_type_declaration,
            $.input_declaration,
            $.inferred_binding_declaration,
            $.binding_declaration,
        ),

        provided_category_declaration: ($) => seq(
            'provided',
            field('category', $.category_identifier),
            commaSep1(field('name', $.identifier)),
        ),

        product_type_declaration: ($) => seq(
            optional(field('modifier', $.gen_modifier)),
            field('category', $.category_identifier),
            field('name', $.identifier),
            optional($.product_field_list),
            '=',
            field('type', $._type),
        ),

        category_identifier: () => choice(...CATEGORY_NAMES),

        product_field_list: ($) => seq(
            '<',
            commaSep1(field('name', $.identifier)),
            '>',
        ),

        category_type_declaration: ($) => prec(1, seq(
            field('category', $.category_identifier),
            field('name', $.identifier),
            '=',
            field('base', $._type),
            $.category_op_list,
        )),

        category_op_list: ($) => seq(
            '{',
            commaSep1($.category_op_binding),
            '}',
        ),

        category_op_binding: ($) => seq(
            field('operator', choice('0', '1', 'e', '+', '-', '*', 'inv', 'scale')),
            ':',
            field('name', $.identifier),
        ),

        input_declaration: ($) => choice(
            seq(
                'provided',
                field('type', $._type),
                commaSep1(field('name', $.identifier)),
            ),
            seq(
                'provided',
                commaSep1(field('name', $.identifier)),
                ':',
                field('type', $.function_arrow_type),
            ),
        ),

        binding_declaration: ($) => seq(
            optional(field('modifier', $.gen_modifier)),
            field('type', $._type),
            field('name', $.identifier),
            '=',
            field('value', $._expression),
        ),

        inferred_binding_declaration: ($) => seq(
            optional(field('modifier', $.gen_modifier)),
            field('name', $.identifier),
            '=',
            field('value', $._expression),
        ),

        gen_modifier: () => choice('construct', 'const'),

        _type: ($) => choice(
            $.product_type,
            $._non_product_type,
        ),

        _non_product_type: ($) => choice(
            $.hom_type,
            $.end_type,
            $.array_type,
            $.unit_type,
            $.generic_type,
            $.parenthesized_type,
            alias($.identifier, $.type_identifier),
        ),

        unit_type: () => '*',

        generic_type: ($) => seq(
            '{',
            field('name', $.identifier),
            '}',
        ),

        product_type: ($) => prec.left(PREC.multiply, seq(
            $._non_product_type,
            repeat1(seq(choice('×', 'x'), $._non_product_type)),
        )),

        parenthesized_type: ($) => seq('(', $._type, ')'),


        hom_type: ($) => seq(
            choice('Hom', 'Func'),
            '(',
            field('domain', $._type),
            ',',
            field('codomain', $._type),
            ')',
        ),

        function_arrow_type: ($) => seq(
            field('domain', $._type),
            '->',
            field('codomain', $._type),
        ),

        end_type: ($) => seq(
            'End',
            '(',
            field('value', $._type),
            ')',
        ),

        array_type: ($) => seq(
            'Array',
            '(',
            field('element', $._type),
            ')',
        ),

        _expression: ($) => choice(
            $.closure_expression,
            $.conditional_expression,
            $.binary_expression,
            $.call_expression,
            $.field_access_expression,
            $.index_expression,
            $.unary_expression,
            $.parenthesized_expression,
            $.tuple_expression,
            $.bracket_literal,
            $.string,
            $.identifier,
            $.number,
        ),

        closure_expression: ($) => prec.right(PREC.closure, choice(
            seq(field('parameter', $.identifier), '|->', field('body', $._expression)),
            seq(field('parameters', $.closure_parameter_list), '|->', field('body', $._expression)),
        )),

        closure_parameter_list: ($) => seq(
            '(',
            commaSep1(field('parameter', $.identifier)),
            ')',
        ),

        call_expression: ($) => prec.left(PREC.call, seq(
            field('function', choice(
                $.call_expression,
                $.field_access_expression,
                $.index_expression,
                $.unary_expression,
                $.parenthesized_expression,
                $.tuple_expression,
                $.bracket_literal,
                $.identifier,
            )),
            field('arguments', $.argument_list),
        )),

        index_expression: ($) => prec.left(PREC.call, seq(
            field('array', choice(
                $.call_expression,
                $.field_access_expression,
                $.index_expression,
                $.unary_expression,
                $.parenthesized_expression,
                $.tuple_expression,
                $.bracket_literal,
                $.identifier,
            )),
            '[',
            field('index', $._expression),
            ']',
        )),

        field_access_expression: ($) => prec.left(PREC.call, seq(
            field('object', choice(
                $.call_expression,
                $.field_access_expression,
                $.index_expression,
                $.parenthesized_expression,
                $.tuple_expression,
                $.bracket_literal,
                $.identifier,
            )),
            '.',
            field('field', $.identifier),
        )),

        argument_list: ($) => seq(
            '(',
            optional(commaSep1($.named_argument)),
            optional(commaSep1($._expression)),
            ')',
        ),

        named_argument: ($) => seq(
            field('name', $.identifier),
            '=',
            field('value', $._expression),
        ),

        conditional_expression: ($) => prec.right(seq(
            'if',
            '(',
            field('condition', $._expression),
            ')',
            field('then', $._expression),
            optional(seq('else', field('else', $._expression))),
        )),

        unary_expression: ($) => choice(
            prec.right(PREC.unary, seq(field('operator', '-'), field('argument', $._expression))),
            prec.right(PREC.unary, seq(field('operator', 'not'), field('argument', $._expression))),
        ),

        binary_expression: ($) => choice(
            prec.left(PREC.compare, seq(field('left', $._expression), field('operator', '=='), field('right', $._expression))),
            prec.left(PREC.compare, seq(field('left', $._expression), field('operator', '!='), field('right', $._expression))),
            prec.left(PREC.compare, seq(field('left', $._expression), field('operator', '<'), field('right', $._expression))),
            prec.left(PREC.compare, seq(field('left', $._expression), field('operator', '<='), field('right', $._expression))),
            prec.left(PREC.compare, seq(field('left', $._expression), field('operator', '>'), field('right', $._expression))),
            prec.left(PREC.compare, seq(field('left', $._expression), field('operator', '>='), field('right', $._expression))),
            prec.left(PREC.add, seq(field('left', $._expression), field('operator', '+'), field('right', $._expression))),
            prec.left(PREC.add, seq(field('left', $._expression), field('operator', '-'), field('right', $._expression))),
            prec.left(PREC.multiply, seq(field('left', $._expression), field('operator', '*'), field('right', $._expression))),
            prec.left(PREC.multiply, seq(field('left', $._expression), field('operator', '/'), field('right', $._expression))),
            prec.right(PREC.compose, seq(field('left', $._expression), field('operator', '@'), field('right', $._expression))),
            prec.right(PREC.power, seq(field('left', $._expression), field('operator', '^'), field('right', $._expression))),
            prec.left(PREC.multiply, seq(field('left', $._expression), field('operator', 'x'), field('right', $._expression))),
            prec.left(PREC.multiply, seq(field('left', $._expression), field('operator', '×'), field('right', $._expression))),
            prec.left(PREC.multiply, seq(field('left', $._expression), field('operator', 'and'), field('right', $._expression))),
            prec.left(PREC.add, seq(field('left', $._expression), field('operator', 'or'), field('right', $._expression))),
        ),

        parenthesized_expression: ($) => seq(
            '(',
            field('value', $._expression),
            ')',
        ),

        tuple_expression: ($) => seq(
            '(',
            field('first', $._expression),
            ',',
            field('second', $._expression),
            repeat(seq(',', field('rest', $._expression))),
            ')',
        ),

        bracket_literal: ($) => seq(
            '[',
            optional(commaSep1($._expression)),
            ']',
        ),

        identifier: ($) => prec.right(PREC.unary, seq(
            $._identifier_text,
            repeat(choice($.name_template_slot, $._identifier_suffix_text)),
        )),

        _identifier_text: () => /[A-Za-z_][A-Za-z0-9_]*/,

        _identifier_suffix_text: () => token.immediate(/[A-Za-z_][A-Za-z0-9_]*/),

        name_template_slot: ($) => seq(
            token.immediate('{'),
            optional(field('name', $.template_slot_content)),
            token.immediate('}'),
        ),

        template_slot_content: () => token.immediate(/[A-Za-z0-9_]+/),

        number: () => token(choice(
            /\d+(?:\.\d+)?[eE][+-]?\d+/,
            /\d+\.[eE][+-]?\d+/,
            /\.\d+[eE][+-]?\d+/,
            /\d+\.\d+/,
            /\d+\./,
            /\.\d+/,
            /\d+/,
        )),

        string: ($) => seq(
            '"',
            repeat(choice($.string_content, $.placeholder, /\\./)),
            '"',
        ),

        string_content: () => token.immediate(prec(1, /[^"$\\]+/)),

        placeholder: ($) => seq(
            '${',
            field('name', $.identifier),
            optional(seq('.', field('field', $.identifier))),
            '}',
        ),
    },
});

function commaSep1(rule) {
    return seq(rule, repeat(seq(',', rule)));
}
