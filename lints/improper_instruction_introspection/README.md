# improper_instruction_introspection

**What it does:**

Lint warns uses of absolute indexes with the function `sysvar::instructions::load_instruction_at_checked` and suggests to use relative indexes instead.

**Why is this bad?**

Using the relative indexes ensures that the instructions are implicitly correlated. The programs using
absolute indexes might become vulnerable to exploits if additional validations to ensure the correlation between
instructions are not performed.

See [improper_instruction_introspection](https://github.com/crytic/building-secure-contracts/tree/master/not-so-smart-contracts/solana/improper_instruction_introspection) section in building-secure-contracts for more details.

**Works on:**

- [x] Anchor
- [x] Non Anchor

**Known problems:**

The developer might use the relative index with the `load_instruction_at_checked` (by calculating the absolute index using the offset and the current instruction index).
The lint reports these cases as well. It still a good recommendation as the developer can directly use the `get_instruction_relative` function with the offset and reduce complexity.

**Example:**

```rust
    pub fn mint(
        ctx: Context<Mint>,
        // ...
    ) -> Result<(), ProgramError> {
        // [...]
        let transfer_ix = solana_program::sysvar::instructions::load_instruction_at_checked(
            0usize,
            ctx.instructions_account.to_account_info(),
        )?;
```

Use instead:

Use a relative index, for example `-1`

```rust
    pub fn mint(
        ctx: Context<Mint>,
        // ...
    ) -> Result<(), ProgramError> {
        // [...]
        let transfer_ix = solana_program::sysvar::instructions::get_instruction_relative(
            -1i64,
            ctx.instructions_account.to_account_info(),
        )?;
```

**How the lint is implemented:**

- For every expr
  - If the expr is a call to `load_instruction_at_checked`
    - Report the expression
