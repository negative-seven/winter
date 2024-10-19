#![allow(clippy::missing_panics_doc)]

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, punctuated::Punctuated, token::Comma, Ident, ItemFn};

#[proc_macro]
pub fn hooks(function_name_tokens: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let function_names =
        parse_macro_input!(function_name_tokens with Punctuated::<Ident, Comma>::parse_terminated);
    let hook_item_names = function_names
        .iter()
        .map(|ident| format_ident!("__HOOK_{ident}"));
    quote!(
        [#(#hook_item_names),*]
    )
    .into()
}

#[proc_macro_attribute]
pub fn hook(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let attr = TokenStream::from(attr);
    let function = parse_macro_input!(item as ItemFn);

    let function_type = {
        assert!(function.sig.generics.params.empty_or_trailing());

        let vis = &function.vis;
        let constness = &function.sig.constness;
        let asyncness = &function.sig.asyncness;
        let unsafety = &function.sig.unsafety;
        let abi = &function.sig.abi;
        let inputs = &function.sig.inputs;
        let variadic = &function.sig.variadic;
        let output = &function.sig.output;
        quote!(#vis #constness #asyncness #unsafety #abi fn(#inputs #variadic) #output)
    };
    let module = attr.into_iter().next().unwrap();
    let function_name = &function.sig.ident;
    let item_name = format_ident!("__HOOK_{function_name}");
    quote!(
        #[allow(non_upper_case_globals)]
        const #item_name: (&str, &str, *const winapi::ctypes::c_void) = (
            #module,
            stringify!(#function_name),
            {
                // "rename" the function without needing to change its identifier
                const REPLACEMENT_FUNCTION: #function_type = {
                    #[allow(non_snake_case)]
                    #function
                    #function_name
                };

                // validate function types
                const {
                    let mut f: #function_type;
                    f = REPLACEMENT_FUNCTION;
                    f = #function_name;
                }

                REPLACEMENT_FUNCTION as *const winapi::ctypes::c_void
            },
        );
    )
    .into()
}
