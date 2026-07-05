use crate::{attr, bound};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, Data, DataEnum, DeriveInput, Error, Fields, FieldsNamed, FieldsUnnamed, Result,
};

pub fn derive(input: DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Struct(data) => match attr::struct_kind(&input.attrs, data)? {
            attr::StructKind::Transparent(fields) => derive_transparent_struct(&input, fields),
            attr::StructKind::Named(fields) => derive_struct(&input, fields),
        },
        Data::Enum(enumeration) => derive_enum(&input, enumeration),
        _ => Err(Error::new(Span::call_site(), "unsupported derive input")),
    }
}

fn derive_transparent_struct(input: &DeriveInput, fields: &FieldsUnnamed) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, _) = input.generics.split_for_impl();
    let fieldty = &fields.unnamed[0].ty;
    let bound = parse_quote!(microserde::Deserialize);
    let bounded_where_clause = bound::where_clause_with_bound(&input.generics, bound);

    Ok(quote! {
        impl #impl_generics microserde::Deserialize for #ident #ty_generics #bounded_where_clause {
            type Visitor<'__a>
                = microserde::de::TransparentVisitor<'__a, Self>
            where
                Self: '__a;

            fn begin(__out: &mut microserde::export::Option<Self>) -> Self::Visitor<'_> {
                microserde::de::transparent(__out)
            }
        }

        impl #impl_generics microserde::de::Transparent for #ident #ty_generics #bounded_where_clause {
            type Inner = #fieldty;

            fn wrap(__value: Self::Inner) -> Self {
                #ident(__value)
            }
        }
    })
}

pub fn derive_struct(input: &DeriveInput, fields: &FieldsNamed) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let fieldname = fields.named.iter().map(|f| &f.ident).collect::<Vec<_>>();
    let fieldty = fields.named.iter().map(|f| &f.ty).collect::<Vec<_>>();
    let fieldattrs = fields
        .named
        .iter()
        .map(|f| attr::field_attrs(&f.attrs))
        .collect::<Result<Vec<_>>>()?;
    let fieldstr = fields
        .named
        .iter()
        .zip(&fieldattrs)
        .map(|(field, attrs)| {
            Ok(attrs
                .rename
                .clone()
                .unwrap_or_else(|| field.ident.as_ref().unwrap().to_string()))
        })
        .collect::<Result<Vec<_>>>()?;
    let fielddefault = fields
        .named
        .iter()
        .zip(&fieldattrs)
        .map(|(field, attrs)| {
            if attrs.default {
                let ty = &field.ty;
                quote!(microserde::export::Some(<#ty as microserde::export::Default>::default()))
            } else {
                let ty = &field.ty;
                quote!(<#ty as microserde::Deserialize>::default())
            }
        })
        .collect::<Vec<_>>();

    let wrapper_generics = bound::with_lifetime_bound(&input.generics, "'__a");
    let (wrapper_impl_generics, wrapper_ty_generics, _) = wrapper_generics.split_for_impl();
    let bound = parse_quote!(microserde::Deserialize);
    let bounded_where_clause = bound::where_clause_with_bound(&input.generics, bound);

    Ok(quote! {
        #[allow(non_local_definitions)]
        const _: () = {
            // Public to satisfy E0446 when the derived type is public, since
            // this type is named by the `Deserialize::Visitor` associated
            // type; the anonymous const keeps it unnameable from outside.
            pub struct __Visitor #wrapper_impl_generics #where_clause {
                __out: &'__a mut microserde::export::Option<#ident #ty_generics>,
            }

            impl #impl_generics microserde::Deserialize for #ident #ty_generics #bounded_where_clause {
                type Visitor<'__a>
                    = __Visitor #wrapper_ty_generics
                where
                    Self: '__a;

                fn begin(__out: &mut microserde::export::Option<Self>) -> Self::Visitor<'_> {
                    __Visitor { __out }
                }
            }

            impl #wrapper_impl_generics microserde::de::Visitor for __Visitor #wrapper_ty_generics #bounded_where_clause {
                fn map<'__s>(self: microserde::export::Box<Self>) -> microserde::Result<microserde::export::Box<dyn microserde::de::Map + '__s>>
                where
                    Self: '__s,
                {
                    microserde::export::Ok(microserde::export::Box::new(__State {
                        #(
                            #fieldname: #fielddefault,
                        )*
                        __out: self.__out,
                    }))
                }
            }

            struct __State #wrapper_impl_generics #where_clause {
                #(
                    #fieldname: microserde::export::Option<#fieldty>,
                )*
                __out: &'__a mut microserde::export::Option<#ident #ty_generics>,
            }

            impl #wrapper_impl_generics microserde::de::Map for __State #wrapper_ty_generics #bounded_where_clause {
                fn key(&mut self, __k: &microserde::export::str) -> microserde::Result<microserde::export::Box<dyn microserde::de::Visitor + '_>> {
                    match __k {
                        #(
                            #fieldstr => microserde::export::Ok(microserde::export::Box::new(<#fieldty as microserde::Deserialize>::begin(&mut self.#fieldname))),
                        )*
                        _ => microserde::export::Ok(<dyn microserde::de::Visitor>::ignore()),
                    }
                }

                fn scalar(&mut self, __k: &microserde::export::str, __s: &microserde::de::Scalar) -> microserde::Result<()> {
                    match __k {
                        #(
                            #fieldstr => microserde::de::Visitor::scalar(
                                &mut <#fieldty as microserde::Deserialize>::begin(&mut self.#fieldname),
                                __s,
                            ),
                        )*
                        _ => microserde::export::Ok(()),
                    }
                }

                fn finish(&mut self) -> microserde::Result<()> {
                    #(
                        let #fieldname = self.#fieldname.take().ok_or(microserde::Error)?;
                    )*
                    *self.__out = microserde::export::Some(#ident {
                        #(
                            #fieldname,
                        )*
                    });
                    microserde::export::Ok(())
                }
            }
        };
    })
}

