use crate::versioned::{require_named_attr, take_all_attrs_with_name, to_version_range, Version};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use std::{collections::HashSet, ops::Range};
use syn::{
	parse::ParseStream, parse_quote, spanned::Spanned, AngleBracketedGenericArguments, Attribute,
	Error, ExprRange, Fields, GenericArgument, Generics, Ident, ItemEnum, PathArguments, Result,
	Token, Type, TypeParam, Variant, Visibility, WherePredicate,
};

use super::{find_named_attr, get_version_val};

mod kw {
	syn::custom_keyword!(flatten);
}

pub(crate) struct VersionVariant {
	raw: Variant,
	version: Version,
	variant_struct: VersionVariantStruct,
	next_version_conv: Option<NextVersionConversion>,
}
impl VersionVariant {
	pub fn new(
		toplevel_version_range: &Range<Version>,
		version: Version,
		mut raw: Variant,
		enum_generics: &Generics,
	) -> Result<Self> {
		Self::flatten_versioned_type(version, &mut raw)?;

		let variant_struct = VersionVariantStruct::new(&mut raw, enum_generics)?;

		let variant_version_range =
			Self::get_variant_version_range(&mut raw, toplevel_version_range)?;

		let is_last_toplevel_version = version == toplevel_version_range.end;
		let is_last_variant_version = version == variant_version_range.end;

		let next_version_conv;

		if let Some((removed_since, removed_since_span)) =
			Self::get_removed_since_version(&mut raw)?
		{
			if removed_since != variant_version_range.end {
				return Err(Error::new(
					removed_since_span,
					"removal version must be equal to the next version after the variant's last version",
				));
			}

			next_version_conv = None;
		} else if let Some(superseded_by) = Self::superseded_by_variant(&mut raw)? {
			if superseded_by == raw.ident {
				return Err(Error::new(superseded_by.span(), "`superseded_by` must point to an variant with a different name. You can simply remove this attribute if the variant changed its format in the new version"));
			} else if is_last_toplevel_version {
				return Err(Error::new(superseded_by.span(), "`superseded_by` can't be applied on an variant that exists in the last version (`superseded_by` must point to an instruction in the next version)"));
			}

			next_version_conv = Some(NextVersionConversion::UserConversion(superseded_by));
		} else if is_last_toplevel_version {
			next_version_conv = None;
		} else if is_last_variant_version {
			next_version_conv = Some(NextVersionConversion::UserConversion(raw.ident.clone()));
		} else {
			next_version_conv = Some(NextVersionConversion::AutoConversion);
		}

		Ok(Self { raw, version, variant_struct, next_version_conv })
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

		let meta = removed_attr.meta.require_name_value()?;

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

pub(crate) enum NextVersionConversion {
	AutoConversion,
	UserConversion(Ident),
}

pub(crate) struct VersionVariantStruct {
	attrs: Vec<Attribute>,
	impl_generics: Option<TokenStream2>,
	ty_generics: Option<TokenStream2>,
	where_clause: Option<TokenStream2>,
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

		let variant_ident_types = variant
			.fields
			.iter()
			.map(|field| Self::all_type_idents(&field.ty))
			.flatten()
			.collect::<HashSet<_>>();

		let impl_generics = enum_generics
			.type_params()
			.filter(|param| variant_ident_types.contains(&param.ident))
			.cloned()
			.collect::<Vec<_>>();
		let ty_generics = impl_generics.iter().map(|param| param.ident.clone()).collect::<Vec<_>>();

		let where_clause = if let Some(wc) = &enum_generics.where_clause {
			let predicates = wc
				.predicates
				.iter()
				.filter_map(|p| {
					if let WherePredicate::Type(ty_pred) = p {
						let type_ident = Self::type_ident(&ty_pred.bounded_ty)?;
						ty_generics.contains(&type_ident).then_some(p)
					} else {
						None
					}
				})
				.collect::<Vec<_>>();

			(!predicates.is_empty()).then(|| quote!(where #(#predicates),*))
		} else {
			None
		};

		let ident = variant.ident.clone();
		let mut fields = variant.fields.clone();
		for field in fields.iter_mut() {
			field.vis = Visibility::Public(Default::default());
		}

		Ok(Self {
			attrs,
			impl_generics: (!impl_generics.is_empty()).then_some(quote!(< #(#impl_generics),* >)),
			ty_generics: (!ty_generics.is_empty()).then_some(quote!(< #(#ty_generics),* >)),
			where_clause,
			ident,
			fields,
		})
	}

	pub fn expand_decl(&self) -> TokenStream2 {
		let Self { attrs, impl_generics, ty_generics, where_clause, ident, fields } = self;

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
		let Self { attrs, impl_generics, ty_generics, where_clause, ident, fields } = self;

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

	pub fn generics_for_impl(
		&self,
	) -> (Option<&TokenStream2>, Option<&TokenStream2>, Option<&TokenStream2>) {
		(self.impl_generics.as_ref(), self.ty_generics.as_ref(), self.where_clause.as_ref())
	}

	fn type_ident(ty: &Type) -> Option<Ident> {
		match ty {
			Type::Path(type_path) =>
				if let Some(qself) = &type_path.qself {
					Self::type_ident(&qself.ty)
				} else {
					type_path.path.get_ident().cloned()
				},
			_ => None,
		}
	}

	fn all_type_idents(ty: &Type) -> impl Iterator<Item = Ident> {
		match ty {
			Type::Path(type_path) =>
				if let Some(qself) = &type_path.qself {
					Self::all_type_idents(&qself.ty)
				} else {
					if let Some(type_ident) = type_path.path.get_ident().cloned() {
						return vec![type_ident].into_iter();
					}

					let Some(last_seg) = type_path.path.segments.last() else {
						return vec![].into_iter();
					};

					let mut idents = vec![];
					if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
						args,
						..
					}) = &last_seg.arguments
					{
						let generics_types = args.iter().filter_map(|arg| {
							if let GenericArgument::Type(ty) = arg {
								Some(ty)
							} else {
								None
							}
						});

						for ty in generics_types {
							idents.extend(Self::all_type_idents(ty));
						}
					}

					idents.into_iter()
				},
			_ => vec![].into_iter(),
		}
	}
}
