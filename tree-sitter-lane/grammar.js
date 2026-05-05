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

        directive: () => token(seq('#', /.*/)),

        _declaration: ($) => choice(
            $.directive,
            $.provided_category_declaration,
            $.product_type_declaration,
            $.input_declaration,
            $.output_declaration,
            $.inferred_binding_declaration,
            $.binding_declaration,
        ),

        provided_category_declaration: ($) => seq(
            'provided',
            field('category', alias(choice(
                'Ab',
                'Mon',
                'Grp',
                'Ring',
                'Field',
                'VectR',
                'RAlg',
                'Set',
            ), $.type_identifier)),
            field('name', $.identifier),
        ),

        product_type_declaration: ($) => seq(
            optional(field('modifier', $.gen_modifier)),
            field('category', $.category_type),
            field('name', $.identifier),
            '=',
            field('type', $.product_type),
            optional($.product_field_list),
        ),

        category_type: ($) => alias(choice(
            'Ab',
            'Mon',
            'Grp',
            'Ring',
            'Field',
            'VectR',
            'RAlg',
            'Set',
        ), $.type_identifier),

        product_field_list: ($) => seq(
            '{',
            commaSep1(field('name', $.identifier)),
            optional(','),
            '}',
        ),

        input_declaration: ($) => seq(
            'provided',
            field('type', $._type),
            field('name', $.identifier),
        ),

        output_declaration: ($) => seq(
            choice('generate', 'gen'),
            field('value', $._expression),
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
            $.function_type,
            $.hom_type,
            $.end_type,
            $.array_type,
            $.parenthesized_type,
            alias($.identifier, $.type_identifier),
        ),

        product_type: ($) => prec.left(PREC.product, seq(
            field('left', $._non_product_type),
            repeat1(seq(choice('×', 'x'), field('right', $._non_product_type))),
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

        array_type: ($) => seq(
            'Array',
            '(',
            field('element', $._type),
            ')',
        ),

        _expression: ($) => choice(
            $.binary_expression,
            $.call_expression,
            $.index_expression,
            $.unary_expression,
            $.parenthesized_expression,
            $.tuple_expression,
            $.array_expression,
            $.identifier,
            $.number,
        ),

        call_expression: ($) => prec.left(PREC.call, seq(
            field('function', choice(
                $.call_expression,
                $.index_expression,
                $.unary_expression,
                $.parenthesized_expression,
                $.tuple_expression,
                $.array_expression,
                $.identifier,
                $.number,
            )),
            field('arguments', $.argument_list),
        )),

        index_expression: ($) => prec.left(PREC.call, seq(
            field('array', choice(
                $.call_expression,
                $.index_expression,
                $.unary_expression,
                $.parenthesized_expression,
                $.tuple_expression,
                $.array_expression,
                $.identifier,
                $.number,
            )),
            '[',
            field('index', $._expression),
            ']',
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

        array_expression: ($) => seq(
            '[',
            optional(commaSep1($._expression)),
            ']',
        ),

        identifier: () => /[A-Za-z_][A-Za-z0-9_]*/,

        number: () => token(choice(
            /\d+(?:\.\d+)?[eE][+-]?\d+/,
            /\d+\.[eE][+-]?\d+/,
            /\.\d+[eE][+-]?\d+/,
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
