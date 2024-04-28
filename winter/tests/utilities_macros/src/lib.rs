#![allow(clippy::missing_panics_doc)]

use itertools::Itertools;
use proc_macro2::{TokenStream, TokenTree};
use quote::{format_ident, quote};
use syn::{parse2, ItemFn};

#[proc_macro_attribute]
pub fn test_for(
    attributes: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    struct Variant {
        suffix: &'static str,
        argument: TokenStream,
    }

    let attributes = TokenStream::from(attributes);
    let item = TokenStream::from(item);

    let mut categories = Vec::new();
    let mut attributes = attributes.into_iter();
    loop {
        match attributes.next() {
            Some(TokenTree::Ident(key)) => {
                categories.push(key);
            }
            None => break,
            _ => panic!("expected an identifier"),
        }
        match attributes.next() {
            Some(TokenTree::Punct(p)) if p.as_char() == ',' => (),
            None => break,
            _ => panic!("expected a comma or end of attributes"),
        }
    }

    let mut variant_groups = Vec::new();
    for category in categories {
        match &*category.to_string() {
            "architecture" => {
                variant_groups.push(vec![
                    Variant {
                        suffix: "_x86",
                        argument: quote!(test_utilities::Architecture::X86),
                    },
                    Variant {
                        suffix: "_x64",
                        argument: quote!(test_utilities::Architecture::X64),
                    },
                ]);
            }
            _ => panic!("expected \"architecture\""),
        }
    }

    let inner_function = parse2::<ItemFn>(item).expect("expected function");
    let inner_function_name = &inner_function.sig.ident;
    let function_return_type = &inner_function.sig.output;

    let mut outer_function_names = Vec::new();
    let mut inner_function_argument_lists = Vec::new();
    for variant_combination in variant_groups.iter().multi_cartesian_product() {
        let mut outer_function_name = format_ident!("{inner_function_name}");
        let mut inner_function_arguments = Vec::new();
        for variant in variant_combination {
            outer_function_name = format_ident!("{outer_function_name}{}", variant.suffix);
            inner_function_arguments.push(&variant.argument);
        }
        outer_function_names.push(outer_function_name);
        inner_function_argument_lists.push(
            inner_function_arguments
                .into_iter()
                .flat_map(|a| a.clone().into_iter())
                .collect::<TokenStream>(),
        );
    }

    quote! {
        #(
            #[tokio::test]
            async fn #outer_function_names() #function_return_type {
                #inner_function
                #inner_function_name(#inner_function_argument_lists).await
            }
        )*
    }
    .into()
}
