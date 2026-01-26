use crate::versioned::{require_named_attr, take_all_attrs_with_name, to_version_range, Version};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::{collections::HashSet, ops::Range};
use syn::{
	parse::ParseStream, parse_quote, spanned::Spanned, AngleBracketedGenericArguments, Attribute,
	Error, ExprRange, Fields, GenericArgument, Generics, Ident, ItemEnum, PathArguments, Result,
	Token, Type, TypeParam, Variant, Visibility, WherePredicate, MetaNameValue,
};

use super::{find_named_attr, get_version_val, FilteredGenerics};

mod kw {
	syn::custom_keyword!(flatten);
}

#[derive(Debug)]
pub(crate) struct VersionVariant {
	raw: Variant,
	version: Version, // FIXME remove this
	variant_struct: VersionVariantStruct,
	next_version_conv: Option<NextVersionConversion>,
}
impl VersionVariant {
	pub fn from(
		toplevel_version_range: &Range<Version>,
		version: Version,
		mut raw: Variant,
		enum_generics: &Generics,
	) -> Result<Option<Self>> {
		Self::flatten_versioned_type(version, &mut raw)?;

		let variant_struct = VersionVariantStruct::new(&mut raw, enum_generics)?;

		let variant_version_range =
			Self::get_variant_version_range(&mut raw, toplevel_version_range)?;

		if version < variant_version_range.start || version >= variant_version_range.end {
			return Ok(None);
		}

		let next_version = version + 1;
		let is_last_toplevel_version = next_version == toplevel_version_range.end;
		let is_last_variant_version = next_version == variant_version_range.end;

		let mut next_version_conv = (!is_last_toplevel_version).then_some(NextVersionConversion::AutoConversion);

		if let Some((removed_since, removed_since_span)) =
			Self::get_removed_since_version(&mut raw)?
		{
			if removed_since != variant_version_range.end {
				return Err(Error::new(
					removed_since_span,
					"removal version must be equal to the next version after the variant's last version",
				));
			}

			if version >= removed_since {
				return Ok(None);
			} else if is_last_variant_version {
				next_version_conv = None;
			}
		} else if let Some(superseded_by) = Self::superseded_by_variant(&mut raw)? {
			if superseded_by == raw.ident {
				return Err(Error::new(superseded_by.span(), "`superseded_by` must point to an variant with a different name. You can simply remove this attribute if the variant changed its format in the new version"));
			} else if is_last_toplevel_version {
				return Err(Error::new(superseded_by.span(), "`superseded_by` can't be applied on an variant that exists in the last version (`superseded_by` must point to an instruction in the next version)"));
			}

			next_version_conv = Some(NextVersionConversion::UserConversion(superseded_by));
		}

		if is_last_variant_version && next_version_conv == Some(NextVersionConversion::AutoConversion) {
			next_version_conv = Some(NextVersionConversion::UserConversion(raw.ident.clone()));
		}

		Ok(Some(Self { raw, version, variant_struct, next_version_conv }))
	}

	pub fn next_version_conversion(&self) -> Option<&NextVersionConversion> {
		self.next_version_conv.as_ref()
	}

	pub fn inner(&self) -> &Variant {
		&self.raw
	}

	pub fn as_struct(&self) -> &VersionVariantStruct {
		&self.variant_struct
	}

	fn get_variant_version_range(
		raw: &mut Variant,
		toplevel_version_range: &Range<Version>,
	) -> Result<Range<Version>> {
		let notfound_err_span = raw.ident.span();
		let version_attr = require_named_attr("version", &mut raw.attrs, notfound_err_span)?;

		version_attr.parse_args_with(|input: ParseStream| -> Result<Range<Version>> {
			let range = input.parse::<ExprRange>()?;
			let (start, end) = to_version_range(&range)?;

			if start < toplevel_version_range.start {
				let range_start_span =
					range.start.expect("to_version_range guarantees start exists; qed").span();
				return Err(Error::new(
					range_start_span,
					"the variant start version must be greater or equal to the top-level start version",
				));
			}

			if let Some(end) = end {
				if end <= toplevel_version_range.end {
					Ok(start..end)
				} else {
					Err(Error::new(
						range.end.span(),
						format![
							"the end version {} is greater than the max version {}",
							end - 1,
							toplevel_version_range.end - 1
						],
					))
				}
			} else {
				Ok(start..toplevel_version_range.end)
			}
		})
	}

	fn get_removed_since_version(raw: &mut Variant) -> Result<Option<(Version, Span)>> {
		let Some(removed_attr) = find_named_attr("removed", &mut raw.attrs)? else {
			return Ok(None);
		};

		let meta = removed_attr.meta.require_list()?;
		let meta: MetaNameValue = meta.parse_args()?;

		if !meta.path.is_ident("since") {
			return Err(Error::new(meta.path.span(), "the `since` parameter is expected"));
		}

		let removed_since = get_version_val(&meta.value)?;

		Ok(Some((removed_since, meta.value.span())))
	}

