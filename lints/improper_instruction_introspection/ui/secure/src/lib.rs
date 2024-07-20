use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_lang::solana_program::entrypoint::ProgramResult;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod improper_instruction_introspection_secure {
    use super::*;
    pub fn mint(ctx: Context<Mint>) -> ProgramResult {
        let transfer_ix = solana_program::sysvar::instructions::get_instruction_relative(
            -1i64,
            ctx.instructions_account.to_account_info(),
        )?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Mint<'info> {
    instructions_account: AccountInfo<'info>,
}

#[allow(dead_code)]
fn main() {}
