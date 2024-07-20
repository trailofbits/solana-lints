#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_span;

use clippy_utils::{diagnostics::span_lint, match_def_path};
use if_chain::if_chain;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;
use solana_lints::paths;

dylint_linting::declare_late_lint! {
    /// **What it does:**
    ///
    /// Lint warns uses of absolute indexes with the function `sysvar::instructions::load_instruction_at_checked` and suggests to use relative indexes instead.
    ///
    /// **Why is this bad?**
    ///
    /// Using the relative indexes ensures that the instructions are implicitly correlated. The programs using
    /// absolute indexes might become vulnerable to exploits if additional validations to ensure the correlation between
    /// instructions are not performed.
    ///
    /// See [improper_instruction_introspection](https://github.com/crytic/building-secure-contracts/tree/master/not-so-smart-contracts/solana/improper_instruction_introspection) section in building-secure-contracts for more details.
    ///
    /// **Works on:**
    ///
    /// - [x] Anchor
    /// - [x] Non Anchor
    ///
    /// **Known problems:**
    ///
    /// The developer might use the relative index with the `load_instruction_at_checked` (by calculating the absolute index using the offset and the current instruction index).
    /// The lint reports these cases as well. It still a good recommendation as the developer can directly use the `get_instruction_relative` function with the offset and reduce complexity.
    ///
    /// **Example:**
    ///
    /// ```rust
    ///     pub fn mint(
    ///         ctx: Context<Mint>,
    ///         // ...
    ///     ) -> Result<(), ProgramError> {
    ///         // [...]
    ///         let transfer_ix = solana_program::sysvar::instructions::load_instruction_at_checked(
    ///             0usize,
    ///             ctx.instructions_account.to_account_info(),
    ///         )?;
    /// ```
    ///
    /// Use instead:
    ///
    /// Use a relative index, for example `-1`
    ///
    /// ```rust
    ///     pub fn mint(
    ///         ctx: Context<Mint>,
    ///         // ...
    ///     ) -> Result<(), ProgramError> {
    ///         // [...]
    ///         let transfer_ix = solana_program::sysvar::instructions::get_instruction_relative(
    ///             -1i64,
    ///             ctx.instructions_account.to_account_info(),
    ///         )?;
    /// ```
    ///
    /// **How the lint is implemented:**
    ///
    /// - For every expr
    ///   - If the expr is a call to `load_instruction_at_checked`
    ///     - Report the expression
    pub IMPROPER_INSTRUCTION_INTROSPECTION,
    Warn,
    "Using absolute indexes to access instructions instead of relative indexes"
}

impl<'tcx> LateLintPass<'tcx> for ImproperInstructionIntrospection {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'tcx>) {
        if_chain! {
            if !expr.span.from_expansion();
            if let ExprKind::Call(func_expr, _) = expr.kind;
            if is_load_instruction_fn(cx, func_expr);
            then {
                // S3v3ru5:
                // if let ExprKind::Lit(_) = arg_exprs[0]
                span_lint(
                    cx,
                    IMPROPER_INSTRUCTION_INTROSPECTION,
                    expr.span,
                    &format!(
                        "Access instructions through relative indexes using the `get_instruction_relative` helper function."
                    )
                )
            }
        }
    }
}

fn is_load_instruction_fn(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    match cx.typeck_results().type_dependent_def_id(expr.hir_id) {
        Some(def_id) => match_def_path(cx, def_id, &paths::LOAD_INSTRUCTION_AT_CHECKED),
        None => false,
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
