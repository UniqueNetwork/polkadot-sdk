// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
	deprecation::extract_or_return_allow_attrs,
	pallet::{
		expand::warnings::{weight_constant_warning, weight_witness_warning},
		parse::{
			call::{CallVariantDef, CallWeightDef},
			helper::CallReturnType,
		},
		Def,
	},
	COUNTER,
};
use proc_macro2::TokenStream as TokenStream2;
use proc_macro_warning::Warning;
use quote::{quote, ToTokens};
use syn::spanned::Spanned;

/// Expand the weight to final token stream and accumulate warnings.
fn expand_weight(
	prefix: &str,
	frame_support: &syn::Path,
	dev_mode: bool,
	weight_warnings: &mut Vec<Warning>,
	method: &CallVariantDef,
	weight: &CallWeightDef,
) -> TokenStream2 {
	match weight {
		CallWeightDef::DevModeDefault => quote::quote!(
			#frame_support::pallet_prelude::Weight::zero()
		),
		CallWeightDef::Immediate(e) => {
			weight_constant_warning(e, dev_mode, weight_warnings);
			weight_witness_warning(method, dev_mode, weight_warnings);

			e.into_token_stream()
		},
		CallWeightDef::Inherited(t) => {
			// Expand `<<T as Config>::WeightInfo>::$prefix$call_name()`.
			let n = &syn::Ident::new(&format!("{}{}", prefix, method.name), method.name.span());
			quote!({ < #t > :: #n () })
		},
	}
}

///
/// * Generate enum call and implement various trait on it.
/// * Implement Callable and call_function on `Pallet`
pub fn expand_call(def: &mut Def) -> proc_macro2::TokenStream {
	let (span, where_clause, methods, docs) = match def.call.as_ref() {
		Some(call) => {
			let span = call.attr_span;
			let where_clause = call.where_clause.clone();
			let methods = call.methods.clone();
			let docs = call.docs.clone();

			(span, where_clause, methods, docs)
		},
		None => (def.item.span(), def.config.where_clause.clone(), Vec::new(), Vec::new()),
	};
	let frame_support = &def.frame_support;
	let frame_system = &def.frame_system;
	let type_impl_gen = &def.type_impl_generics(span);
	let type_decl_bounded_gen = &def.type_decl_bounded_generics(span);
	let type_use_gen = &def.type_use_generics(span);
	let call_ident = syn::Ident::new("Call", span);
	let pallet_ident = &def.pallet_struct.pallet;

	let fn_name = methods.iter().map(|method| &method.name).collect::<Vec<_>>();
	let call_index = methods.iter().map(|method| method.call_index).collect::<Vec<_>>();
	let new_call_variant_fn_name = fn_name
		.iter()
		.map(|fn_name| quote::format_ident!("new_call_variant_{}", fn_name))
		.collect::<Vec<_>>();

	let new_call_variant_doc = fn_name
		.iter()
		.map(|fn_name| format!("Create a call with the variant `{}`.", fn_name))
		.collect::<Vec<_>>();

	let mut call_index_warnings = Vec::new();
	// Emit a warning for each call that is missing `call_index` when not in dev-mode.
	for method in &methods {
		if method.explicit_call_index || def.dev_mode {
			continue
		}

		let warning = Warning::new_deprecated("ImplicitCallIndex")
			.index(call_index_warnings.len())
			.old("use implicit call indices")
			.new("ensure that all calls have a `pallet::call_index` attribute or put the pallet into `dev` mode")
			.help_links(&[
				"https://github.com/paritytech/substrate/pull/12891",
				"https://github.com/paritytech/substrate/pull/11381"
			])
			.span(method.name.span())
			.build_or_panic();
		call_index_warnings.push(warning);
	}

	let mut fn_weight = Vec::<TokenStream2>::new();
	let mut weight_warnings = Vec::new();
	for method in &methods {
		let w = expand_weight(
			"",
			frame_support,
			def.dev_mode,
			&mut weight_warnings,
			method,
			&method.weight,
		);
		fn_weight.push(w);
	}
	debug_assert_eq!(fn_weight.len(), methods.len());

	let fn_doc = methods.iter().map(|method| &method.docs).collect::<Vec<_>>();

	let args_name = methods
		.iter()
		.map(|method| method.args.iter().map(|(_, name, _)| name.clone()).collect::<Vec<_>>())
		.collect::<Vec<_>>();

	let args_name_stripped = methods
		.iter()
		.map(|method| {
			method
				.args
				.iter()
				.map(|(_, name, _)| {
					syn::Ident::new(name.to_string().trim_start_matches('_'), name.span())
				})
				.collect::<Vec<_>>()
		})
		.collect::<Vec<_>>();

	let make_args_name_pattern = |ref_tok| {
		args_name
			.iter()
			.zip(args_name_stripped.iter())
			.map(|(args_name, args_name_stripped)| {
				args_name
					.iter()
					.zip(args_name_stripped)
					.map(|(args_name, args_name_stripped)| {
						if args_name == args_name_stripped {
							quote::quote!( #ref_tok #args_name )
						} else {
							quote::quote!( #args_name_stripped: #ref_tok #args_name )
						}
					})
					.collect::<Vec<_>>()
			})
			.collect::<Vec<_>>()
	};

	let args_name_pattern = make_args_name_pattern(None);
	let args_name_pattern_ref = make_args_name_pattern(Some(quote::quote!(ref)));

	let args_type = methods
		.iter()
		.map(|method| method.args.iter().map(|(_, _, type_)| type_.clone()).collect::<Vec<_>>())
		.collect::<Vec<_>>();

	let args_compact_attr = methods.iter().map(|method| {
		method
			.args
			.iter()
			.map(|(is_compact, _, type_)| {
				if *is_compact {
					quote::quote_spanned!(type_.span() => #[codec(compact)] )
				} else {
					quote::quote!()
				}
			})
			.collect::<Vec<_>>()
	});

	let default_docs =
		[syn::parse_quote!(r"Contains a variant per dispatchable extrinsic that this pallet has.")];
	let docs = if docs.is_empty() { &default_docs[..] } else { &docs[..] };

	let maybe_compile_error = if def.call.is_none() {
		quote::quote! {
			compile_error!(concat!(
				"`",
				stringify!($pallet_name),
				"` does not have #[pallet::call] defined, perhaps you should remove `Call` from \
				construct_runtime?",
			));
		}
	} else {
		proc_macro2::TokenStream::new()
	};

	let count = COUNTER.with(|counter| counter.borrow_mut().inc());
	let macro_ident = syn::Ident::new(&format!("__is_call_part_defined_{}", count), span);

	let capture_docs = if cfg!(feature = "no-metadata-docs") { "never" } else { "always" };

	// Wrap all calls inside of storage layers
	if let Some(call) = def.call.as_ref() {
		let item_impl =
			&mut def.item.content.as_mut().expect("Checked by def parser").1[call.index];
		let syn::Item::Impl(item_impl) = item_impl else {
			unreachable!("Checked by def parser");
		};

		item_impl.items.iter_mut().enumerate().for_each(|(i, item)| {
			if let syn::ImplItem::Fn(method) = item {
				let return_type =
					&call.methods.get(i).expect("def should be consistent with item").return_type;

				let (ok_type, err_type) = match return_type {
					CallReturnType::DispatchResult => (
						quote::quote!(()),
						quote::quote!(#frame_support::pallet_prelude::DispatchError),
					),
					CallReturnType::DispatchResultWithPostInfo => (
						quote::quote!(#frame_support::dispatch::PostDispatchInfo),
						quote::quote!(#frame_support::dispatch::DispatchErrorWithPostInfo),
					),
				};

				let block = &method.block;
				method.block = syn::parse_quote! {{
					// We execute all dispatchable in a new storage layer, allowing them
					// to return an error at any point, and undoing any storage changes.
					#frame_support::storage::with_storage_layer::<#ok_type, #err_type, _>(
						|| #block
					)
				}};
			}
		});
	}

	// Extracts #[allow] attributes, necessary so that we don't run into compiler warnings
	let maybe_allow_attrs = methods
		.iter()
		.map(|method| {
			let attrs = extract_or_return_allow_attrs(&method.attrs);
			quote::quote! {
					#(#attrs)*
			}
		})
		.collect::<Vec<_>>();

	let cfg_attrs = methods
		.iter()
		.map(|method| {
			let attrs =
				method.cfg_attrs.iter().map(|attr| attr.to_token_stream()).collect::<Vec<_>>();
			quote::quote!( #( #attrs )* )
		})
		.collect::<Vec<_>>();

	let feeless_checks = methods.iter().map(|method| &method.feeless_check).collect::<Vec<_>>();
	let feeless_check =
		feeless_checks.iter().zip(args_name.iter()).map(|(feeless_check, arg_name)| {
			if let Some(check) = feeless_check {
				quote::quote_spanned!(span => #check)
			} else {
				quote::quote_spanned!(span => |_origin, #( #arg_name, )*| { false })
			}
		});

	let deprecation = match crate::deprecation::get_deprecation_enum(
		&quote::quote! {#frame_support},
		methods.iter().map(|item| (item.call_index as u8, item.attrs.as_ref())),
	) {
		Ok(deprecation) => deprecation,
		Err(e) => return e.into_compile_error(),
	};

	// Implementation of the authorize function for each call
	// `authorize_fn_pallet_impl` writes the user-defined authorize function as a function
	// implementation for the pallet.
	// `authorize_impl` is the call to this former function to implement `Authorize` trait.
	let (authorize_fn_pallet_impl, authorize_impl) = methods
		.iter()
		.zip(args_name.iter())
		.zip(args_type.iter())
		.zip(cfg_attrs.iter())
		.map(|(((method, arg_name), arg_type), cfg_attr)| {
			if let Some(authorize_def) = &method.authorize {
				let authorize_fn = &authorize_def.expr;
				let attr_fn_getter = syn::Ident::new(
					&format!("__macro_inner_authorize_call_for_{}", method.name),
					authorize_fn.span(),
				);
				let source = syn::Ident::new("source", span);

				let authorize_fn_pallet_impl = quote::quote_spanned!(authorize_fn.span() =>
					// Closure don't have a writable type. So we fix the authorize token stream to
					// be any implementation of a specific function.
					// This allows to have good type inference on the closure.
					//
					// Then we wrap this into an implementation for `Pallet` in order to get access
					// to `Self` as `Pallet` instead of `Call`.
					#cfg_attr
					impl<#type_impl_gen> Pallet<#type_use_gen> #where_clause {
						#[doc(hidden)]
						fn #attr_fn_getter() -> impl Fn(
							#frame_support::pallet_prelude::TransactionSource,
							#( &#arg_type ),*
						) -> #frame_support::pallet_prelude::TransactionValidityWithRefund {
							#authorize_fn
						}
					}
				);

				// `source` is from outside this block, so we can't use the authorize_fn span.
				let authorize_impl = quote::quote!(
					{
						let authorize_fn = Pallet::<#type_use_gen>::#attr_fn_getter();
						let res = authorize_fn(#source, #( #arg_name, )*);

						Some(res)
					}
				);

				(authorize_fn_pallet_impl, authorize_impl)
			} else {
				(Default::default(), quote::quote!(None))
			}
		})
		.unzip::<_, _, Vec<TokenStream2>, Vec<TokenStream2>>();

	// Implementation of the authorize function weight for each call
	let mut authorize_fn_weight = Vec::<TokenStream2>::new();
	for method in &methods {
		let w = match &method.authorize {
			Some(authorize_def) => expand_weight(
				"authorize_",
				frame_support,
				def.dev_mode,
				&mut weight_warnings,
				method,
				&authorize_def.weight,
			),
			// No authorize logic, weight is negligible
			None => quote::quote!(#frame_support::pallet_prelude::Weight::zero()),
		};
		authorize_fn_weight.push(w);
	}
	assert_eq!(authorize_fn_weight.len(), methods.len());

	quote::quote_spanned!(span =>
		#[doc(hidden)]
		mod warnings {
			#(
				#call_index_warnings
			)*
			#(
				#weight_warnings
			)*
		}

		#[allow(unused_imports)]
		#[doc(hidden)]
		pub mod __substrate_call_check {
			#[macro_export]
			#[doc(hidden)]
			macro_rules! #macro_ident {
				($pallet_name:ident) => {
					#maybe_compile_error
				};
			}

			#[doc(hidden)]
			pub use #macro_ident as is_call_part_defined;
		}

		#( #[doc = #docs] )*
		#[derive(
			#frame_support::RuntimeDebugNoBound,
			#frame_support::CloneNoBound,
			#frame_support::EqNoBound,
			#frame_support::PartialEqNoBound,
			#frame_support::__private::codec::Encode,
			#frame_support::__private::codec::Decode,
			#frame_support::__private::codec::DecodeWithMemTracking,
			#frame_support::__private::scale_info::TypeInfo,
		)]
		#[codec(encode_bound())]
		#[codec(decode_bound())]
		#[scale_info(skip_type_params(#type_use_gen), capture_docs = #capture_docs)]
		#[allow(non_camel_case_types)]
		pub enum #call_ident<#type_decl_bounded_gen> #where_clause {
			#[doc(hidden)]
			#[codec(skip)]
			__Ignore(
				::core::marker::PhantomData<(#type_use_gen,)>,
				#frame_support::Never,
			),
			#(
				#cfg_attrs
				#( #[doc = #fn_doc] )*
				#[codec(index = #call_index)]
				#fn_name {
					#(
						#[allow(missing_docs)]
						#args_compact_attr #args_name_stripped: #args_type
					),*
				},
			)*
		}

		impl<#type_impl_gen> #call_ident<#type_use_gen> #where_clause {
			#(
				#cfg_attrs
				#[doc = #new_call_variant_doc]
				pub fn #new_call_variant_fn_name(
					#( #args_name_stripped: #args_type ),*
				) -> Self {
					Self::#fn_name {
						#( #args_name_stripped ),*
					}
				}
			)*
		}

		impl<#type_impl_gen> #frame_support::dispatch::GetDispatchInfo
			for #call_ident<#type_use_gen>
			#where_clause
		{
			fn get_dispatch_info(&self) -> #frame_support::dispatch::DispatchInfo {
				match *self {
					#(
						#cfg_attrs
						Self::#fn_name { #( #args_name_pattern_ref, )* } => {
							let __pallet_base_weight = #fn_weight;

							let __pallet_weight = <
								dyn #frame_support::dispatch::WeighData<( #( & #args_type, )* )>
							>::weigh_data(&__pallet_base_weight, ( #( #args_name, )* ));

							let __pallet_class = <
								dyn #frame_support::dispatch::ClassifyDispatch<
									( #( & #args_type, )* )
								>
							>::classify_dispatch(&__pallet_base_weight, ( #( #args_name, )* ));

							let __pallet_pays_fee = <
								dyn #frame_support::dispatch::PaysFee<( #( & #args_type, )* )>
							>::pays_fee(&__pallet_base_weight, ( #( #args_name, )* ));

							#frame_support::dispatch::DispatchInfo {
								call_weight: __pallet_weight,
								extension_weight: Default::default(),
								class: __pallet_class,
								pays_fee: __pallet_pays_fee,
							}
						},
					)*
					Self::__Ignore(_, _) => unreachable!("__Ignore cannot be used"),
				}
			}
		}

		impl<#type_impl_gen> #frame_support::dispatch::CheckIfFeeless for #call_ident<#type_use_gen>
			#where_clause
		{
			type Origin = #frame_system::pallet_prelude::OriginFor<T>;
			#[allow(unused_variables)]
			fn is_feeless(&self, origin: &Self::Origin) -> bool {
				match *self {
					#(
						#cfg_attrs
						Self::#fn_name { #( #args_name_pattern_ref, )* } => {
							let feeless_check = #feeless_check;
							feeless_check(origin, #( #args_name, )*)
						},
					)*
					Self::__Ignore(_, _) => unreachable!("__Ignore cannot be used"),
				}
			}
		}

		impl<#type_impl_gen> #frame_support::traits::GetCallName for #call_ident<#type_use_gen>
			#where_clause
		{
			fn get_call_name(&self) -> &'static str {
				match *self {
					#( #cfg_attrs Self::#fn_name { .. } => stringify!(#fn_name), )*
					Self::__Ignore(_, _) => unreachable!("__PhantomItem cannot be used."),
				}
			}

			fn get_call_names() -> &'static [&'static str] {
				&[ #( #cfg_attrs stringify!(#fn_name), )* ]
			}
		}

		impl<#type_impl_gen> #frame_support::traits::GetCallIndex for #call_ident<#type_use_gen>
			#where_clause
		{
			fn get_call_index(&self) -> u8 {
				match *self {
					#( #cfg_attrs Self::#fn_name { .. } => #call_index, )*
					Self::__Ignore(_, _) => unreachable!("__PhantomItem cannot be used."),
				}
			}

			fn get_call_indices() -> &'static [u8] {
				&[ #( #cfg_attrs #call_index, )* ]
			}
		}

		impl<#type_impl_gen> #frame_support::traits::UnfilteredDispatchable
			for #call_ident<#type_use_gen>
			#where_clause
		{
			type RuntimeOrigin = #frame_system::pallet_prelude::OriginFor<T>;
			fn dispatch_bypass_filter(
				self,
				origin: Self::RuntimeOrigin
			) -> #frame_support::dispatch::DispatchResultWithPostInfo {
				#frame_support::dispatch_context::run_in_context(|| {
					match self {
						#(
							#cfg_attrs
							Self::#fn_name { #( #args_name_pattern, )* } => {
								#frame_support::__private::sp_tracing::enter_span!(
									#frame_support::__private::sp_tracing::trace_span!(stringify!(#fn_name))
								);
								#maybe_allow_attrs
								#[allow(clippy::useless_conversion)]
								<#pallet_ident<#type_use_gen>>::#fn_name(origin, #( #args_name, )* )
									.map(Into::into).map_err(Into::into)
							},
						)*
						Self::__Ignore(_, _) => {
							let _ = origin; // Use origin for empty Call enum
							unreachable!("__PhantomItem cannot be used.");
						},
					}
				})
			}
		}

		impl<#type_impl_gen> #frame_support::dispatch::Callable<T> for #pallet_ident<#type_use_gen>
			#where_clause
		{
			type RuntimeCall = #call_ident<#type_use_gen>;
		}

		impl<#type_impl_gen> #pallet_ident<#type_use_gen> #where_clause {
			#[allow(dead_code)]
			#[doc(hidden)]
			pub fn call_functions() -> #frame_support::__private::metadata_ir::PalletCallMetadataIR {
				#frame_support::__private::metadata_ir::PalletCallMetadataIR  {
					ty: #frame_support::__private::scale_info::meta_type::<#call_ident<#type_use_gen>>(),
					deprecation_info: #deprecation,
				}
			}
		}

		#( #authorize_fn_pallet_impl )*

		impl<#type_impl_gen> #frame_support::traits::Authorize for #call_ident<#type_use_gen>
			#where_clause
		{
			fn authorize(&self, source: #frame_support::pallet_prelude::TransactionSource) -> ::core::option::Option<::core::result::Result<
				(
					#frame_support::pallet_prelude::ValidTransaction,
					#frame_support::pallet_prelude::Weight,
				),
				#frame_support::pallet_prelude::TransactionValidityError
			>>
			{
				match *self {
					#(
						#cfg_attrs
						Self::#fn_name { #( #args_name_pattern_ref, )* } => {
							#authorize_impl
						},
					)*
					Self::__Ignore(_, _) => {
						let _ = source;
						unreachable!("__Ignore cannot be used")
					},
				}
			}

			fn weight_of_authorize(&self) -> #frame_support::pallet_prelude::Weight {
				match *self {
					#(
						#cfg_attrs
						Self::#fn_name { #( #args_name_pattern_ref, )* } => {
							#authorize_fn_weight
						},
					)*
					Self::__Ignore(_, _) => unreachable!("__Ignore cannot be used"),
				}
			}
		}
	)
}
