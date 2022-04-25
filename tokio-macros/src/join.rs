use proc_macro::TokenStream;
use quote::quote;
use std::fmt::Write;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Token};

#[doc(hidden)]
struct Join {
    fut_exprs: Vec<Expr>,
}

impl Parse for Join {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut exprs = Vec::new();

        while !input.is_empty() {
            exprs.push(input.parse::<Expr>()?);

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Join { fut_exprs: exprs })
    }
}

fn destructure_nth_tuple_element(n: usize) -> String {
    return String::from("1;");
    let mut buffer = String::new();

    write!(&mut buffer, "let (fut, .. ) = &mut futures;").unwrap();

    // for _ in 0..n {
    //   write!(&mut buffer, "_,").unwrap();
    // }

    // write!(&mut buffer, "fut, ..) = &mut futures;").unwrap();

    buffer
}

pub(crate) fn join(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as Join);

    let futures_count = parsed.fut_exprs.len();

    let match_statement_branches = (0..futures_count).map(|i| {
        let pos = syn::Index::from(i);

        let get_tuple_element = destructure_nth_tuple_element(i);

        quote! {
            #pos => {
                // #get_tuple_element

                // Safety: future is stored on the stack above
                // and never moved.
                let mut fut = unsafe { Pin::new_unchecked(fut) };

                // Try polling
                if fut.poll(cx).is_pending() {
                    is_pending = true;
                }
            }
        }
    });

    let ready_output = (0..futures_count).map(|i| {
        let i = syn::Index::from(i);

        quote! {{
            let fut = &mut futures.#i;

            // Safety: future is stored on the stack above
            // and never moved.
            let mut fut = unsafe { Pin::new_unchecked(fut) };

            fut.take_output().expect("expected completed future")
        }}
    });

    let futures = parsed.fut_exprs.into_iter();

    TokenStream::from(quote! {{
      use tokio::macros::support::{maybe_done, poll_fn, Future, Pin};
      use tokio::macros::support::Poll::{Ready, Pending};

      // Safety: nothing must be moved out of `futures`. This is to satisfy
      // the requirement of `Pin::new_unchecked` called below.
      // let mut futures = #futures;
      let mut futures = ( #( maybe_done(#futures), )* );

      // When poll_fn is polled, start polling the future at this index.
      let mut start_index = 0;

      poll_fn(move |cx| {
          let mut is_pending = false;

          for i in 0..#futures_count {
              let turn;

              #[allow(clippy::modulo_one)]
              {
                  turn = (start_index + i) % #futures_count
              };

              match turn {
                #( #match_statement_branches, )*
                _ => unreachable!("reaching this means there probably is an off by one bug")
              }
          }

          if is_pending {
              // Start by polling the next future first the next time poll_fn is polled
              #[allow(clippy::modulo_one)]
              {
                  start_index = (start_index + 1) % #futures_count;
              }

              Pending
          } else {
             Ready( ( #( #ready_output, )* ) )
          }
      }).await
    }})
}
