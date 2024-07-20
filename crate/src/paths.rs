// smoelius: Default const names to:
//   crate '_' last_segment
// all in upper snake case.

pub const ANCHOR_LANG_ACCOUNT: [&str; 4] = ["anchor_lang", "accounts", "account", "Account"];
pub const ANCHOR_LANG_ACCOUNT_LOADER: [&str; 4] =
    ["anchor_lang", "accounts", "account_loader", "AccountLoader"];
pub const ANCHOR_LANG_PROGRAM: [&str; 4] = ["anchor_lang", "accounts", "program", "Program"];
pub const ANCHOR_LANG_INTERFACE: [&str; 4] = ["anchor_lang", "accounts", "interface", "Interface"];
pub const ANCHOR_LANG_SYSTEM_ACCOUNT: [&str; 4] =
    ["anchor_lang", "accounts", "system_account", "SystemAccount"];
pub const ANCHOR_LANG_ACCOUNT_DESERIALIZE: [&str; 2] = ["anchor_lang", "AccountDeserialize"];
pub const ANCHOR_LANG_CONTEXT: [&str; 3] = ["anchor_lang", "context", "Context"];
pub const ANCHOR_LANG_DISCRIMINATOR: [&str; 2] = ["anchor_lang", "Discriminator"];
pub const ANCHOR_LANG_SIGNER: [&str; 4] = ["anchor_lang", "accounts", "signer", "Signer"];
pub const ANCHOR_LANG_SYSVAR: [&str; 4] = ["anchor_lang", "accounts", "sysvar", "Sysvar"];
pub const ANCHOR_LANG_TO_ACCOUNT_INFO: [&str; 3] =
    ["anchor_lang", "ToAccountInfo", "to_account_info"];
pub const ANCHOR_LANG_TRY_DESERIALIZE: [&str; 3] =
    ["anchor_lang", "AccountDeserialize", "try_deserialize"];
// key() method call path
pub const ANCHOR_LANG_KEY: [&str; 3] = ["anchor_lang", "Key", "key"];
pub const ANCHOR_LANG_TO_ACCOUNT_INFOS_TRAIT: [&str; 2] = ["anchor_lang", "ToAccountInfos"];
// CpiContext::new()
pub const ANCHOR_CPI_CONTEXT_NEW: [&str; 4] = ["anchor_lang", "context", "CpiContext", "new"];
// CpiContext::new_with_signer()
pub const ANCHOR_CPI_CONTEXT_NEW_SIGNER: [&str; 4] =
    ["anchor_lang", "context", "CpiContext", "new_with_signer"];
pub const BORSH_TRY_FROM_SLICE: [&str; 4] = ["borsh", "de", "BorshDeserialize", "try_from_slice"];

pub const CORE_BRANCH: [&str; 5] = ["core", "ops", "try_trait", "Try", "branch"];
pub const CORE_CLONE: [&str; 4] = ["core", "clone", "Clone", "clone"];

pub const SOLANA_PROGRAM_ACCOUNT_INFO: [&str; 3] =
    ["solana_program", "account_info", "AccountInfo"];
pub const SOLANA_PROGRAM_INVOKE: [&str; 3] = ["solana_program", "program", "invoke"];
// Instruction {..}
pub const SOLANA_PROGRAM_INSTRUCTION: [&str; 3] = ["solana_program", "instruction", "Instruction"];
pub const SOLANA_PROGRAM_CREATE_PROGRAM_ADDRESS: [&str; 4] = [
    "solana_program",
    "pubkey",
    "Pubkey",
    "create_program_address",
];

pub const SPL_TOKEN_INSTRUCTION: [&str; 2] = ["spl_token", "instruction"];

pub const SYSVAR_FROM_ACCOUNT_INFO: [&str; 4] =
    ["solana_program", "sysvar", "Sysvar", "from_account_info"];
pub const SYSVAR_CLOCK: [&str; 3] = ["solana_program", "clock", "Clock"];
pub const SYSVAR_EPOCH_REWARDS: [&str; 3] = ["solana_program", "epoch_rewards", "EpochRewards"];
pub const SYSVAR_EPOCH_SCHEDULE: [&str; 3] = ["solana_program", "epoch_schedule", "EpochSchedule"];
pub const SYSVAR_FEES: [&str; 3] = ["solana_program", "fees", "Fees"];
pub const SYSVAR_LAST_RESTART_SLOT: [&str; 3] =
    ["solana_program", "last_restart_slot", "LastRestartSlot"];
pub const SYSVAR_RENT: [&str; 3] = ["solana_program", "rent", "Rent"];
pub const LOAD_INSTRUCTION_AT_CHECKED: [&str; 3] = [
    "solana_program",
    "instructions",
    "load_instruction_at_checked",
];
