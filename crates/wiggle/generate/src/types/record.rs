use crate::lifetimes::{LifetimeExt, anon_lifetime};
use crate::{Context, names};

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use witx::Layout;

pub(super) fn define_struct(
    ctx: &Context,
    name: &witx::Id,
    s: &witx::RecordDatatype,
) -> TokenStream {
    let ident = names::type_(name);
    let size = s.mem_size_align().size;
    let align = s.mem_size_align().align;
    let width = &ctx.width;
    let member_names = s.members.iter().map(|m| names::struct_member(&m.name));
    let member_decls = s.members.iter().map(|m| {
        let name = names::struct_member(&m.name);
        let type_ = match &m.tref {
            witx::TypeRef::Name(nt) => {
                let tt = names::type_(&nt.name);
                quote!(#tt)
            }
            witx::TypeRef::Value(ty) => match &**ty {
                witx::Type::Builtin(builtin) => names::builtin_type(*builtin),
                witx::Type::Pointer(pointee) | witx::Type::ConstPointer(pointee) => {
                    let pointee_type = names::type_ref(ctx, &pointee, quote!('a));
                    quote!(wiggle::GuestPtr<#pointee_type, #width>)
                }
                _ => unimplemented!("other anonymous struct members: {:?}", m.tref),
            },
        };
        quote!(pub #name: #type_)
    });

    let member_offsets = s.member_layout().into_iter().map(|ml| {
        let name = names::struct_member(&ml.member.name);
        let offset = ml.offset;
        let method_name = format_ident!("offset_of_{}", name);
        quote! {
            pub const fn #method_name () -> #width {
                #offset as #width
            }
        }
    });

    let member_reads = s.member_layout().into_iter().map(|ml| {
        let name = names::struct_member(&ml.member.name);
        let offset = ml.offset;
        let location = quote! {
            location.cast::<u8>().add(#offset.try_into()?)?.cast()
        };
        match &ml.member.tref {
            witx::TypeRef::Name(nt) => {
                let type_ = names::type_(&nt.name);
                quote! {
                    let #name = <#type_ as wiggle::GuestType<#width>>::read(mem, #location)?;
                }
            }
            witx::TypeRef::Value(ty) => match &**ty {
                witx::Type::Builtin(builtin) => {
                    let type_ = names::builtin_type(*builtin);
                    quote! {
                        let #name = <#type_ as wiggle::GuestType<#width>>::read(mem, #location)?;
                    }
                }
                witx::Type::Pointer(pointee) | witx::Type::ConstPointer(pointee) => {
                    let pointee_type = names::type_ref(ctx, &pointee, anon_lifetime());
                    quote! {
                        let #name = <wiggle::GuestPtr::<#pointee_type, #width> as wiggle::GuestType<#width>>::read(mem, #location)?;
                    }
                }
                _ => unimplemented!("other anonymous struct members: {:?}", ty),
            },
        }
    });

    let member_writes = s.member_layout().into_iter().map(|ml| {
        let name = names::struct_member(&ml.member.name);
        let offset = ml.offset;
        quote! {
            wiggle::GuestType::write(
                mem,
                location.cast::<u8>().add(#offset.try_into()?)?.cast(),
                val.#name,
            )?;
        }
    });

    let (struct_lifetime, extra_derive) = if s.needs_lifetime() {
        (quote!(<'a>), quote!())
    } else {
        (quote!(), quote!(, PartialEq))
    };

    quote! {
        #[derive(Clone, Debug #extra_derive)]
        pub struct #ident {
            #(#member_decls),*
        }

        impl #struct_lifetime #ident {
            #(#member_offsets)*
        }

        impl wiggle::GuestType<#width> for #ident {
            #[inline]
            fn guest_size() -> #width {
                #size.try_into().unwrap()
            }

            #[inline]
            fn guest_align() -> usize {
                #align
            }

            fn read(
                mem: &wiggle::GuestMemory,
                location: wiggle::GuestPtr<Self, #width>,
            ) -> Result<Self, wiggle::GuestError<#width>> {
                #(#member_reads)*
                Ok(#ident { #(#member_names),* })
            }

            fn write(
                mem: &mut wiggle::GuestMemory,
                location: wiggle::GuestPtr<Self, #width>,
                val: Self,
            ) -> Result<(), wiggle::GuestError<#width>> {
                #(#member_writes)*
                Ok(())
            }
        }
    }
}

impl super::WiggleType for witx::RecordDatatype {
    fn impls_display(&self) -> bool {
        false
    }
}
