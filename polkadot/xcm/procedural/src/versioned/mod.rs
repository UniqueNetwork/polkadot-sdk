use proc_macro2::{Group, Literal, Span, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote, ToTokens};
use std::{collections::BTreeMap, ops::Range};
use syn::{
	parse::ParseStream, parse_macro_input, parse_quote, spanned::Spanned, Attribute, Error, Expr,
	ExprLit, ExprRange, Fields, GenericParam, Generics, Ident, ItemEnum, Lit, LitInt, Meta,
	MetaList, MetaNameValue, RangeLimits, Result, Token, Type, TypeParam, Variant, Visibility,
	WherePredicate,
};

mod variant;
mod version_enum;
mod filtered_generics;

pub use variant::*;
pub use version_enum::*;
pub use filtered_generics::*;

pub(crate) type Version = u32;

pub fn versioned(item_enum: ItemEnum) -> Result<TokenStream2> {
	Versioned::new(item_enum)?.expand()
}

pub(crate) struct Versioned {
	vis: Visibility,
	ident: Ident,

	versioned_enum_attrs: Vec<Attribute>,
	versioned_enum_variants_attrs_tokens: Vec<TokenStream2>,

	version_enum_generics: Generics,
	version_enum_attrs: Vec<Attribute>,
	versions: BTreeMap<Version, Vec<VersionVariant>>,
}
impl Versioned {
	pub fn new(mut item_enum: ItemEnum) -> Result<Self> {
		let toplevel_version_range = Self::get_toplevel_version_range(&mut item_enum)?;

		let mut version_enum_attrs = vec![];
		let mut versioned_enum_attrs = vec![];
		let mut versioned_enum_variants_attrs_tokens = vec![];

		for attr in item_enum.attrs.drain(..) {
			let path = attr.path();
			let meta = &attr.meta;

			if path.is_ident("versioned_enum") {
				let list = meta.require_list()?;
				let attr_tokens = &list.tokens;

				versioned_enum_attrs.push(parse_quote!(#[ #attr_tokens ]));
			} else if path.is_ident("versioned_enum_variants") {
				let list = meta.require_list()?;
				versioned_enum_variants_attrs_tokens.push(list.tokens.clone());
			} else {
				version_enum_attrs.push(attr);
			}
		}

		let mut versions = BTreeMap::new();

		for version in toplevel_version_range.clone() {
			let variants = item_enum
				.variants
				.iter()
				.cloned()
				.filter_map(|variant| {
					VersionVariant::from(
						&toplevel_version_range,
						version,
						variant,
						&item_enum.generics,
					).transpose()
				})
				.collect::<Result<Vec<_>>>()?;

			versions.insert(version, variants);
		}

		Ok(Self {
			vis: item_enum.vis,
			ident: item_enum.ident,

			versioned_enum_attrs,
			versioned_enum_variants_attrs_tokens,

			version_enum_generics: item_enum.generics,
			version_enum_attrs,
			versions,
		})
	}

	pub fn expand(self) -> Result<TokenStream2> {
		let version_pairs = || {
			let mut iter = self.versions.iter();
			let mut next = iter.next();

			core::iter::from_fn(move || {
				let current = next.take();
				next = iter.next();

				Some((current?, next?))
			})
		};

		let mut result = TokenStream2::new();

		let version_mod = |version| format_ident!["v{version}"];
		let version_variant = |version| format_ident!["V{version}"];

		let Self {
			vis,
			ident,

			versioned_enum_attrs,
			versioned_enum_variants_attrs_tokens,

			version_enum_generics,
			version_enum_attrs,
			versions,
		} = &self;

		let (impl_generics, ty_generics, where_clause) = version_enum_generics.split_for_impl();

		for (version, version_variants) in self.versions.iter() {
			let version_mod = version_mod(*version);
			let variants = version_variants.iter().map(|v| v.inner());
			let variants_as_structs = version_variants.iter().map(|v| v.as_struct().expand_decl());

			let impl_from_traits = version_variants.iter().map(|v| {
				let variant_ident = &v.inner().ident;
				let (pattern, fields) = Self::variant_pattern_and_fields(v.inner());
				let fields = if matches![&v.inner().fields, Fields::Named(_)] {
					quote!({ #fields })
				} else {
					quote!(( #fields ))
				};

				let as_struct = v.as_struct();
				let struct_ty_generics = as_struct.generics().ty_generics.as_ref();

				quote! {
					impl #impl_generics From<#variant_ident #struct_ty_generics> for #ident #ty_generics #where_clause {
						fn from(variant: #variant_ident #struct_ty_generics) -> Self {
							let #pattern = variant;
							Self :: #variant_ident #fields
						}
					}
				}
			});

			let version_tokens = quote! {
				#vis mod #version_mod {
					use super::*;

					#(#variants_as_structs)*

					#(#version_enum_attrs)*
					pub enum #ident #version_enum_generics {
						#(#variants),*
					}

					#(#impl_from_traits)*
				}
			};

			result.extend(version_tokens);
		}

		for ((current_version, current_variants), (next_version, next_variants)) in version_pairs()
		{
			let current_version_mod_ident = version_mod(*current_version);
			let next_version_mod_ident = version_mod(*next_version);

			let mut current2next_arms = vec![];
			let mut next2current_arms = vec![];

			for variant in current_variants.iter() {
				if let Some(next_version_variant) =
					Self::get_next_version_variant(&variant, next_variants)?
				{
					let current_variant = &variant.inner().ident;
					let next_variant = &next_version_variant.inner().ident;

					let (current_pattern, current_fields) =
						Self::variant_pattern_and_fields(variant.inner());
					let (next_pattern, next_fields) =
						Self::variant_pattern_and_fields(next_version_variant.inner());

					current2next_arms.push(quote! {
						#current_version_mod_ident :: #ident :: #current_pattern => {
							let current = #current_version_mod_ident :: #current_variant :: new(#current_fields);
							let next = #next_version_mod_ident :: #next_variant :: try_from(current)?;
							Ok(next.into())
						}
					});

					next2current_arms.push(quote! {
						#next_version_mod_ident :: #ident :: #next_pattern => {
							let next = #next_version_mod_ident :: #next_variant :: new(#next_fields);
							let current = #current_version_mod_ident :: #current_variant ::try_from(next)?;
							Ok(current.into())
						}
					});
				}
			}

			let variant_structs_auto_convs = current_variants.iter().map(|v| {
				v.as_struct().expand_auto_conversions(
					*current_version,
					*next_version,
					v.next_version_conversion(),
				)
			});

			let current_version_tokens = quote! {
				#(#variant_structs_auto_convs)*

				impl #impl_generics TryFrom<#current_version_mod_ident :: #ident #ty_generics> for #next_version_mod_ident :: #ident #ty_generics #where_clause {
					type Error = ();

					fn try_from(value: #current_version_mod_ident :: #ident #ty_generics) -> Result<Self, Self::Error> {
						match value {
							#(#current2next_arms),*

							#[allow(dead_code)]
							_ => Err(())
						}
					}
				}

				impl #impl_generics TryFrom<#next_version_mod_ident :: #ident #ty_generics> for #current_version_mod_ident :: #ident #ty_generics #where_clause {
					type Error = ();

					fn try_from(value: #next_version_mod_ident :: #ident #ty_generics) -> Result<Self, Self::Error> {
						match value {
							#(#next2current_arms),*

							#[allow(dead_code)]
							_ => Err(())
						}
					}
				}
			};

			result.extend(current_version_tokens);
		}

