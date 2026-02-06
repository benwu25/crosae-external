/* Defines a visitor which tags all primitives that can be tagged,
 * based on the common::can_literal_be_tupled function. Further,
 * finds uses of these values that require them to be "untupled"
 * within tracked functions (like when passed to an untracked function),
 * unbinding the tag from the value in that case (TaggedValue<T> -> T).
*/
use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_span::{DUMMY_SP, Ident};

use crate::common;
use crate::types::ati_info::FunctionBoundaries;

pub struct TupleLiteralsVisitor<'a> {
    fbs: &'a FunctionBoundaries,
}

impl<'a> MutVisitor for TupleLiteralsVisitor<'a> {
    /// Converts all literals into TaggedValue<T>'s
    /// while making sure those values are correctly passed
    /// between the tracked/untracked boundary.
    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        mut_visit::walk_expr(self, expr);

        match expr.kind {
            // Convert all literals into TaggedValues, if necessary
            ast::ExprKind::Lit(lit) => {
                if common::can_literal_be_tupled(&lit) {
                    *expr = self.tupleify_expr(expr);
                }
            }

            // Convert all invocations of untracked functions
            // to use the un-tupled values, then bringing the return
            // back into a TaggedValue
            ast::ExprKind::Call(ref func, ref mut args) => {
                if let ast::ExprKind::Path(_, _) = &func.kind {
                    if let Some(ret_ty) = self.fbs.get_untracked_fn_call_ret_ty(&func.span) {
                        // TODO: what if a vec is supposed to store a tracked value?
                        // the Vec.push operation is an "untracked" function, but we DO
                        // want to pass in a TaggedValue to it?
                        for arg_expr in args.iter_mut() {
                            arg_expr.kind = self.unbind_tupled_expr(arg_expr);
                        }

                        // at this point, we need to make a decision about
                        // what values are moved back into our "tracking" context.
                        // references can refer to memory anywhere -> no guarantees of
                        // the returned reference to be "our code".
                        // owned values are safe to move in?
                        // TODO: use ret_ty to resolve ^
                        if common::can_type_string_be_tupled(ret_ty) {
                            *expr = self.tupleify_expr(expr);
                        }
                    }
                }
            }

            // WIP: with above TODOs regarding collections
            // need to untuple to allow us to actually index slices, vectors, etc
            // could this be done by overriding the index operator?
            // this works but I find myself squinting at it....
            ast::ExprKind::Index(_, ref mut index_expr, _) => {
                index_expr.kind = self.unbind_tupled_expr(index_expr)
            }

            // TODO: handle macro invocationss similar to Call
            ast::ExprKind::MacCall(box ast::MacCall {
                ref mut path,
                ref mut args,
            }) => {}

            // TODO: handle method calls?
            _ => {}
        }
    }
}

impl<'a> TupleLiteralsVisitor<'a> {
    pub fn new(
        // modified_funcs: &'modfuncs HashMap<String, FnInfo>,
        // call_spans: &'modfuncs HashMap<Span, String>,
        fbs: &'a FunctionBoundaries,
    ) -> Self {
        Self { fbs }
    }

    /// Takes an expression of type T and converts it to an expression of TaggedValue<T>,
    /// by using the ATI::track function from ati.rs
    fn tupleify_expr(&self, expr: &ast::Expr) -> ast::Expr {
        ast::Expr {
            id: ast::DUMMY_NODE_ID,
            kind: ast::ExprKind::Call(
                Box::new(ast::Expr {
                    id: ast::DUMMY_NODE_ID,
                    kind: ast::ExprKind::Path(
                        None,
                        ast::Path {
                            span: DUMMY_SP,
                            segments: [
                                ast::PathSegment {
                                    ident: Ident::from_str("ATI"),
                                    id: ast::DUMMY_NODE_ID,
                                    args: None,
                                },
                                ast::PathSegment {
                                    ident: Ident::from_str("track"),
                                    id: ast::DUMMY_NODE_ID,
                                    args: None,
                                },
                            ]
                            .into(),
                            tokens: None,
                        },
                    ),
                    span: DUMMY_SP,
                    attrs: [].into(),
                    tokens: None,
                }),
                [Box::new(expr.clone())].into(),
            ),
            span: DUMMY_SP,
            attrs: [].into(),
            tokens: None,
        }
    }

    /// Takes a TaggedValue<T> expression and unwraps it to just T,
    /// by accessing the TaggedValue's 0'th field.
    fn unbind_tupled_expr(&self, expr: &mut ast::Expr) -> ast::ExprKind {
        ast::ExprKind::Field(Box::new(expr.clone()), Ident::from_str("0"))
    }
}
