use proc_macro::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Paren;
use syn::{
    parenthesized, parse_macro_input, Attribute, FnArg, Ident, PatType, Result, ReturnType, Token,
    Type,
};

/// Parses the following syntax:
///
/// ```
/// /// Accepts a pointer to a string and returns a pointer to a JSON object.
/// /// The function created will be called "get" but will bind to "env_get".
/// env::get(key: &str) -> box Json<Option<String>>;
/// ```
#[allow(dead_code)]
struct Binding {
    attrs: Vec<Attribute>,
    mod_name: Option<Ident>,
    name: Ident,
    paren_token: Paren,
    inputs: Punctuated<FnArg, Token![,]>,
    return_arrow: Token![->],
    boxed_return: bool,
    ret: Type,
    semi_token: Token![;],
}

impl Parse for Binding {
    fn parse(input: ParseStream) -> Result<Self> {
        // Rust automatically replaces doc comments with attributes
        let attrs = input.call(Attribute::parse_outer)?;

        // This might be the module name, so if we see :: next we'll move it into the mod_name variable
        // and replace this with the actual function name.
        let mut name = input.parse::<Ident>()?;
        let mod_name = if input.parse::<Token![::]>().is_ok() {
            Some(std::mem::replace(&mut name, input.parse::<Ident>()?))
        } else {
            None
        };

        let signature_content;
        Ok(Self {
            attrs,
            mod_name,
            name,
            paren_token: parenthesized!(signature_content in input),
            inputs: signature_content.parse_terminated(FnArg::parse, Token![,])?,
            return_arrow: input.parse()?,
            boxed_return: input.parse::<Token![box]>().is_ok(),
            ret: input.parse()?,
            semi_token: input.parse()?,
        })
    }
}

/// Parses multiple [`Binding`]s at once.
struct Bindings {
    bindings: Vec<Binding>,
}

impl Parse for Bindings {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut bindings = Vec::new();
        while !input.is_empty() {
            bindings.push(input.parse()?);
        }
        Ok(Self { bindings })
    }
}

/// Add type safety to a raw binding.
///
/// The host application can provide "bindings" to the plugin, which are functions that the plugin can call.
/// Only numbers can be passed to or returned from bindings, but this does give the opportunity to pass
/// memory pointers for the host to dereference. This macro creates a translation layer to handle allocating,
/// sending the pointer, and then deallocating the memory and provides type safety so that plugin developers can
/// only send the correct pointers.
#[proc_macro]
pub fn bindings(item: TokenStream) -> TokenStream {
    let bindings = parse_macro_input!(item as Bindings);
    let mut output = TokenStream::new();

    for Binding {
        attrs,
        inputs,
        ret,
        name,
        mod_name,
        boxed_return,
        ..
    } in bindings.bindings
    {
        let arg_names = inputs.iter().map(|arg| match arg {
            FnArg::Typed(PatType { pat, .. }) => pat.to_token_stream(),
            FnArg::Receiver(_) => panic!("Receiver arguments are not supported"),
        });
        let arg_offsets = arg_names.clone().map(|arg| quote!(#arg.offset));

        let raw_name = match mod_name {
            Some(mod_name) => {
                Ident::new(&format!("{}_{}", mod_name, name.to_string()), name.span())
            }
            None => name.clone(),
        };

        let ret_span = ret.span();
        let return_expr = if boxed_return {
            quote_spanned! {ret_span=>
                <#ret as ::extism_pdk::FromBytes>::from_bytes(
                    ::extism_pdk::Memory::find(ret_value).unwrap().to_vec()
                ).unwrap()
            }
        } else {
            quote_spanned!(ret_span=> ret_value)
        };

        let tokens = quote! {
            #(#attrs)*
            pub fn #name(#inputs) -> #ret {
                #(
                    let #arg_names = ::extism_pdk::Memory::from_bytes(#arg_names);
                )*

                let ret_value = unsafe {
                    crate::raw::#raw_name(
                        #(#arg_offsets),*
                    )
                };

                #return_expr
            }
        };
        output.extend(TokenStream::from(tokens));
    }

    output
}
