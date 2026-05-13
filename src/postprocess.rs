use super::*;

/// Lowers typed semantic expressions into the smaller core used by emission.
pub(super) fn postprocess_typed_program(mut program: TypedProgram) -> TypedProgram {
    for func in &mut program.funcs {
        if let TypedFuncBody::Expr(expr) = &mut func.body {
            postprocess_value_expr(expr);
        }
    }
    for binding in &mut program.value_bindings {
        postprocess_value_expr(&mut binding.expr);
    }
    program
}

/// Applies a typed function expression to a typed value and lowers function-syntax
/// forms into value core syntax.
pub(super) fn apply_function_expr(func: &FunctionExpr, arg: ValueExpr) -> ValueExpr {
    match &func.kind {
        FunctionExprKind::Named(name) => ValueExpr::Call {
            func: name.clone(),
            args: vec![arg],
            ty: func.output.clone(),
        },
        FunctionExprKind::Operator(op) => {
            let (left, right) = operator_function_args(func, arg);
            ValueExpr::Binary {
                op: *op,
                left: Box::new(left),
                right: Box::new(right),
                ty: func.output.clone(),
            }
        }
        FunctionExprKind::Projection { index, field } => {
            apply_projection_function(*index, field.as_deref(), &func.input, &func.output, arg)
        }
        FunctionExprKind::Diagonal { dimension } => {
            product_value(std::iter::repeat_n(arg, *dimension).collect())
        }
        FunctionExprKind::ObjectGetter {
            object,
            getter,
            captures,
        } => ValueExpr::ObjectGetterCall {
            object: object.clone(),
            getter: *getter,
            point: Box::new(arg),
            captures: captures.clone(),
            ty: func.output.clone(),
        },
        FunctionExprKind::Compose(outer, inner) => {
            let inner_value = apply_function_expr(inner, arg);
            apply_function_expr(outer, inner_value)
        }
        FunctionExprKind::PointwiseBinary { op, left, right } => ValueExpr::Binary {
            op: *op,
            left: Box::new(apply_pointwise_call_arg(left, arg.clone())),
            right: Box::new(apply_pointwise_call_arg(right, arg)),
            ty: func.output.clone(),
        },
        FunctionExprKind::PointwiseUnary { op, arg: call_arg } => ValueExpr::Unary {
            op: *op,
            expr: Box::new(apply_pointwise_call_arg(call_arg, arg)),
            ty: func.output.clone(),
        },
        FunctionExprKind::PointwiseCall { func: name, args } => ValueExpr::Call {
            func: name.clone(),
            args: args
                .iter()
                .map(|call_arg| apply_pointwise_call_arg(call_arg, arg.clone()))
                .collect(),
            ty: func.output.clone(),
        },
        FunctionExprKind::PointwiseConditional {
            condition,
            then_branch,
            else_branch,
        } => ValueExpr::Conditional {
            condition: Box::new(apply_pointwise_call_arg(condition, arg.clone())),
            then_branch: Box::new(apply_pointwise_call_arg(then_branch, arg.clone())),
            else_branch: Box::new(apply_pointwise_call_arg(else_branch, arg)),
            ty: func.output.clone(),
        },
        FunctionExprKind::ProductSameDomain(funcs) => product_value(
            funcs
                .iter()
                .map(|func| apply_function_expr(func, arg.clone()))
                .collect(),
        ),
        FunctionExprKind::ProductTensor(left, right) => {
            let left_arg = ValueExpr::Index {
                array: Box::new(arg.clone()),
                index: Box::new(ValueExpr::Int(0)),
                ty: left.input.clone(),
            };
            let right_arg = ValueExpr::Index {
                array: Box::new(arg),
                index: Box::new(ValueExpr::Int(1)),
                ty: right.input.clone(),
            };
            product_value(vec![
                apply_function_expr(left, left_arg),
                apply_function_expr(right, right_arg),
            ])
        }
    }
}

fn apply_pointwise_call_arg(call_arg: &PointwiseCallArg, arg: ValueExpr) -> ValueExpr {
    match call_arg {
        PointwiseCallArg::Function {
            func,
            expected: expected_ty,
        } => cast_value_for_expected_type(apply_function_expr(func, arg), expected_ty),
        PointwiseCallArg::Value(value) => value.as_ref().clone(),
    }
}

