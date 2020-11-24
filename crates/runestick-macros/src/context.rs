use crate::internals::*;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned as _;
use syn::Lit;
use syn::Meta::*;
use syn::NestedMeta::*;

/// Parsed field attributes.
#[derive(Debug, Default)]
pub(crate) struct FieldAttrs {
    /// `#[rune(get)]` to generate a getter.
    pub(crate) getter: bool,
    /// `#[rune(set)]` to generate a setter.
    pub(crate) setter: bool,
    /// `#[rune(copy)]` to indicate that a field is copy and does not need to be
    /// cloned.
    pub(crate) copy: bool,
}

/// Parsed field attributes.
#[derive(Default)]
pub(crate) struct DeriveAttrs {
    /// `#[rune(name = "TypeName")]` to override the default type name.
    pub(crate) name: Option<syn::LitStr>,
}

pub(crate) struct Context {
    pub(crate) any: TokenStream,
    pub(crate) context_error: TokenStream,
    pub(crate) errors: Vec<syn::Error>,
    pub(crate) from_value: TokenStream,
    pub(crate) hash: TokenStream,
    pub(crate) module: TokenStream,
    pub(crate) named: TokenStream,
    pub(crate) object: TokenStream,
    pub(crate) pointer_guard: TokenStream,
    pub(crate) raw_into_mut: TokenStream,
    pub(crate) raw_into_ref: TokenStream,
    pub(crate) raw_str: TokenStream,
    pub(crate) shared: TokenStream,
    pub(crate) to_value: TokenStream,
    pub(crate) tuple: TokenStream,
    pub(crate) type_info: TokenStream,
    pub(crate) type_of: TokenStream,
    pub(crate) unit_struct: TokenStream,
    pub(crate) unsafe_from_value: TokenStream,
    pub(crate) unsafe_to_value: TokenStream,
    pub(crate) value: TokenStream,
    pub(crate) vm_error_kind: TokenStream,
    pub(crate) vm_error: TokenStream,
    pub(crate) install_into: TokenStream,
}

impl Context {
    /// Construct a new context.
    pub fn new() -> Self {
        Self::with_module(RUNESTICK)
    }

