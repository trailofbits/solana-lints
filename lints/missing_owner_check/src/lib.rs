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
    def_id::{DefId, LocalDefId},
    intravisit::{walk_expr, FnKind, Visitor},
    BinOpKind, Body, Expr, ExprKind, FnDecl, Item, QPath,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;
use solana_lints::anchor_syn::{AccountField, AccountsStruct, ConstraintGroup};
use solana_lints::{paths, utils::get_anchor_accounts_struct, utils::visit_expr_no_bodies};
use std::collections::HashMap;

dylint_linting::impl_late_lint! {
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
    /// **Works on:**
    ///
    /// - [x] Anchor
    /// - [x] Non Anchor
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
    ///
    /// **How the lint is implemented:**
    ///
    /// check_fn:
    ///
    /// - for every function defined in the package
    /// - exclude functions generated from macro expansion.
    /// - Get a list of unique and unsafe AccountInfo's referenced in the body
    ///   - for each expression in the function body
    ///   - Ignore `.clone()` expressions as the expression referencing original account will be checked
    ///   - Check if the expression's type is Solana's `AccountInfo` (`solana_program::account_info::AccountInfo`)
    ///   - Ignore local variable expressions (`x` where x is defined in the function `let x = y`)
    ///     - Removes duplcate warnings: both `x` and `y` are reported by the lint. reporting `y` is sufficient.
    ///     - Also the owner could be checked on `y`. reporting `x` which a copy/ref of `y` would be false-positive.
    ///     - Determined using the expression kind (`.kind`): expr.kind = ExprKind::Path(QPath::Resolved(None, path)); path.segments.len() == 1
    ///   - Ignore safe `.to_account_info()` expressions
    ///     - `.to_account_info()` method can be called to convert different Anchor account types to `AccountInfo`
    ///     - The Anchor account types such as `Account` implement `Owner` trait: The owner of the account is checked during deserialization
    ///     - The expressions `x.to_account_info()` where `x` has one of following types are ignored:
    ///       - `Account` requires its type argument to implement `anchor_lang::Owner`.
    ///       - `Program`'s implementation of `try_from` checks the account's program id. So there is
    ///         no ambiguity in regard to the account's owner.
    ///       - `SystemAccount`'s implementation of `try_from` checks that the account's owner is the System Program.
    ///       - `AccountLoader` requires its type argument to implement `anchor_lang::Owner`.
    ///       - `Signer` are mostly accounts with a private key and most of the times owned by System Program.
    ///       - `Sysvar` type arguments checks the account key.
    ///   - Ignore `x.to_account_info()` expressions called on Anchor `AccountInfo` to remove duplicates.
    ///     - the lint checks the original expression `x`; no need for checking both.
    /// - For each of the collected expressions, check if `owner` is accessed or if the `key` is compared
    ///   - Ignore the `account_expr` if any of the expressions in the function is `{account_expr}.owner`
    ///   - Ignore the `account_expr` if `key` is compared
    ///     - if there is a comparison expression (`==` or `!=`) and one of the expressions being compared accesses key on `account_expr`:
    ///       - lhs or rhs of the comparison is `{account_expr}.key()`; The key for Anchor's `AccountInfo` is accessed using `.key()`
    ///       - Or lhs or rhs is `{account_expr}.key`; The key of Solana `AccountInfo` are accessed using `.key`
    ///   - Else
    ///     - If the expression is `.to_account_info()` and the receiver is a field access on a struct: `x.y.to_account_info()`
    ///     - Or If the expression is a field access on a struct `x.y`
    ///       - Then store the struct(x) def id and the accessed field name (y) in `MissingOwnerCheck.account_exprs`.
    ///     - Else report the expression.
    ///
    /// check_item: Collect Anchor `Accounts` structs
    ///
    /// - for each item defined in the crate
    ///   - If Item is a Struct and implements `anchor_lang::ToAccountInfos` trait.
    ///     - Get the pre-expansion source code and parse it using anchor's accounts parser
    ///     - If parsing succeeds
    ///       - Then store the struct def id and the resultant AccountsStruct in `MissingOwnerCheck.anchor_accounts`
    ///
    /// check_crate_post:
    ///
    /// - for each account expression in `MissingOwnerCheck.account_exprs`
    ///   - If the struct accessed in the expression is in `MissingOwnerCheck.anchor_accounts`
    ///     - find the `#[account(...)]` constraints applied on the accessed field
    ///     - If any of the following constraints are applied on the field/account
    ///       - Then ignore the expression.
    ///       - Constraints:
    ///         - `#[account(signer)]` - Signer accounts are assumed to be EOA accounts and are ignored.
    ///         - `#[account(init, ...)]` - init creates a new account and sets its owner to current program or the given program.
    ///         - `#[account(seeds = ..., ...)]` - Anchor derives a PDA using the seeds. This is essentially a `key` check
    ///         - `#[account(address = ...)]` - Validates the key of the account.
    ///         - `#[account(owner = ...)]` - Checks the owner.
    ///         - `#[account(executable)]` - The account is an executable; All executables are owned by `BPFLoaders`.
    ///       - Else report the expression.
    pub MISSING_OWNER_CHECK,
    Warn,
    "using an account without checking if its owner is as expected",
    MissingOwnerCheck::new()
}

