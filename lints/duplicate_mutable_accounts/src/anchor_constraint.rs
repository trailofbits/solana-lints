use std::default::Default;

use rustc_ast::{
    token::{Delimiter, Token, TokenKind},
    tokenstream::{DelimSpan, Spacing, TokenStream, TokenTree},
};
use rustc_hir::{def::Res, FieldDef, GenericArg, QPath, TyKind};
use rustc_span::{
    def_id::DefId,
    symbol::{Ident, Symbol},
    DUMMY_SP,
};

use crate::ANCHOR_ACCOUNT_GENERIC_ARG_COUNT;
use if_chain::if_chain;

/// Returns the `DefId` of the anchor account type, ie, `T` in `Account<'info, T>`.
/// Returns `None` if the type of `field` is not an anchor account.
pub fn get_anchor_account_type_def_id(field: &FieldDef) -> Option<DefId> {
    if_chain! {
        if let TyKind::Path(qpath) = &field.ty.kind;
        if let QPath::Resolved(_, path) = qpath;
        if !path.segments.is_empty();
        if let Some(generic_args) = path.segments[0].args;
        if generic_args.args.len() == ANCHOR_ACCOUNT_GENERIC_ARG_COUNT;
        if let GenericArg::Type(hir_ty) = &generic_args.args[1];
        then {
            get_def_id(hir_ty)
        } else {
            None
        }
    }
}

/// Returns the `DefId` of `ty`, an hir type. Returns `None` if cannot resolve type.
pub fn get_def_id(ty: &rustc_hir::Ty) -> Option<DefId> {
    if_chain! {
        if let TyKind::Path(qpath) = &ty.kind;
        if let QPath::Resolved(_, path) = qpath;
        if let Res::Def(_, def_id) = path.res;
        then {
            Some(def_id)
        } else {
            None
        }
    }
}

/// Returns a `TokenStream` of form: `a`.key() != `b`.key().
pub fn create_key_check_constraint_tokenstream(a: Symbol, b: Symbol) -> TokenStream {
    // TODO: may be more efficient way to do this, since the stream is effectively fixed
    // and determined. Only two tokens are variable.
    let constraint = vec![
        create_token_from_ident(a.as_str()),
        TokenTree::Token(Token::new(TokenKind::Dot, DUMMY_SP), Spacing::Alone),
        create_token_from_ident("key"),
        TokenTree::Delimited(
            DelimSpan::dummy(),
            Delimiter::Parenthesis,
            TokenStream::new(vec![]),
        ),
        TokenTree::Token(Token::new(TokenKind::Ne, DUMMY_SP), Spacing::Alone),
        create_token_from_ident(b.as_str()),
        TokenTree::Token(Token::new(TokenKind::Dot, DUMMY_SP), Spacing::Alone),
        create_token_from_ident("key"),
        TokenTree::Delimited(
            DelimSpan::dummy(),
            Delimiter::Parenthesis,
            TokenStream::new(vec![]),
        ),
    ];

    TokenStream::new(constraint)
}

/// Returns a `TokenTree::Token` which has `TokenKind::Ident`, with the string set to `s`.
fn create_token_from_ident(s: &str) -> TokenTree {
    let ident = Ident::from_str(s);
    TokenTree::Token(Token::from_ast_ident(ident), Spacing::Alone)
}

#[derive(Debug, Default)]
pub struct Streams(pub Vec<TokenStream>);

impl Streams {
    /// Returns true if `self` has a `TokenStream` that `other` is a substream of
    pub fn contains(&self, other: &TokenStream) -> bool {
        self.0
            .iter()
            .any(|token_stream| Self::is_substream(token_stream, other))
    }

    /// Returns true if `other` is a substream of `stream`. By substream we mean in the sense of a substring.
    // NOTE: a possible optimization is when a match is found, to remove the matched
    // TokenTrees from the TokenStream, since the constraint has been "checked" so it never
    // needs to be validated again. This cuts down the number of comparisons.
    fn is_substream(stream: &TokenStream, other: &TokenStream) -> bool {
        for i in 0..stream.len() {
            if other
                .trees()
                .enumerate()
                .all(|(j, other_token)| match stream.trees().nth(i + j) {
                    Some(token_tree) => token_tree.eq_unspanned(other_token),
                    None => false,
                })
            {
                return true;
            }
        }
        false
    }
}