    /// Construct a new context.
    pub fn with_module<M>(module: M) -> Self
    where
        M: Copy + ToTokens,
    {
        Self {
            any: quote!(#module::Any),
            context_error: quote!(#module::ContextError),
            errors: Vec::new(),
            from_value: quote!(#module::FromValue),
            hash: quote!(#module::Hash),
            module: quote!(#module::Module),
            named: quote!(#module::Named),
            object: quote!(#module::Object),
            pointer_guard: quote!(#module::SharedPointerGuard),
            raw_into_mut: quote!(#module::RawMut),
            raw_into_ref: quote!(#module::RawRef),
            raw_str: quote!(#module::RawStr),
            shared: quote!(#module::Shared),
            to_value: quote!(#module::ToValue),
            tuple: quote!(#module::Tuple),
            type_info: quote!(#module::TypeInfo),
            type_of: quote!(#module::TypeOf),
            unit_struct: quote!(#module::UnitStruct),
            unsafe_from_value: quote!(#module::UnsafeFromValue),
            unsafe_to_value: quote!(#module::UnsafeToValue),
            value: quote!(#module::Value),
            vm_error_kind: quote!(#module::VmErrorKind),
            vm_error: quote!(#module::VmError),
            install_into: quote!(#module::InstallInto),
        }
    }

    /// Parse the toplevel component of the attribute, which must be `#[rune(..)]`.
    fn get_rune_meta_items(&mut self, attr: &syn::Attribute) -> Option<Vec<syn::NestedMeta>> {
        if attr.path != RUNE {
            return Some(Vec::new());
        }

        match attr.parse_meta() {
            Ok(List(meta)) => Some(meta.nested.into_iter().collect()),
            Ok(other) => {
                self.errors
                    .push(syn::Error::new_spanned(other, "expected #[rune(...)]"));
                None
            }
            Err(error) => {
                self.errors.push(syn::Error::new(Span::call_site(), error));
                None
            }
        }
    }

    /// Parse field attributes.
    pub(crate) fn parse_field_attrs(&mut self, attrs: &[syn::Attribute]) -> Option<FieldAttrs> {
        let mut output = FieldAttrs::default();

        for attr in attrs {
            for meta in self.get_rune_meta_items(attr)? {
                match meta {
                    Meta(Path(path)) if path == GET => {
                        output.getter = true;
                    }
                    Meta(Path(path)) if path == SET => {
                        output.setter = true;
                    }
                    Meta(Path(path)) if path == COPY => {
                        output.copy = true;
                    }
                    _ => {
                        self.errors
                            .push(syn::Error::new_spanned(meta, "unsupported attribute"));

                        return None;
                    }
                }
            }
        }

        Some(output)
    }

    /// Parse field attributes.
    pub(crate) fn parse_derive_attrs(&mut self, attrs: &[syn::Attribute]) -> Option<DeriveAttrs> {
        let mut output = DeriveAttrs::default();

        for attr in attrs {
            for meta in self.get_rune_meta_items(attr)? {
                match meta {
                    // Parse `#[rune(name = "..")]`.
                    Meta(NameValue(syn::MetaNameValue {
                        path,
                        lit: Lit::Str(name),
                        ..
                    })) if path == NAME => {
                        output.name = Some(name);
                    }
                    meta => {
                        self.errors
                            .push(syn::Error::new_spanned(meta, "unsupported attribute"));

                        return None;
                    }
                }
            }
        }

        Some(output)
    }

    /// Expannd the install into impl.
    pub(crate) fn expand_install_into(&mut self, input: &syn::DeriveInput) -> Option<TokenStream> {
        let mut installers = Vec::new();

        let ident = &input.ident;

        match &input.data {
            syn::Data::Struct(st) => {
                for field in &st.fields {
                    let attrs = self.parse_field_attrs(&field.attrs)?;

                    let field_ident = match &field.ident {
                        Some(ident) => ident,
                        None => {
                            if attrs.getter || attrs.setter {
                                self.errors.push(syn::Error::new_spanned(
                                    field,
                                    "only named fields can be used with `#[rune(get)]`",
                                ));
                                return None;
                            }

                            continue;
                        }
                    };

                    let field_ty = &field.ty;
                    let name = &syn::LitStr::new(&field_ident.to_string(), field_ident.span());

                    if attrs.getter {
                        let access = if attrs.copy {
                            quote!(s.#field_ident)
                        } else {
                            quote!(Clone::clone(&s.#field_ident))
                        };

                        installers.push(quote_spanned! { field.span() =>
                            module.getter(#name, |s: &#ident| #access)?;
                        });
                    }

                    if attrs.setter {
                        installers.push(quote_spanned! { field.span() =>
                            module.setter(#name, |s: &mut #ident, value: #field_ty| {
                                s.#field_ident = value;
                            })?;
                        });
                    }
                }
            }
            syn::Data::Enum(..) => {
                self.errors.push(syn::Error::new_spanned(
                    input,
                    "`Any` not supported on enums",
                ));
                return None;
            }
            syn::Data::Union(..) => {
                self.errors.push(syn::Error::new_spanned(
                    input,
                    "`Any` not supported on unions",
                ));
                return None;
            }
        }

        Some(quote! {
            #(#installers)*
            Ok(())
        })
    }

    /// Expand the necessary implementation details for `Any`.
    pub(super) fn expand_any<T>(
        &self,
        ident: T,
        name: &TokenStream,
        install_into: &TokenStream,
    ) -> Result<TokenStream, Vec<syn::Error>>
    where
        T: Copy + ToTokens,
    {
        let any = &self.any;
        let context_error = &self.context_error;
        let hash = &self.hash;
        let module = &self.module;
        let named = &self.named;
        let pointer_guard = &self.pointer_guard;
        let raw_into_mut = &self.raw_into_mut;
        let raw_into_ref = &self.raw_into_ref;
        let raw_str = &self.raw_str;
        let shared = &self.shared;
        let type_info = &self.type_info;
        let type_of = &self.type_of;
        let unsafe_from_value = &self.unsafe_from_value;
        let unsafe_to_value = &self.unsafe_to_value;
        let value = &self.value;
        let vm_error = &self.vm_error;
        let install_into_trait = &self.install_into;

        Ok(quote! {
            impl #any for #ident {
                fn type_hash() -> #hash {
                    // Safety: `Hash` asserts that it is layout compatible with `TypeId`.
                    // TODO: remove this once we can have transmute-like functionality in a const fn.
                    #hash::from_type_id(std::any::TypeId::of::<#ident>())
                }
            }

            impl #install_into_trait for #ident {
                fn install_into(module: &mut #module) -> Result<(), #context_error> {
                    #install_into
                }
            }

            impl #named for #ident {
                const NAME: #raw_str = #raw_str::from_str(#name);
            }

            impl #type_of for #ident {
                fn type_hash() -> #hash {
                    <Self as #any>::type_hash()
                }

                fn type_info() -> #type_info {
                    #type_info::Any(<Self as #named>::NAME)
                }
            }

            impl #unsafe_from_value for &#ident {
                type Output = *const #ident;
                type Guard = #raw_into_ref;

                fn from_value(
                    value: #value,
                ) -> Result<(Self::Output, Self::Guard), #vm_error> {
                    Ok(value.into_any_ptr()?)
                }

                unsafe fn unsafe_coerce(output: Self::Output) -> Self {
                    &*output
                }
            }

            impl #unsafe_from_value for &mut #ident {
                type Output = *mut #ident;
                type Guard = #raw_into_mut;

                fn from_value(
                    value: #value,
                ) -> Result<(Self::Output, Self::Guard), #vm_error> {
                    Ok(value.into_any_mut()?)
                }

                unsafe fn unsafe_coerce(output: Self::Output) -> Self {
                    &mut *output
                }
            }

            impl #unsafe_to_value for &#ident {
                type Guard = #pointer_guard;

                unsafe fn unsafe_to_value(self) -> Result<(#value, Self::Guard), #vm_error> {
                    let (shared, guard) = #shared::from_ref(self);
                    Ok((#value::from(shared), guard))
                }
            }

            impl #unsafe_to_value for &mut #ident {
                type Guard = #pointer_guard;

                unsafe fn unsafe_to_value(self) -> Result<(#value, Self::Guard), #vm_error> {
                    let (shared, guard) = #shared::from_mut(self);
                    Ok((#value::from(shared), guard))
                }
            }
        })
    }
}
