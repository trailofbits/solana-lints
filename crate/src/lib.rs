#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;

#[allow(unused_extern_crates)]
extern crate rustc_driver;

pub mod paths;

pub mod utils;

pub use anchor_syn;
pub use syn;
