use std::collections::{HashMap, HashSet};

use rustc_middle::ty::TyCtxt;
use rustc_hir as hir;
use rustc_hir::intravisit::{self, Visitor};
use rustc_middle::hir::nested_filter;
use rustc_hir::def_id::DefId;
use rustc_hir::def::Res;
use rustc_middle::ty::Ty;
use rustc_span::Span;

pub struct FindCallsVisitor<'tcx> {
    pub tcx: TyCtxt<'tcx>,
    pub defs: HashSet<DefId>,
    pub call_spans: HashMap<Span, String>,
}

impl<'tcx> Visitor<'tcx> for FindCallsVisitor<'tcx> {
    type NestedFilter = nested_filter::All;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.tcx
    }

    fn visit_expr(&mut self, expr: &'tcx hir::Expr<'tcx>) {
        match expr.kind {
            hir::ExprKind::Call(func, _args) => {
                if let hir::ExprKind::Path(ref qpath) = func.kind {
                    let hir_id = func.hir_id;

                    let typeck = self.tcx.typeck(expr.hir_id.owner.def_id);
                    if let Res::Def(kind, id) = typeck.qpath_res(qpath, hir_id) {
                        if !self.defs.contains(&id) {

                            let span = func.span;
                            let call_type = typeck.expr_ty_opt(expr).unwrap();

                            self.call_spans.insert(span, call_type.to_string());

                            // for (phir_id, node) in self.tcx.hir_parent_iter(hir_id) {
                            //     println!("{:?} -> {:#?}", phir_id, node);
                            // }

                        }
                    }
                }
            }
            
            _ => {}
        }

        intravisit::walk_expr(self, expr);
    }
}
