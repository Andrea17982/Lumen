use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ItemFn, Pat, ReturnType, Type};

/// Keep the command implementation and its JSON boundary in one place.
#[proc_macro_attribute]
pub fn command(_: TokenStream, input: TokenStream) -> TokenStream {
    let f = parse_macro_input!(input as ItemFn);
    let name = &f.sig.ident;
    let wrapper = format_ident!("__ipc_{}", name);
    let mut bindings = Vec::new();
    let mut params = Vec::new();
    for arg in &f.sig.inputs {
        let FnArg::Typed(arg) = arg else {
            panic!("commands cannot have self")
        };
        let Pat::Ident(pat) = &*arg.pat else {
            panic!("commands require named arguments")
        };
        let ident = &pat.ident;
        let ty = &arg.ty;
        let typename = quote!(#ty).to_string();
        if typename.contains("CoreRef") {
            params.push(quote!(crate::CoreRef(core)));
        } else if typename.contains("EventHub") {
            params.push(quote!(app.clone()));
        } else {
            let mut key = String::new();
            let mut upper = false;
            for c in ident.to_string().chars() {
                if c == '_' {
                    upper = true;
                } else if upper {
                    key.extend(c.to_uppercase());
                    upper = false;
                } else {
                    key.push(c);
                }
            }
            bindings.push(quote! {
                let #ident: #ty = serde_json::from_value(args.get(#key).cloned().unwrap_or(serde_json::Value::Null))
                    .map_err(|_| format!("Invalid argument: {}", #key))?;
            });
            params.push(quote!(#ident));
        }
    }
    let wait = f.sig.asyncness.map(|_| quote!(.await));
    let result = match &f.sig.output {
        ReturnType::Type(_, ty) => {
            matches!(&**ty, Type::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == "Result"))
        }
        _ => false,
    };
    let check = result.then(|| quote!(?));
    quote! {
        #f
        #[allow(unused_variables)]
        pub(crate) async fn #wrapper(app: crate::host::EventHub, core: &std::sync::Arc<crate::Core>, args: serde_json::Value) -> Result<serde_json::Value, String> {
            #(#bindings)*
            let result = #name(#(#params),*) #wait #check;
            serde_json::to_value(result).map_err(|_| "Command result could not be encoded".to_string())
        }
    }.into()
}
