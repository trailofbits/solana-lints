#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use clippy_utils::{
    diagnostics::span_lint, match_any_def_paths, match_def_path, ty::match_type, SpanlessEq,
};
use if_chain::if_chain;
use rustc_hir::{
    def_id::LocalDefId,
    intravisit::{walk_expr, FnKind, Visitor},
    BinOpKind, Body, Expr, ExprKind, FnDecl, QPath,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;
use solana_lints::{paths, utils::visit_expr_no_bodies};

dylint_linting::declare_late_lint! {
    /// **What it does:**
    ///
    /// This lint checks that for each account referenced in a program, that there is a
    /// corresponding owner check on that account. Specifically, this means that the owner
    /// field is referenced on that account.
    ///
    /// **Why is this bad?**
    ///
    /// The missing-owner-check vulnerability occurs when a program uses an account, but does
    /// not check that it is owned by the expected program. This could lead to vulnerabilities
    /// where a malicious actor passes in an account owned by program `X` when what was expected
    /// was an account owned by program `Y`. The code may then perform unexpected operations
    /// on that spoofed account.
    ///
    /// For example, suppose a program expected an account to be owned by the SPL Token program.
    /// If no owner check is done on the account, then a malicious actor could pass in an
    /// account owned by some other program. The code may then perform some actions on the
    /// unauthorized account that is not owned by the SPL Token program.
    ///
    /// **Known problems:**
    ///
    /// Key checks can be strengthened. Currently, the lint only checks that the account's owner
    /// field is referenced somewhere, ie, `AccountInfo.owner`.
    ///
    /// **Example:**
    ///
    /// See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/2-owner-checks/insecure/src/lib.rs
    /// for an insecure example.
    ///
    /// Use instead:
    ///
    /// See https://github.com/coral-xyz/sealevel-attacks/blob/master/programs/2-owner-checks/secure/src/lib.rs
    /// for a secure example.
    pub MISSING_OWNER_CHECK,
    Warn,
    "using an account without checking if its owner is as expected"
}

impl<'tcx> LateLintPass<'tcx> for MissingOwnerCheck {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        _: LocalDefId,
    ) {
        if !span.from_expansion() {
            let accounts = get_referenced_accounts(cx, body);
            for account_expr in accounts {
                if !contains_owner_use(cx, body, account_expr)
                    && !contains_key_check(cx, body, account_expr)
                {
                    span_lint(
                        cx,
                        MISSING_OWNER_CHECK,
                        account_expr.span,
                        "this Account struct is used but there is no check on its owner field",
                    );
                }
            }
        }
    }
}

struct AccountUses<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    uses: Vec<&'tcx Expr<'tcx>>,
}

fn get_referenced_accounts<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
) -> Vec<&'tcx Expr<'tcx>> {
    let mut accounts = AccountUses {
        cx,
        uses: Vec::new(),
    };

    accounts.visit_expr(body.value);
    accounts.uses
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            // s3v3ru5: the following check removes duplicate warnings where lint would report both `x` and `x.clone()` expressions.
            // ignore `clone()` expressions
            if is_expr_method_call(self.cx, expr, &paths::CORE_CLONE).is_none();
            let ty = self.cx.typeck_results().expr_ty(expr);
            if match_type(self.cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO);
            if !is_expr_local_variable(expr);
            if !is_safe_to_account_info(self.cx, expr);
            let mut spanless_eq = SpanlessEq::new(self.cx);
            if !self.uses.iter().any(|e| spanless_eq.eq_expr(e, expr));
            then {
                self.uses.push(expr);
            }
        }
        walk_expr(self, expr);
    }
}

// s3v3ru5: if a local variable is of type AccountInfo, the rhs of the let statement assigning to variable
// will be of type AccountInfo. The lint would check that expression and there is no need for checking the
// local variable as well.
// This removes the false positives of following pattern:
// `let x = {Account, Program, ...verified structs}.to_account_info()`,
// the lint reports uses of `x`. Having this check would remove such false positives.
fn is_expr_local_variable<'tcx>(expr: &'tcx Expr<'tcx>) -> bool {
    if_chain! {
        if let ExprKind::Path(QPath::Resolved(None, path)) = expr.kind;
        if path.segments.len() == 1;
        then {
            true
        } else {
            false
        }
    }
}

