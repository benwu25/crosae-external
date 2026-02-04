/* Defines a visitor which tags all primitives that can be tagged,
 * based on the common::can_literal_be_tupled function. Further,
 * finds uses of these values that require them to be "untupled" 
 * within tracked functions (like when passed to an untracked function),
 * unbinding the tag from the value in that case (TaggedValue<T> -> T).
*/
use std::collections::HashMap;

use rustc_ast as ast;
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_span::{DUMMY_SP, Ident};
use rustc_span::Span;

use crate::instrumentation::common::{self, FnInfo};

pub struct TupleLiteralsVisitor<'modfuncs> {
    modified_funcs: &'modfuncs HashMap<String, FnInfo>,
    call_spans: &'modfuncs HashMap<Span, String>,
}

impl<'modfuncs> MutVisitor for TupleLiteralsVisitor<'modfuncs> {
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
                if let ast::ExprKind::Path(None, path) = &func.kind {
                    // TODO: not sure if this works with complex function invocations
                    // that involve use statements and renames. might have to construct
                    // down paths from crate::. Temporary workaround below,
                    // probably need to change it later.

                    // TODO: Another problem, we have no way of knowing what type of 
                    // value an untracked function will return. If it returns a basic type,
                    // then it can be tupled as normal, but what if it returns a complex
                    // type? a struct? a vec?
                    // Further, what if a vec is supposed to store a tracked value?
                    // the Vec.push operation is an "untracked" function, but we DO 
                    // want to pass in a TaggedValue to it?
                    if let Some(last_segment) = path.segments.last() {
                        if !self
                            .modified_funcs
                            .contains_key(last_segment.ident.as_str())
                        {
                            for arg_expr in args.iter_mut() {
                                arg_expr.kind = self.unbind_tupled_expr(arg_expr);
                            }

                            if !self.call_spans.contains_key(&func.span) {
                                *expr = self.tupleify_expr(expr);
                            }
                        }
                    }
                }
            }

            // WIP: with above TODOs regarding collections
            // need to untuple to allow us to actually index slices, vectors, etc
            // could this be done by overriding the index operator?
            ast::ExprKind::Index(_, ref mut index_expr, _) => {
                index_expr.kind = self.unbind_tupled_expr(index_expr)
            }

            // TODO: handle macro invocationss similar to Call
            ast::ExprKind::MacCall(box ast::MacCall {
                ref mut path,
                ref mut args,
            }) => {
            }

            // TODO: handle method calls?
            _ => {}
        }
    }
}

impl<'modfuncs> TupleLiteralsVisitor<'modfuncs> {
    pub fn new(modified_funcs: &'modfuncs HashMap<String, FnInfo>, call_spans: &'modfuncs HashMap<Span, String>) -> Self {
        Self { modified_funcs, call_spans }
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
        ast::ExprKind::Field(
            Box::new(expr.clone()),
            Ident::from_str("0"),
        )
    }
}
