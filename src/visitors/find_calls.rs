use rustc_hir as hir;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter;
use rustc_middle::ty::TyCtxt;

use crate::types::ati_info::FunctionBoundaries;

pub struct FindUntrackedCallsVisitor<'tcx, 'a> {
    pub tcx: TyCtxt<'tcx>,
    pub fbs: &'a mut FunctionBoundaries,
}

impl<'tcx, 'a> Visitor<'tcx> for FindUntrackedCallsVisitor<'tcx, 'a> {
    type NestedFilter = nested_filter::All;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        match expr.kind {
            // we've found a call to a function...
            hir::ExprKind::Call(func, _args) => {
                if let hir::ExprKind::Path(ref qpath) = func.kind {
                    // ... the function is a user defined function
                    let ldid = expr.hir_id.owner.def_id;

                    let typeck = self.tcx.typeck(ldid);
                    if let Res::Def(_kind, def_id) = typeck.qpath_res(qpath, func.hir_id) {
                        // ... we have type information for it
                        if !self.fbs.is_fn_def_id_tracked(&def_id) {
                            // .. and the user defined function is untracked.

                            // this function call might need to have it's inputs
                            // untupled, and it's output tupled, depending on the type signature.
                            // store all this information in FunctionBoundaries.
                            let span = func.span;
                            let ret_ty = typeck.expr_ty_opt(expr).unwrap();
                            self.fbs.observe_untracked_fn_call(span, ret_ty);
                        }
                    }
                }
            }

            _ => {}
        }

        intravisit::walk_expr(self, expr);
    }
}
