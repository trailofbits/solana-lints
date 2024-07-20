use std::collections::HashMap;

use rustc_hir::{
    def_id::DefId,
    intravisit::{walk_expr, Visitor},
    BinOpKind, Body, Expr, ExprKind, Mutability,
};
use rustc_lint::LateContext;
use rustc_middle::ty::TyKind as MiddleTyKind;

use crate::ANCHOR_ACCOUNT_GENERIC_ARG_COUNT;
use clippy_utils::{ty::match_type, SpanlessEq};
use if_chain::if_chain;
use solana_lints::paths;

/// Stores the accounts and if-statements (constraints) found in a function body.
pub struct Values<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    /// Lists of account expressions, partitioned by the Account type T
    pub accounts: HashMap<DefId, Vec<&'tcx Expr<'tcx>>>,
    /// List of tuples, where (x, y), where x is the left operand of the if statement and y is the right
    pub if_statements: Vec<(&'tcx Expr<'tcx>, &'tcx Expr<'tcx>)>,
}

impl<'cx, 'tcx> Values<'cx, 'tcx> {
    pub fn new(cx: &'cx LateContext<'tcx>) -> Self {
        Values {
            cx,
            accounts: HashMap::new(),
            if_statements: Vec::new(),
        }
    }

    pub fn get_referenced_accounts_and_if_statements(&mut self, body: &'tcx Body<'tcx>) -> &Self {
        self.visit_expr(&body.value);
        self
    }

    /// Checks if there is a valid key constraint for `first_account` and `second_account`.
    /// TODO: if == relation used, should return some error in the THEN block
    pub fn check_key_constraint(
        &self,
        first_account: &Expr<'_>,
        second_account: &Expr<'_>,
    ) -> bool {
        for (left, right) in &self.if_statements {
            if_chain! {
                if let ExprKind::MethodCall(path_seg_left, exprs_left, _span) = left.kind;
                if let ExprKind::MethodCall(path_seg_right, exprs_right, _span) = right.kind;
                if path_seg_left.ident.name.as_str() == "key"
                    && path_seg_right.ident.name.as_str() == "key";
                if !exprs_left.is_empty() && !exprs_right.is_empty();
                let mut spanless_eq = SpanlessEq::new(self.cx);
                if (spanless_eq.eq_expr(&exprs_left[0], first_account)
                    && spanless_eq.eq_expr(&exprs_right[0], second_account))
                    || (spanless_eq.eq_expr(&exprs_left[0], second_account)
                        && spanless_eq.eq_expr(&exprs_right[0], first_account));
                then {
                    return true;
                }
            }
        }
        false
    }
}

impl<'cx, 'tcx> Visitor<'tcx> for Values<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            // get mutable reference expressions
            if let ExprKind::AddrOf(_, mutability, mut_expr) = expr.kind;
            if let Mutability::Mut = mutability;
            // check type of expr == Account<'info, T>
            let middle_ty = self.cx.typeck_results().expr_ty(mut_expr);
            // let mut_expr_def_id = self.cx.tcx.hir().local_def_id(mut_expr.hir_id).to_def_id();
            // let middle_ty = self.cx.tcx.type_of(mut_expr_def_id);
            if match_type(self.cx, middle_ty, &paths::ANCHOR_LANG_ACCOUNT);
            // grab T generic parameter
            if let MiddleTyKind::Adt(_adt_def, substs) = middle_ty.kind();
            if substs.len() == ANCHOR_ACCOUNT_GENERIC_ARG_COUNT;
            let account_type = substs[1].expect_ty();
            if let Some(adt_def) = account_type.ty_adt_def();
            then {
                let def_id = adt_def.did();
                if let Some(exprs) = self.accounts.get_mut(&def_id) {
                    let mut spanless_eq = SpanlessEq::new(self.cx);
                    // check that expr is not a duplicate within its particular key-pair
                    if exprs.iter().all(|e| !spanless_eq.eq_expr(e, mut_expr)) {
                        exprs.push(mut_expr);
                    }
                } else {
                    self.accounts.insert(def_id, vec![mut_expr]);
                }
            }
        }

        // get if statements
        if_chain! {
            if let ExprKind::If(wrapped_if_expr, _then, _else_opt) = expr.kind;
            if let ExprKind::DropTemps(if_expr) = wrapped_if_expr.kind;
            if let ExprKind::Binary(op, left, right) = if_expr.kind;
            // TODO: leaves out || or &&. Could implement something that pulls apart
            // an if expr that is of this form into individual == or != comparisons
            if let BinOpKind::Ne | BinOpKind::Eq = op.node;
            then {
                // println!("{:#?}, {:#?}", expr, then);
                self.if_statements.push((left, right));
            }
        }
        walk_expr(self, expr);
    }
}
