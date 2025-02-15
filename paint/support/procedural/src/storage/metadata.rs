// Copyright 2017-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Implementation of `storage_metadata` on module structure, used by construct_runtime.

use paint_support_procedural_tools::clean_type_string;
use proc_macro2::TokenStream;
use quote::quote;
use super::{DeclStorageDefExt, StorageLineDefExt, StorageLineTypeDef};

fn storage_line_metadata_type(scrate: &TokenStream, line: &StorageLineDefExt) -> TokenStream {
	let value_type = &line.value_type;
	let value_type = clean_type_string(&quote!( #value_type ).to_string());
	match &line.storage_type {
		StorageLineTypeDef::Simple(_) => {
			quote!{
				#scrate::metadata::StorageEntryType::Plain(
					#scrate::metadata::DecodeDifferent::Encode(#value_type),
				)
			}
		},
		StorageLineTypeDef::Map(map) => {
			let hasher = map.hasher.into_metadata();
			let key = &map.key;
			let key = clean_type_string(&quote!(#key).to_string());
			quote!{
				#scrate::metadata::StorageEntryType::Map {
					hasher: #scrate::metadata::#hasher,
					key: #scrate::metadata::DecodeDifferent::Encode(#key),
					value: #scrate::metadata::DecodeDifferent::Encode(#value_type),
					is_linked: false,
				}
			}
		},
		StorageLineTypeDef::LinkedMap(map) => {
			let hasher = map.hasher.into_metadata();
			let key = &map.key;
			let key = clean_type_string(&quote!(#key).to_string());
			quote!{
				#scrate::metadata::StorageEntryType::Map {
					hasher: #scrate::metadata::#hasher,
					key: #scrate::metadata::DecodeDifferent::Encode(#key),
					value: #scrate::metadata::DecodeDifferent::Encode(#value_type),
					is_linked: true,
				}
			}
		},
		StorageLineTypeDef::DoubleMap(map) => {
			let hasher1 = map.hasher1.into_metadata();
			let hasher2 = map.hasher2.into_metadata();
			let key1 = &map.key1;
			let key1 = clean_type_string(&quote!(#key1).to_string());
			let key2 = &map.key2;
			let key2 = clean_type_string(&quote!(#key2).to_string());
			quote!{
				#scrate::metadata::StorageEntryType::DoubleMap {
					hasher: #scrate::metadata::#hasher1,
					key1: #scrate::metadata::DecodeDifferent::Encode(#key1),
					key2: #scrate::metadata::DecodeDifferent::Encode(#key2),
					value: #scrate::metadata::DecodeDifferent::Encode(#value_type),
					key2_hasher: #scrate::metadata::#hasher2,
				}
			}
		},
	}
}

fn default_byte_getter(
	scrate: &TokenStream,
	line: &StorageLineDefExt,
	def: &DeclStorageDefExt,
) -> (TokenStream, TokenStream) {
	let default = line.default_value.as_ref().map(|d| quote!( #d ))
		.unwrap_or_else(|| quote!( Default::default() ));

	let str_name = line.name.to_string();
	let struct_name = syn::Ident::new(&("__GetByteStruct".to_string() + &str_name), line.name.span());
	let cache_name = syn::Ident::new(&("__CACHE_GET_BYTE_STRUCT_".to_string() + &str_name), line.name.span());

	let runtime_generic = &def.module_runtime_generic;
	let runtime_trait = &def.module_runtime_trait;
	let optional_instance_bound_optional_default = &def.optional_instance_bound_optional_default;
	let optional_instance_bound = &def.optional_instance_bound;
	let optional_instance = &def.optional_instance;
	let optional_comma_instance = optional_instance.as_ref().map(|i| quote!(, #i));
	let where_clause = &def.where_clause;

	let query_type = &line.query_type;

	let struct_def = quote! {
		#[doc(hidden)]
		pub struct #struct_name<
			#runtime_generic, #optional_instance_bound_optional_default
		>(pub #scrate::rstd::marker::PhantomData<(#runtime_generic #optional_comma_instance)>);

		#[cfg(feature = "std")]
		#[allow(non_upper_case_globals)]
		static #cache_name: #scrate::once_cell::sync::OnceCell<#scrate::rstd::vec::Vec<u8>> =
			#scrate::once_cell::sync::OnceCell::new();

		#[cfg(feature = "std")]
		impl<#runtime_generic: #runtime_trait, #optional_instance_bound>
			#scrate::metadata::DefaultByte
			for #struct_name<#runtime_generic, #optional_instance>
			#where_clause
		{
			fn default_byte(&self) -> #scrate::rstd::vec::Vec<u8> {
				use #scrate::codec::Encode;
				#cache_name.get_or_init(|| {
					let def_val: #query_type = #default;
					<#query_type as Encode>::encode(&def_val)
				}).clone()
			}
		}

		unsafe impl<#runtime_generic: #runtime_trait, #optional_instance_bound> Send
			for #struct_name<#runtime_generic, #optional_instance> #where_clause {}

		unsafe impl<#runtime_generic: #runtime_trait, #optional_instance_bound> Sync
			for #struct_name<#runtime_generic, #optional_instance> #where_clause {}

		#[cfg(not(feature = "std"))]
		impl<#runtime_generic: #runtime_trait, #optional_instance_bound>
			#scrate::metadata::DefaultByte
			for #struct_name<#runtime_generic, #optional_instance>
			#where_clause
		{
			fn default_byte(&self) -> #scrate::rstd::vec::Vec<u8> {
				use #scrate::codec::Encode;
				let def_val: #query_type = #default;
				<#query_type as Encode>::encode(&def_val)
			}
		}
	};
	let struct_instance = quote!(
		#struct_name::<#runtime_generic, #optional_instance>(#scrate::rstd::marker::PhantomData)
	);

	(struct_def, struct_instance)
}

pub fn impl_metadata(scrate: &TokenStream, def: &DeclStorageDefExt) -> TokenStream {
	let mut entries = TokenStream::new();
	let mut default_byte_getter_struct_defs = TokenStream::new();

	for line in def.storage_lines.iter() {
		let str_name = line.name.to_string();

		let modifier = if line.is_option {
			quote!(#scrate::metadata::StorageEntryModifier::Optional)
		} else {
			quote!(#scrate::metadata::StorageEntryModifier::Default)
		};

		let ty = storage_line_metadata_type(scrate, line);

		let (
			default_byte_getter_struct_def,
			default_byte_getter_struct_instance,
		) = default_byte_getter(scrate, line, def);

		let mut docs = TokenStream::new();
		for attr in line.attrs.iter().filter_map(|v| v.parse_meta().ok()) {
			if let syn::Meta::NameValue(meta) = attr {
				if meta.path.is_ident("doc") {
					let lit = meta.lit;
					docs.extend(quote!(#lit,));
				}
			}
		}

		let entry = quote! {
			#scrate::metadata::StorageEntryMetadata {
				name: #scrate::metadata::DecodeDifferent::Encode(#str_name),
				modifier: #modifier,
				ty: #ty,
				default: #scrate::metadata::DecodeDifferent::Encode(
					#scrate::metadata::DefaultByteGetter(&#default_byte_getter_struct_instance)
				),
				documentation: #scrate::metadata::DecodeDifferent::Encode(&[ #docs ]),
			},
		};

		default_byte_getter_struct_defs.extend(default_byte_getter_struct_def);
		entries.extend(entry);
	}

	let prefix = if let Some(instance) = &def.module_instance {
		let instance_generic = &instance.instance_generic;
		quote!(#instance_generic::PREFIX)
	} else {
		let prefix = def.crate_name.to_string();
		quote!(#prefix)
	};

	let store_metadata = quote!(
		#scrate::metadata::StorageMetadata {
			prefix: #scrate::metadata::DecodeDifferent::Encode(#prefix),
			entries: #scrate::metadata::DecodeDifferent::Encode(&[ #entries ][..]),
		}
	);

	let module_struct = &def.module_struct;
	let module_impl = &def.module_impl;
	let where_clause = &def.where_clause;

	quote!(
		#default_byte_getter_struct_defs

		impl#module_impl #module_struct #where_clause {
			#[doc(hidden)]
			pub fn storage_metadata() -> #scrate::metadata::StorageMetadata {
				#store_metadata
			}
		}
	)
}