// smoelius: See: https://github.com/crytic/solana-lints/issues/31
fn is_safe_to_account_info<'tcx>(cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) -> bool {
    if_chain! {
        // is the expression method call `to_account_info()`
        if let Some(recv) = is_expr_method_call(cx, expr, &paths::ANCHOR_LANG_TO_ACCOUNT_INFO);
        if let ty::Ref(_, recv_ty, _) = cx.typeck_results().expr_ty_adjusted(recv).kind();
        if let ty::Adt(adt_def, _) = recv_ty.kind();
        // smoelius:
        // - `Account` requires its type argument to implement `anchor_lang::Owner`.
        // - `Program`'s implementation of `try_from` checks the account's program id. So there is
        //   no ambiguity in regard to the account's owner.
        // - `SystemAccount`'s implementation of `try_from` checks that the account's owner is the
        //   System Program.
        // - `AccountLoader` requires its type argument to implement `anchor_lang::Owner`.
        // - `Signer` are mostly accounts with a private key and most of the times owned by System Program.
        // - `Sysvar` type arguments checks the account key.
        if match_any_def_paths(
            cx,
            adt_def.did(),
            &[
                &paths::ANCHOR_LANG_ACCOUNT,
                &paths::ANCHOR_LANG_PROGRAM,
                &paths::ANCHOR_LANG_SYSTEM_ACCOUNT,
                &paths::ANCHOR_LANG_ACCOUNT_LOADER,
                &paths::ANCHOR_LANG_SIGNER,
                &paths::ANCHOR_LANG_SYSVAR,
                // s3v3ru5: The following line will remove duplicate warnings where lint reports both `x` and `x.to_account_info()` when x is of type Anchor's AccountInfo.
                &paths::SOLANA_PROGRAM_ACCOUNT_INFO,
            ],
        )
        .is_some();
        then {
            true
        } else {
            false
        }
    }
}

/// if `expr` is a method call of `def_path` return the receiver else None
fn is_expr_method_call<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    def_path: &[&str],
) -> Option<&'tcx Expr<'tcx>> {
    if_chain! {
        if let ExprKind::MethodCall(_, recv, _, _) = expr.kind;
        if let Some(def_id) = cx.typeck_results().type_dependent_def_id(expr.hir_id);
        if match_def_path(cx, def_id, def_path);
        then {
            Some(recv)
        } else {
            None
        }
    }
}

fn contains_owner_use<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    visit_expr_no_bodies(body.value, |expr| {
        uses_given_field(cx, expr, account_expr, "owner")
    })
}

/// Checks if `expr` is references `field` on `account_expr`
fn uses_given_field<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    account_expr: &Expr<'tcx>,
    field: &str,
) -> bool {
    if_chain! {
        if let ExprKind::Field(object, field_name) = expr.kind;
        // TODO: add check for key, is_signer
        if field_name.as_str() == field;
        let mut spanless_eq = SpanlessEq::new(cx);
        if spanless_eq.eq_expr(account_expr, object);
        then {
            true
        } else {
            false
        }
    }
}

/// Checks if `expr` is a method call of `path` on `account_expr`
fn calls_method_on_expr<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    account_expr: &Expr<'tcx>,
    def_path: &[&str],
) -> bool {
    if_chain! {
        // check if expr is a method call
        if let Some(recv) = is_expr_method_call(cx, expr, def_path);
        // check if recv is same expression as account_expr
        let mut spanless_eq = SpanlessEq::new(cx);
        if spanless_eq.eq_expr(account_expr, recv);
        then {
            true
        } else {
            false
        }
    }
}

// Return true if the expr access key of account_expr(AccountInfo)
fn expr_accesses_key<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    // Anchor AccountInfo: `.key()` and Solana AccountInfo: `.key` field.
    calls_method_on_expr(cx, expr, account_expr, &paths::ANCHOR_LANG_KEY)
        || uses_given_field(cx, expr, account_expr, "key")
}

fn contains_key_check<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    visit_expr_no_bodies(body.value, |expr| compares_key(cx, expr, account_expr))
}

fn compares_key<'tcx>(
    cx: &LateContext<'tcx>,
    expr: &Expr<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    if_chain! {
        // check if the expr is comparison expression
        if let ExprKind::Binary(op, lhs, rhs) = expr.kind;
        // == or !=
        if matches!(op.node, BinOpKind::Eq | BinOpKind::Ne);
        // check if lhs or rhs accesses key of `account_expr`
        if expr_accesses_key(cx, lhs, account_expr) || expr_accesses_key(cx, rhs, account_expr);
        then {
            true
        } else {
            false
        }
    }
}

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

#[test]
fn recommended() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "recommended");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}

#[test]
fn secure_fixed() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure-fixed");
}

#[test]
fn secure_account_owner() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure-account-owner");
}

#[test]
fn secure_programn_id() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure-program-id");
}