	fn superseded_by_variant(raw: &mut Variant) -> Result<Option<Ident>> {
		let Some(superseded_by_attr) = find_named_attr("superseded_by", &mut raw.attrs)? else {
			return Ok(None);
		};

		let meta = superseded_by_attr.meta.require_list()?;
		let ident = meta.parse_args::<Ident>()?;
		Ok(Some(ident))
	}

	fn flatten_versioned_type(version: Version, variant: &mut Variant) -> Result<()> {
		for field in variant.fields.iter_mut() {
			if Self::contains_version_flatten(&mut field.attrs)? {
				let ty = &field.ty;
				field.ty = syn::parse2(quote!( < #ty as Versioned< #version > >::Type ))?;
			}
		}

		Ok(())
	}

	fn contains_version_flatten(attrs: &mut Vec<Attribute>) -> Result<bool> {
		let Some(version_attr) = find_named_attr("version", attrs)? else {
			return Ok(false);
		};

		version_attr.parse_args::<kw::flatten>().map(|_| true)
	}
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NextVersionConversion {
	AutoConversion,
	UserConversion(Ident),
}

#[derive(Debug)]
pub(crate) struct VersionVariantStruct {
	attrs: Vec<Attribute>,
	filtered_generics: FilteredGenerics,
	ident: Ident,
	fields: Fields,
}
impl VersionVariantStruct {
	fn new(variant: &mut Variant, enum_generics: &Generics) -> Result<Self> {
		let attrs = take_all_attrs_with_name("for_struct", &mut variant.attrs)
			.iter()
			.map(|for_struct| {
				let attrs = &for_struct.meta.require_list()?.tokens;
				Ok(parse_quote!(#[ #attrs ]))
			})
			.collect::<Result<Vec<_>>>()?;

		let filtered_generics = FilteredGenerics::from_enum_variant(variant, enum_generics);

		let ident = variant.ident.clone();
		let mut fields = variant.fields.clone();
		for field in fields.iter_mut() {
			field.vis = Visibility::Public(Default::default());
		}

		Ok(Self {
			attrs,
			filtered_generics,
			ident,
			fields,
		})
	}

	pub fn generics(&self) -> &FilteredGenerics {
		&self.filtered_generics
	}

	pub fn expand_decl(&self) -> TokenStream2 {
		let Self { attrs, filtered_generics, ident, fields } = self;
		let FilteredGenerics { impl_generics, ty_generics, where_clause } = filtered_generics;

		let named_fields = matches!(&fields, Fields::Named(_));
		let end_semicolon = (!named_fields).then_some(<Token![;]>::default());

		let new_impl = {
			let types = fields.iter().map(|field| field.ty.clone());

			let (arg_decls, struct_val) = if named_fields {
				let members = fields.members();
				let decls = members
					.clone()
					.zip(types)
					.map(|(member, ty)| quote!(#member: #ty))
					.collect::<Vec<_>>();

				(decls, quote!(Self { #(#members),* }))
			} else {
				let members =
					fields.members().enumerate().map(|(idx, _member)| format_ident!("arg{idx}"));
				let decls = members
					.clone()
					.zip(types)
					.map(|(member, ty)| quote!(#member: #ty))
					.collect::<Vec<_>>();

				(decls, quote!(Self(#(#members),*)))
			};

			quote! {
				impl #impl_generics #ident #ty_generics #where_clause {
					pub fn new(#(#arg_decls),*) -> Self {
						#struct_val
					}
				}
			}
		};

		quote! {
			#(#attrs)*
			pub struct #ident #impl_generics #fields #where_clause #end_semicolon

			#new_impl
		}
	}

	pub fn expand_auto_conversions(
		&self,
		current_version: Version,
		next_version: Version,
		next_version_conv: Option<&NextVersionConversion>,
	) -> TokenStream2 {
		let Self { attrs, filtered_generics, ident, fields } = self;
		let FilteredGenerics { impl_generics, ty_generics, where_clause } = filtered_generics;

		if let Some(NextVersionConversion::AutoConversion) = next_version_conv {
			let current_version_mod = format_ident!["v{current_version}"];
			let next_version_mod = format_ident!["v{next_version}"];

			let current_version_struct = quote!(#current_version_mod :: #ident #ty_generics);
			let next_version_struct = quote!(#next_version_mod :: #ident #ty_generics);

			let members = fields.members();
			let assignments =
				members.map(|member| quote!(from_val.#member.try_into().map_err(|_| ())?));
			let struct_val = quote! {
				Self::new(#(#assignments),*)
			};

			quote! {
				impl #impl_generics TryFrom<#current_version_struct> for #next_version_struct #where_clause {
					type Error = ();

					fn try_from(from_val: #current_version_struct) -> Result<Self, Self::Error> {
						Ok(#struct_val)
					}
				}

				impl #impl_generics TryFrom<#next_version_struct> for #current_version_struct #where_clause {
					type Error = ();

					fn try_from(from_val: #next_version_struct) -> Result<Self, Self::Error> {
						Ok(#struct_val)
					}
				}
			}
		} else {
			quote!()
		}
	}
}
