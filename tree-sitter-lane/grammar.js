const PREC = {
    add: 1,
    multiply: 2,
    compose: 3,
    call: 4,
    unary: 5,
    product: 1,
};

module.exports = grammar({
    name: 'lane',

    extras: ($) => [
        /\s/,
        $.comment,
    ],

    word: ($) => $.identifier,

    rules: {
        source_file: ($) => repeat($._declaration),

        comment: () => token(seq('//', /.*/)),

        _declaration: ($) => choice(
            $.input_declaration,
            $.output_declaration,
            $.binding_declaration,
        ),

        input_declaration: ($) => seq(
            'in',
            ':',
            field('type', $._type),
            field('name', $.identifier),
        ),

        output_declaration: ($) => seq(
            'out',
            ':',
            field('value', $._expression),
        ),

        binding_declaration: ($) => seq(
            optional(field('modifier', $.gen_modifier)),
            field('type', $._type),
            field('name', $.identifier),
            '=',
            field('value', $._expression),
        ),

        gen_modifier: () => 'gen',

        _type: ($) => choice(
            $.product_type,
            $._non_product_type,
        ),

        _non_product_type: ($) => choice(
            $.function_type,
            $.legacy_function_type,
            $.hom_type,
            $.end_type,
            $.constraint_type,
            $.parenthesized_type,
            $.type_identifier,
        ),

        product_type: ($) => prec.left(PREC.product, seq(
            field('left', $._non_product_type),
            repeat1(seq('×', field('right', $._non_product_type))),
        )),

        parenthesized_type: ($) => seq('(', $._type, ')'),

        function_type: ($) => seq(
            'Func',
            '(',
            field('input', $._type),
            ',',
            field('output', $._type),
            ')',
        ),

        legacy_function_type: ($) => seq(
            'func',
            '(',
            field('input', $._type),
            '->',
            field('output', $._type),
            ')',
        ),

        hom_type: ($) => seq(
            'Hom',
            '(',
            field('input', $._type),
            ',',
            field('output', $._type),
            ')',
        ),

        end_type: ($) => seq(
            'End',
            '(',
            field('value', $._type),
            ')',
        ),

        constraint_type: ($) => seq(
            'C',
            '(',
            field('value', $._type),
            ')',
        ),

        type_identifier: () => token(choice(
            'Float',
            'R',
            'float',
            'Int',
            'Z',
            'int',
            'Vec2',
            'R2',
            'vec2',
            'Vec3',
            'R3',
            'vec3',
            'Vec4',
            'R4',
            'vec4',
            'Mat2',
            'mat2',
            'Mat3',
            'mat3',
            'Mat4',
            'mat4',
            'Obj3',
        )),

        _expression: ($) => choice(
            $.binary_expression,
            $.call_expression,
            $.unary_expression,
            $.parenthesized_expression,
            $.tuple_expression,
            $.identifier,
            $.number,
        ),

        call_expression: ($) => prec.left(PREC.call, seq(
            field('function', choice(
                $.call_expression,
                $.unary_expression,
                $.parenthesized_expression,
                $.tuple_expression,
                $.identifier,
                $.number,
            )),
            field('arguments', $.argument_list),
        )),

        argument_list: ($) => seq(
            '(',
            optional(choice(
                commaSep1($.named_argument),
                commaSep1($._expression),
            )),
            ')',
        ),

        named_argument: ($) => seq(
            field('name', $.identifier),
            '=',
            field('value', $._expression),
        ),

        unary_expression: ($) => prec.right(PREC.unary, seq(
            field('operator', '-'),
            field('argument', $._expression),
        )),

        binary_expression: ($) => choice(
            prec.left(PREC.add, seq(field('left', $._expression), field('operator', '+'), field('right', $._expression))),
            prec.left(PREC.add, seq(field('left', $._expression), field('operator', '-'), field('right', $._expression))),
            prec.left(PREC.multiply, seq(field('left', $._expression), field('operator', '*'), field('right', $._expression))),
            prec.left(PREC.multiply, seq(field('left', $._expression), field('operator', '/'), field('right', $._expression))),
            prec.left(PREC.compose, seq(field('left', $._expression), field('operator', '@'), field('right', $._expression))),
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

        identifier: () => /[A-Za-z_][A-Za-z0-9_]*/,

        number: () => token(choice(
            /\d+\.\d+/,
            /\d+\./,
            /\.\d+/,
            /\d+/,
        )),
    },
});

function commaSep1(rule) {
    return seq(rule, repeat(seq(',', rule)));
}
