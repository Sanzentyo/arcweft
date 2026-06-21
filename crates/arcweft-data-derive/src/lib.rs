#![forbid(unsafe_code)]
//! Syntax-derived data traits for Arcweft.

mod attrs;
mod expand;
mod rename;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(ArcweftEncode, attributes(arcweft))]
pub fn derive_encode(input: TokenStream) -> TokenStream {
    expand::encode(&parse_macro_input!(input as DeriveInput)).into()
}

#[proc_macro_derive(ArcweftDecode, attributes(arcweft))]
pub fn derive_decode(input: TokenStream) -> TokenStream {
    expand::decode(&parse_macro_input!(input as DeriveInput)).into()
}

#[proc_macro_derive(ArcweftReflect, attributes(arcweft))]
pub fn derive_reflect(input: TokenStream) -> TokenStream {
    expand::reflect(&parse_macro_input!(input as DeriveInput)).into()
}