		let versioned_enum_ident = format_ident!["Versioned{ident}"];

		let versioned_enum_variants = self.versions.keys().map(|version| {
			let attrs = versioned_enum_variants_attrs_tokens.into_iter().map(|tokens| {
				let attr_tokens = expand_version_macro(*version, tokens.clone());
				quote!(#[ #attr_tokens ])
			});
			let version_mod = version_mod(*version);
			let version_variant = version_variant(*version);

			quote! {
				#(#attrs)*
				#version_variant(#version_mod :: #ident #ty_generics)
			}
		});

		let impl_versioned_trait = self.versions.keys()
			.map(|version| {
				let version_mod = version_mod(*version);
				quote! {
					impl #impl_generics crate::Versioned<#version> for #versioned_enum_ident #ty_generics #where_clause {
						type Type = #version_mod :: #ident #ty_generics;
					}
				}
			});

		let impl_identify_version_trait = {
			let match_arms = self.versions.keys().map(|version| {
				let version_variant = version_variant(*version);
				quote! { Self::#version_variant(_) => #version }
			});

			quote! {
				impl #impl_generics crate::IdentifyVersion for #versioned_enum_ident #ty_generics #where_clause {
					fn identify_version(&self) -> crate::Version {
						match self {
							#(#match_arms),*
						}
					}
				}
			}
		};

