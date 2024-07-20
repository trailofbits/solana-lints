# duplicate_mutable_accounts

**What it does:** Checks to make sure there is a key check on identical Anchor accounts.
The key check serves to make sure that two identical accounts do not have the same key,
ie, they are unique. An Anchor account (`Account<'info, T>`) is identical to another if
the generic parameter `T` is the same type for each account.

**Why is this bad?** If a program contains two identical, mutable Anchor accounts, and
performs some operation on those accounts, then a user could pass in the same account
twice. Then any previous operations may be overwritten by the last operation, which may
not be what the program wanted if it expected different accounts.

**Known problems:** If a program is not using the anchor `#[account]` macro constraints,
and is instead using checks in the function bodies, and the program uses boolean operator
&& or || to link constraints in a single if statement, the lint will flag this as a false
positive since the lint only catches statements with `==` or `!=`.

Another issue is if a program uses an if statement such as `a.key() == b.key()` and then
continues to modify the accounts, then this will not be caught. The reason is because the
lint regards expressions with `==` as a secure check, since it assumes the program will
then return an error (see the secure example). However, it does not explicitly check that
an error is returned.

In general, this lint will catch all vulnerabilities if the anchor macro constraints are
used (see the recommended example). It is not as robust if alternative methods are utilized.
Thus it is encouraged to use the anchor `#[account]` macro constraints.

**Example:**

```rust
#[derive(Accounts)]
pub struct Update<'info> {
    user_a: Account<'info, User>,
    user_b: Account<'info, User>,
}
```

Use instead:

```rust
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(constraint = user_a.key() != user_b.key())]
    user_a: Account<'info, User>,
    user_b: Account<'info, User>,
}
```
