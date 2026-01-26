use std::collections::HashSet;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Variant, Generics, Type, PathArguments, Ident, WherePredicate, AngleBracketedGenericArguments, GenericArgument};

// TODO intersection

#[derive(Debug)]
pub struct FilteredGenerics {
	pub impl_generics: Option<TokenStream2>,
	pub ty_generics: Option<TokenStream2>,
	pub where_clause: Option<TokenStream2>,
}
impl FilteredGenerics {
	pub fn from_enum_variant(variant: &Variant, enum_generics: &Generics) -> Self {
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

		Self {
			impl_generics: (!impl_generics.is_empty()).then_some(quote!(< #(#impl_generics),* >)),
			ty_generics: (!ty_generics.is_empty()).then_some(quote!(< #(#ty_generics),* >)),
			where_clause,
		}
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
