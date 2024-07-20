#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::{
    diagnostics::span_lint, diagnostics::span_lint_and_then, match_any_def_paths, match_def_path,
};
use if_chain::if_chain;
use rustc_hir::{
    def::Res,
    def_id::LocalDefId,
    intravisit::{walk_expr, FnKind, Visitor},
    Body, Expr, ExprKind, FnDecl, Item, ItemKind, QPath, TyKind,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use solana_lints::anchor_syn::{AccountField, SysvarTy, Ty as FieldTy};
use solana_lints::{paths, utils::get_anchor_accounts_struct};

dylint_linting::declare_late_lint! {
    /// **What it does:**
    ///
    /// Lint warns uses of `Sysvar::from_account_info` and suggests to use `Sysvar::get` instead for
    /// the sysvars implementing `Sysvar::get` function. The following sysvars implement `Sysvar::get`:
    ///
    /// - Clock
    /// - EpochRewards
    /// - EpochSchedule
    /// - Fees
    /// - LastRestartSlot
    /// - Rent
    ///
    /// **Why is this bad?**
    ///
    /// The `Sysvar::from_account_info` is less efficient than `Sysvar::get` because:
    ///
    /// - The `from_account_info` requires that Sysvar account is passed to the program wasting the limited space
    ///   available to the transactions.
    /// - The `from_account_info` deserializes the Sysvar account data wasting the computation budget.
    ///
    /// The `Sysvar::from_account_info` should be used if and only if the program interacts with an old program that
    /// requires the sysvar account to be passed in CPI call. The program could avoid deserialization overhead by using
    /// the passed Sysvar account in CPI (after verifying the ID) and using the `Sysvar::get`.
    ///
    /// References:
    /// [`solana_program/sysvar` docs](https://docs.rs/solana-program/latest/solana_program/sysvar/index.html#:~:text=programs%20should%20prefer%20to%20call%20Sysvar%3A%3Aget),
    /// [Anchor docs](https://docs.rs/anchor-lang/latest/anchor_lang/accounts/sysvar/struct.Sysvar.html#:~:text=If%20possible%2C%20sysvars%20should%20not%20be%20used%20via%20accounts)
    ///
    /// **Works on:**
    ///
    /// - [x] Anchor
    /// - [x] Non Anchor
    ///
    /// **Known problems:**
    ///
    /// None
    ///
    /// **Example:**
    ///
    /// ```rust
    ///     let clock_account = next_account_info(account_info_iter)?;
    ///     let clock = clock::Clock::from_account_info(&clock_account)?;
    /// ```
    ///
    /// Use instead:
    ///
    /// ```rust
    ///     let clock = clock::Clock::get()?;
    /// ```
    ///
    /// **How the lint is implemented:**
    ///
    /// - For every item
    ///   - If item is a struct and has `#[derive(Accounts)]` macro
    ///   - For each field in the struct
    ///     - If field is of type Ty::Sysvar(T) and T is one of `Clock`, `EpochRewards`, `EpochSchedule`, `Fees`, `LastRestartSlot`, `Rent`
    ///       - Then report the field and suggest to T::get().
    /// - For every function
    ///   - If an expr in function calls T::x() where x is `solana_program::Sysvar::from_account_info` and
    ///     T is one of sysvars that implements `Sysvar::get()` method.
    ///     - report the call expr and suggest to use T::get().
    pub SYSVAR_GET,
    Warn,
    "Using `Sysvar::from_account_info` instead of `Sysvar::get`"
}

impl<'tcx> LateLintPass<'tcx> for SysvarGet {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        anchor_sysvar_get(cx, item);
    }

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
            let uses = find_from_account_info_exprs(cx, body);
            for (expr, sysvar) in &uses {
                span_lint(
                    cx,
                    SYSVAR_GET,
                    expr.span,
                    &format!(
                        "Use `{0}::get()` instead of `{0}::from_account_info(...)`",
                        &sysvar
                    ),
                );
            }
        }
    }
}

struct FromAccountInfoUses<'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    uses: Vec<(&'tcx Expr<'tcx>, String)>,
}

