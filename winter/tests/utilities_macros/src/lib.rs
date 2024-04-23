#![allow(clippy::missing_panics_doc)]

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse, ItemFn};

#[proc_macro_attribute]
pub fn test_per_architecture(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let inner_function = parse::<ItemFn>(item).expect("expected function");
    let inner_function_name = &inner_function.sig.ident;
    let function_return_type = &inner_function.sig.output;

    let outer_function_x86_name = format_ident!("{inner_function_name}_x86");
    let outer_function_x64_name = format_ident!("{inner_function_name}_x64");

    quote! {
        #[tokio::test]
        async fn #outer_function_x86_name() #function_return_type {
            #inner_function
            #inner_function_name(test_utilities::Architecture::X86).await
        }

        #[tokio::test]
        async fn #outer_function_x64_name() #function_return_type {
            #inner_function
            #inner_function_name(test_utilities::Architecture::X64).await
        }
    }
    .into()
}
