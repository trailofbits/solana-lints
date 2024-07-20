use anchor_lang::prelude::*;
use anchor_lang::solana_program::entrypoint::ProgramResult;
use anchor_lang::solana_program;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const TRANSFER_IX_INDEX: usize = 1;

#[program]
pub mod improper_instruction_introspection_insecure {
    use super::*;

    pub fn mint(ctx: Context<Mint>, deposit_index: usize) -> ProgramResult {

        let transfer_ix = solana_program::sysvar::instructions::load_instruction_at_checked(
            0usize,
            ctx.instructions_account.to_account_info(),
        )?;

        let transfer_ix_2 = solana_program::sysvar::instructions::load_instruction_at_checked(
            TRANSFER_IX_INDEX,
            ctx.instructions_account.to_account_info(),
        )?;

        let index = 2;

        let transfer_ix_3 = solana_program::sysvar::instructions::load_instruction_at_checked(
            index,
            ctx.instructions_account.to_account_info(),
        )?;

        let deposit_ix = solana_program::sysvar::instructions::load_instruction_at_checked(
            deposit_index,
            ctx.instructions_account.to_account_info(),
        )?;
        Ok(())
    }

    pub fn mint_relative(ctx: Context<MintRelative>) -> ProgramResult {
        // Instruction is accessed using relative index. The program could use `get_instruction_relative` function
        // instead of calculating absolute index.
        let offset = 1;
        let transfer_ix_index = solana_program::sysvar::instructions::load_current_index_checked(
            ctx.instructions_account.to_account_info(),
        )?;

        let transfer_ix = solana_program::sysvar::instructions::load_instruction_at_checked(
            transfer_ix_index - offset,
            ctx.instructions_account.to_account_info()
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Mint {
    instructions_account: AccountInfo<'info>
}

#[derive(Accounts)]
pub struct MintRelative {
    instructions_account: AccountInfo<'info>
}

#[allow(dead_code)]
fn main() {}