fn postprocess_value_expr(expr: &mut ValueExpr) {
    match expr {
        ValueExpr::Call { args, .. } => {
            for arg in args {
                postprocess_value_expr(arg);
            }
        }
        ValueExpr::MonoidPow { exponent, base, .. } => {
            postprocess_value_expr(exponent);
            postprocess_value_expr(base);
        }
        ValueExpr::NumericWidenCast { value, .. } => {
            postprocess_value_expr(value);
        }
        ValueExpr::FieldAccess { value, field, ty } => {
            postprocess_value_expr(value);
            *expr = ValueExpr::Call {
                func: core_field_access_call_name(field),
                args: vec![(**value).clone()],
                ty: ty.clone(),
            };
        }
        ValueExpr::Conditional {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            postprocess_value_expr(condition);
            postprocess_value_expr(then_branch);
            postprocess_value_expr(else_branch);
        }
        ValueExpr::ObjectGetterCall {
            point, captures, ..
        } => {
            postprocess_value_expr(point);
            for capture in captures {
                postprocess_value_expr(capture);
            }
        }
        ValueExpr::Array { elements, .. } | ValueExpr::Product(elements) => {
            for element in elements {
                postprocess_value_expr(element);
            }
        }
        ValueExpr::Index { array, index, .. } => {
            postprocess_value_expr(array);
            postprocess_value_expr(index);
            *expr = ValueExpr::Call {
                func: "__index".to_string(),
                args: vec![(**array).clone(), (**index).clone()],
                ty: expr.ty(),
            };
        }
        ValueExpr::Concat { left, right, .. } => {
            postprocess_value_expr(left);
            postprocess_value_expr(right);
        }
        ValueExpr::Binary {
            op,
            left,
            right,
            ty,
        } => {
            postprocess_value_expr(left);
            postprocess_value_expr(right);
            if let Some(func) = core_binary_operator_call_name(*op, &left.ty(), &right.ty()) {
                *expr = ValueExpr::Call {
                    func,
                    args: vec![(**left).clone(), (**right).clone()],
                    ty: ty.clone(),
                };
            }
        }
        ValueExpr::Unary {
            op,
            expr: inner,
            ty,
        } => {
            postprocess_value_expr(inner);
            if let Some(func) = core_unary_operator_call_name(*op, &inner.ty()) {
                *expr = ValueExpr::Call {
                    func,
                    args: vec![(**inner).clone()],
                    ty: ty.clone(),
                };
            }
        }
        ValueExpr::Vec2(x, y) => {
            postprocess_value_expr(x);
            postprocess_value_expr(y);
            *expr = ValueExpr::Call {
                func: "__vec2".to_string(),
                args: vec![(**x).clone(), (**y).clone()],
                ty: Type::Vec2,
            };
        }
        ValueExpr::Vec3(x, y, z) => {
            postprocess_value_expr(x);
            postprocess_value_expr(y);
            postprocess_value_expr(z);
            *expr = ValueExpr::Call {
                func: "__vec3".to_string(),
                args: vec![(**x).clone(), (**y).clone(), (**z).clone()],
                ty: Type::Vec3,
            };
        }
        ValueExpr::Vec4(x, y, z, w) => {
            postprocess_value_expr(x);
            postprocess_value_expr(y);
            postprocess_value_expr(z);
            postprocess_value_expr(w);
            *expr = ValueExpr::Call {
                func: "__vec4".to_string(),
                args: vec![(**x).clone(), (**y).clone(), (**z).clone(), (**w).clone()],
                ty: Type::Vec4,
            };
        }
        ValueExpr::Matrix { rows, .. } => {
            for row in rows {
                postprocess_value_expr(row);
            }
        }
        ValueExpr::Bool(_)
        | ValueExpr::Float(_)
        | ValueExpr::Int(_)
        | ValueExpr::Neutral { .. }
        | ValueExpr::Var { .. }
        | ValueExpr::MatrixBasis { .. }
        | ValueExpr::UnitVectorBasis { .. } => {}
    }
}

fn core_binary_operator_call_name(op: BinOp, left: &Type, right: &Type) -> Option<String> {
    if left == right && uses_core_algebra_helpers(left) {
        return match op {
            BinOp::Add => Some("__add".to_string()),
            BinOp::Sub => Some("__sub".to_string()),
            BinOp::Mul => Some("__mult".to_string()),
            _ => None,
        };
    }

    if op == BinOp::Mul && matches!(right, Type::Float) && uses_core_algebra_helpers(left) {
        return Some("__scale".to_string());
    }

    None
}

fn core_field_access_call_name(field: &str) -> String {
    format!("__field_{field}")
}

fn core_unary_operator_call_name(op: UnaryOp, ty: &Type) -> Option<String> {
    match (op, ty) {
        (UnaryOp::Inv, ty) if uses_core_algebra_helpers(ty) => Some("__inv".to_string()),
        _ => None,
    }
}

fn uses_core_algebra_helpers(ty: &Type) -> bool {
    !matches!(
        ty,
        Type::Bool
            | Type::Int
            | Type::Float
            | Type::Vec2
            | Type::Vec3
            | Type::Vec4
            | Type::Isom2
            | Type::Isom3
            | Type::Mat(_, _)
            | Type::Array(_)
            | Type::Func(_, _)
    ) && (has_category(ty, Category::Ring)
        || has_category(ty, Category::DivRing)
        || has_category(ty, Category::RAlg)
        || has_category(ty, Category::RDivAlg)
        || has_category(ty, Category::Mon)
        || has_category(ty, Category::Grp)
        || has_category(ty, Category::Ab))
}