fn find_from_account_info_exprs<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
) -> Vec<(&'tcx Expr<'tcx>, String)> {
    let mut f = FromAccountInfoUses {
        cx,
        uses: Vec::new(),
    };
    f.visit_expr(body.value);
    f.uses
}

impl<'cx, 'tcx> Visitor<'tcx> for FromAccountInfoUses<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if let ExprKind::Call(func, _) = expr.kind;
            // T::x()
            if let ExprKind::Path(QPath::TypeRelative(ty_t, _)) = func.kind;
            // T::from_account_info()
            if let Some(def_id) = self.cx.typeck_results().type_dependent_def_id(func.hir_id);
            if match_def_path(self.cx, def_id, &paths::SYSVAR_FROM_ACCOUNT_INFO);
            // T is either CLOCK, EpochRewards, EpochSchedule, Fees, LastRestartSlot, Rent
            if let TyKind::Path(ty_qpath) = &ty_t.kind;
            let res = self.cx.typeck_results().qpath_res(ty_qpath, ty_t.hir_id);
            if let Res::Def(_, t_def_id) = res;
            if let Some(ind) = match_any_def_paths(
                self.cx,
                t_def_id,
                &[
                    &paths::SYSVAR_CLOCK,
                    &paths::SYSVAR_EPOCH_REWARDS,
                    &paths::SYSVAR_EPOCH_SCHEDULE,
                    &paths::SYSVAR_FEES,
                    &paths::SYSVAR_LAST_RESTART_SLOT,
                    &paths::SYSVAR_RENT,
                ],
            );
            then {
                self.uses.push((
                    expr,
                    [
                        "Clock",
                        "EpochRewards",
                        "EpochSchedule",
                        "Fees",
                        "LastRestartSlot",
                        "Rent",
                    ][ind]
                        .to_string(),
                ));
            }
        }
        walk_expr(self, expr);
    }
}

fn anchor_sysvar_get<'tcx>(cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
    if let ItemKind::Struct(variant, _) = item.kind {
        if let Some(accounts_struct) = get_anchor_accounts_struct(cx, item) {
            let mut reported_fields = Vec::new();
            for (item_field, anchor_field) in
                variant.fields().iter().zip(accounts_struct.fields.iter())
            {
                // CompositeFields have type equal to a struct that have #[derive(Accounts)].
                // The field represents multiple accounts. As this function will report that struct, Composite
                // fields are ignored here.
                // TODO: Confirm above statement.
                if let AccountField::Field(field) = anchor_field {
                    if let FieldTy::Sysvar(sysvar_ty) = &field.ty {
                        if matches!(
                            sysvar_ty,
                            SysvarTy::Clock
                                | SysvarTy::Rewards
                                | SysvarTy::EpochSchedule
                                | SysvarTy::Fees
                                | SysvarTy::Rent
                        ) {
                            reported_fields.push((item_field, format!("{sysvar_ty:?}")));
                        }
                    }
                }
            }
            if reported_fields.is_empty() {
                return;
            }
            let warn_message = if reported_fields.len() == 1 {
                format!(
                    "Use `{}::get` instead of passing the account",
                    reported_fields[0].1.as_str()
                )
            } else {
                let (last_field, fields) = reported_fields.split_last().unwrap();
                format!(
                    "Use `Sysvar::get` instead of passing the accounts for `{}`, and `{}`.",
                    fields
                        .iter()
                        .map(|f| f.1.as_str())
                        .collect::<Vec<&str>>()
                        .join("`, `"),
                    last_field.1.as_str()
                )
            };

            span_lint_and_then(
                cx,
                SYSVAR_GET,
                reported_fields.iter().map(|f| f.0.span).collect::<Vec<_>>(),
                &warn_message,
                |diag| {
                    diag.span_label(
                        item.ident.span,
                        "Sysvar accounts passed in this instruction",
                    );
                },
            );
        }
    }
}

#[test]
fn insecure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "insecure");
}

#[test]
fn secure() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure");
}