pub fn derive_enum(input: &DeriveInput, enumeration: &DataEnum) -> Result<TokenStream> {
    if input.generics.lt_token.is_some() || input.generics.where_clause.is_some() {
        return Err(Error::new(
            Span::call_site(),
            "Enums with generics are not supported",
        ));
    }

    let ident = &input.ident;

    let var_idents = enumeration
        .variants
        .iter()
        .map(|variant| match variant.fields {
            Fields::Unit => Ok(&variant.ident),
            _ => Err(Error::new_spanned(
                variant,
                "Invalid variant: only simple enum variants without fields are supported",
            )),
        })
        .collect::<Result<Vec<_>>>()?;
    let names = enumeration
        .variants
        .iter()
        .map(attr::name_of_variant)
        .collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #[allow(non_local_definitions)]
        const _: () = {
            // Public to satisfy E0446 when the derived type is public, since
            // this type is named by the `Deserialize::Visitor` associated
            // type; the anonymous const keeps it unnameable from outside.
            pub struct __Visitor<'__a> {
                __out: &'__a mut microserde::export::Option<#ident>,
            }

            impl microserde::Deserialize for #ident {
                type Visitor<'__a> = __Visitor<'__a>;

                fn begin(__out: &mut microserde::export::Option<Self>) -> Self::Visitor<'_> {
                    __Visitor { __out }
                }
            }

            impl<'__a> microserde::de::Visitor for __Visitor<'__a> {
                fn scalar(&mut self, __s: &microserde::de::Scalar) -> microserde::Result<()> {
                    let microserde::de::Scalar::Str(__s) = __s else {
                        return microserde::export::Err(microserde::Error);
                    };
                    let __value = match *__s {
                        #( #names => #ident::#var_idents, )*
                        _ => { return microserde::export::Err(microserde::Error) },
                    };
                    *self.__out = microserde::export::Some(__value);
                    microserde::export::Ok(())
                }
            }
        };
    })
}