		let impl_from_traits = self.versions.keys()
			.map(|version| {
				let version_mod = version_mod(*version);
				let version_variant = version_variant(*version);

				quote! {
					impl #impl_generics From<#version_mod :: #ident #ty_generics> for #versioned_enum_ident #ty_generics #where_clause {
						fn from(v: #version_mod :: #ident #ty_generics) -> Self {
							Self::#version_variant(v)
						}
					}
				}
			});

		let versioned_enum = quote! {
			#(#versioned_enum_attrs)*
			#vis enum #versioned_enum_ident #version_enum_generics {
				#(#versioned_enum_variants),*
			}

			#(#impl_versioned_trait)*
			#impl_identify_version_trait
			#(#impl_from_traits)*
		};

		result.extend(versioned_enum);

		Ok(result)
	}

	fn get_toplevel_version_range(item_enum: &mut ItemEnum) -> Result<Range<Version>> {
		let version_attr =
			require_named_attr("version", &mut item_enum.attrs, item_enum.ident.span())?;

		version_attr.parse_args_with(|input: ParseStream| -> Result<Range<Version>> {
			let range = input.parse::<ExprRange>()?;
			let (start, end) = to_version_range(&range)?;

			if let Some(end) = end {
				Ok(start..end)
			} else {
				Err(Error::new(range.span(), "no end version is found"))
			}
		})
	}

	fn get_next_version_variant<'a>(
		variant: &VersionVariant,
		next_version_variants: &'a Vec<VersionVariant>,
	) -> Result<Option<&'a VersionVariant>> {
		match variant.next_version_conversion() {
			Some(NextVersionConversion::UserConversion(expected_next_variant)) =>
				Self::ensure_variant_in_next_version(
					&variant.inner().ident,
					expected_next_variant,
					next_version_variants,
				)
				.map(Some),
			Some(NextVersionConversion::AutoConversion) => {
				let auto_conv_variant = next_version_variants
					.iter()
					.find(|nv| nv.inner().ident == variant.inner().ident)
					.expect(&format!["auto conversion variants must exist in the next version: {variant:#?}\n{next_version_variants:#?}"]);

				Ok(Some(auto_conv_variant))
			},
			None => {
				Self::ensure_variant_removed_in_next_version(
					&variant.inner().ident,
					next_version_variants,
				)?;
				Ok(None)
			},
		}
	}

	fn ensure_variant_in_next_version<'a>(
		variant: &Ident,
		expected_next: &Ident,
		variants: &'a Vec<VersionVariant>,
	) -> Result<&'a VersionVariant> {
		variants.iter().find(|next_variant| next_variant.inner().ident == *expected_next).ok_or_else(|| {
			return Error::new(
				expected_next.span(),
				format![
					"the {} variant is expected to be converted into the {} variant in the next version. It is not found in the version. NOTE: you can remove the variant from the future versions by adding the `removed(since = VERSION)` attribute",
					variant,
					expected_next,
				],
			);
		})
	}

	fn ensure_variant_removed_in_next_version(
		variant: &Ident,
		variants: &Vec<VersionVariant>,
	) -> Result<()> {
		if variants
			.iter()
			.find(|next_variant| next_variant.inner().ident == *variant)
			.is_some()
		{
			return Err(Error::new(
				variant.span(),
				format![
						"the {} variant is expected to be removed in the next version, but it is found there",
						variant,
					],
			));
		}

		Ok(())
	}

	fn variant_pattern_and_fields(variant: &Variant) -> (TokenStream2, TokenStream2) {
		let ident = &variant.ident;
		let fields = &variant.fields;

		let members = fields.members();

		if matches!(fields, Fields::Named(_)) {
			let matched_members = members.clone();

			(quote!(#ident { #(#members),* }), quote!(#(#matched_members),*))
		} else {
			let members = members.map(|member| format_ident!("field{}", member));
			let matched_members = members.clone();

			(quote!(#ident ( #(#members)* )), quote!(#(#matched_members),*))
		}
	}
}

fn find_named_attr(attr_name: &str, attrs: &mut Vec<Attribute>) -> Result<Option<Attribute>> {
	let mut version_attrs = attrs.iter().filter(|a| a.path().is_ident(attr_name)).enumerate();

	let Some((pos, _)) = version_attrs.next() else {
		return Ok(None);
	};

	if let Some((_, another_version_attr)) = version_attrs.next() {
		return Err(Error::new(
			another_version_attr.path().span(),
			format!["duplicate `{attr_name}` attribute"],
		));
	}

	let version_attr = attrs.remove(pos);
	Ok(Some(version_attr))
}

fn require_named_attr(
	attr_name: &str,
	attrs: &mut Vec<Attribute>,
	notfound_err_span: Span,
) -> Result<Attribute> {
	// TODO: use Result::flatten (stabilized in 1.89.0)
	match find_named_attr(attr_name, attrs) {
		Ok(Some(attr)) => Ok(attr),
		Ok(None) =>
			Err(Error::new(notfound_err_span, format!["the `{attr_name}` attribute is not found"])),
		Err(err) => Err(err),
	}
}

fn take_all_attrs_with_name(attr_name: &str, attrs: &mut Vec<Attribute>) -> Vec<Attribute> {
	let mut filtered = vec![];

	attrs.retain(|attr| {
		if attr.path().is_ident(attr_name) {
			filtered.push(attr.clone());
			false
		} else {
			true
		}
	});

	filtered
}

fn to_version_range(range: &ExprRange) -> Result<(Version, Option<Version>)> {
	let range_span = range.span();

	let start = range
		.start
		.as_ref()
		.ok_or_else(|| Error::new(range_span, "version must have the start value"))?;
	let range_start = get_version_val(start)?;

	let range_end = range.end.as_ref().map(|end| get_version_val(end)).transpose()?.map(|end| {
		if let RangeLimits::Closed(_) = range.limits {
			end + 1
		} else {
			end
		}
	});
	Ok((range_start, range_end))
}

fn get_version_val(expr: &Expr) -> Result<Version> {
	match expr {
		Expr::Lit(ExprLit { attrs, lit: Lit::Int(lit) }) if attrs.is_empty() => lit.base10_parse(),
		_ => Err(Error::new(expr.span(), "a version value is expected")),
	}
}

fn expand_version_macro(version: Version, tokens: TokenStream2) -> TokenStream2 {
	let mut iter = tokens.into_iter();

	TokenStream2::from_iter(core::iter::from_fn(|| {
		while let Some(tt) = iter.next() {
			match &tt {
				token @ TokenTree::Ident(ident) if ident == "version" => {
					let restore = iter.clone();

					let next1 = iter.next();
					let next2 = iter.next();

					match (next1, next2) {
						(Some(TokenTree::Punct(p)), Some(TokenTree::Group(g)))
							if p.as_char() == '!' && g.stream().is_empty() =>
						{
							return Some(Literal::u32_unsuffixed(version).into());
						},
						_ => {
							iter = restore;
							return Some(token.clone())
						},
					}
				},
				TokenTree::Group(g) =>
					return Some(
						Group::new(g.delimiter(), expand_version_macro(version, g.stream())).into(),
					),
				_ => return Some(tt),
			}
		}

		None
	}))
}