struct MissingOwnerCheck {
    pub anchor_accounts: HashMap<DefId, AccountsStruct>,
    pub account_exprs: Vec<(Span, DefId, String)>,
}

impl MissingOwnerCheck {
    pub fn new() -> Self {
        Self {
            anchor_accounts: HashMap::new(),
            account_exprs: Vec::new(),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for MissingOwnerCheck {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        if let Some(accounts_struct) = get_anchor_accounts_struct(cx, item) {
            // item is an anchor accounts struct
            self.anchor_accounts
                .insert(item.owner_id.to_def_id(), accounts_struct);
        }
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
        // exclude functions generated from macro expansions
        if !span.from_expansion() {
            // get unique and unsafe AccountInfo's referenced in the body
            let accounts = get_referenced_accounts(cx, body);
            for account_expr in accounts {
                // ignore the account_expr if `.owner` field is accessed in the function
                // or key of account_expr is compared using `==` or `!=` in the function
                if !contains_owner_use(cx, body, account_expr)
                    && !contains_key_check(cx, body, account_expr)
                {
                    if let Some((def_id, field_name)) = accesses_anchor_account(cx, account_expr) {
                        self.account_exprs
                            .push((account_expr.span, def_id, field_name));
                    } else {
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

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (span, def_id, field_name) in &self.account_exprs {
            if let Some(accounts_struct) = self.anchor_accounts.get(def_id) {
                if let Some((_, constraints)) = accounts_struct
                    .fields
                    .iter()
                    .map(|account_field| match account_field {
                        AccountField::Field(field) => (field.ident.to_string(), &field.constraints),
                        AccountField::CompositeField(field) => {
                            (field.ident.to_string(), &field.constraints)
                        }
                    })
                    .find(|(anchor_field_name, _)| anchor_field_name == field_name)
                {
                    if is_safe_constraint_for_owner(constraints) {
                        continue;
                    }
                }
            }
            span_lint(
                cx,
                MISSING_OWNER_CHECK,
                *span,
                "this Account struct is used but there is no check on its owner field",
            );
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

    // visit each expr in the body and collect AccountInfo's
    accounts.visit_expr(body.value);
    accounts.uses
}

impl<'cx, 'tcx> Visitor<'tcx> for AccountUses<'cx, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            // s3v3ru5: the following check removes duplicate warnings where lint would report both `x` and `x.clone()` expressions.
            // ignore `clone()` expressions
            if is_expr_method_call(self.cx, expr, &paths::CORE_CLONE).is_none();
            // type of the expression must be Solana's AccountInfo.
            let ty = self.cx.typeck_results().expr_ty(expr);
            if match_type(self.cx, ty, &paths::SOLANA_PROGRAM_ACCOUNT_INFO);
            // ignore expressions which are local variables
            if !is_expr_local_variable(expr);
            // `to_account_info()` returns AccountInfo. look for expressions calling `to_account_info` and ignore safe expressions
            // expression is safe if `to_account_info` is called on Anchor "Owner" types such as Account, which check the owner during deserialization
            if !is_safe_to_account_info(self.cx, expr);
            // check if this expression has been detected and is present in the already collected expressions Vec.
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
        // The expressions accessing simple local variables have expr.kind = ExprKind::Path(QPath::Resolved(None, path))
        // where path only has one segment.
        // Note: The check could be improved by including more checks on path/expr or following a different approach which uses Res.
        // matches!(tcx.hir().qpath_res(qpath, expr.hir_id), Res::Local(_))
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
        // expr_ty_adjusted removes wrappers such as Box, any other implicit conversions and gives the base type
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

/// Given an expression, if the expr accesses account from a struct return `DefId` of the struct and the field name
/// - if expr is a `to_account_info()` method call
///     - then expr = receiver
/// - if expr is a field access on T
///     - return def id of T and the field name.
/// - return None
fn accesses_anchor_account<'tcx>(
    cx: &LateContext<'tcx>,
    mut expr: &'tcx Expr<'tcx>,
) -> Option<(DefId, String)> {
    if let Some(receiver) = is_expr_method_call(cx, expr, &paths::ANCHOR_LANG_TO_ACCOUNT_INFO) {
        // This covers `UncheckedAccount` type. Anchor AccountInfo are flaged by lint directly
        // but UncheckedAccount are only flaged when `to_account_info()` is called on them.
        expr = receiver;
    };
    if_chain! {
        if let ExprKind::Field(recv, field_name) = expr.kind;
        if let ty::Adt(adt_def, _) = cx.typeck_results().expr_ty_adjusted(recv).kind();
        then {
            Some((adt_def.did(), field_name.to_string()))
        } else {
            None
        }
    }
}

/// Given an Anchor `ConstraintGroup`, check if the constraints warrant the exemption of the owner check
/// - if any of the following constraints are applied on the account return true
///     - Constraints:
///     - `#[account(signer)]` - Signer accounts are assumed to be EOA accounts and are ignored.
///         See comment in fn `is_safe_to_account_info`.
///     - `#[account(init, ...)]` - init creates a new account and sets its owner to current program or the given program.
///     - `#[account(seeds = ..., ...)]` - Anchor derives a PDA using the seeds. This is essentially a `key` check and we ignore
///         if the key of the account is validated.
///     - `#[account(address = ...)]` - Validates the key of the account.
///     - `#[account(owner = ...)]` - Checks the owner.
///     - `#[account(executable)]` - The account is an executable; All executables are owned by `BPFLoaders` and these
///         accounts are considered to be exempt from owner check.
/// - else return false
fn is_safe_constraint_for_owner(constraints: &ConstraintGroup) -> bool {
    constraints.signer.is_some()
        || constraints
            .init
            .as_ref()
            .map_or(false, |init_constraint| init_constraint.if_needed)
        || constraints.seeds.is_some()
        || constraints.address.is_some()
        || constraints.owner.is_some()
        || constraints.executable.is_some()
}

/// Check if any of the expressions in the body is `{account_expr}.owner`
fn contains_owner_use<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    visit_expr_no_bodies(body.value, |expr| {
        uses_given_field(cx, expr, account_expr, "owner")
    })
}

/// Check if the key of account returned by `account_expr` is compared
fn contains_key_check<'tcx>(
    cx: &LateContext<'tcx>,
    body: &'tcx Body<'tcx>,
    account_expr: &Expr<'tcx>,
) -> bool {
    visit_expr_no_bodies(body.value, |expr| compares_key(cx, expr, account_expr))
}

/// Check if expr is a comparison expression and one of expressions being compared accesses key on `account_expr`
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

#[test]
fn secure_anchor_constraints() {
    dylint_testing::ui_test_example(env!("CARGO_PKG_NAME"), "secure-anchor-constraints");
}
