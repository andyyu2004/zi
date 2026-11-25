use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{
    Data, DeriveInput, Fields, GenericArgument, Ident, Path, PathArguments, Type, Visibility,
    parse_macro_input,
};

#[proc_macro_derive(Relation, attributes(index, references, relation))]
pub fn derive_relation(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    relation(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

#[proc_macro_derive(Database)]
pub fn derive_database(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    database(&input).unwrap_or_else(syn::Error::into_compile_error).into()
}

struct Field {
    ident: Ident,
    ty: Type,
    vis: Visibility,
}

#[derive(Clone, Copy, PartialEq)]
enum OnDelete {
    Restrict,
    Cascade,
    SetNull,
}

struct Reference {
    field: usize,
    parent: Path,
    on_delete: OnDelete,
    /// The column is `Option<Key>`, so a row may refer to nothing.
    optional: bool,
}

struct Index {
    /// Indexes into the struct's fields.
    columns: Vec<usize>,
    unique: bool,
    ordered: bool,
}

#[derive(Default)]
struct IndexOptions {
    unique: bool,
    ordered: bool,
    columns: Vec<Ident>,
}

fn parse_index_options(attr: &syn::Attribute) -> syn::Result<IndexOptions> {
    let mut opts = IndexOptions::default();
    if matches!(attr.meta, syn::Meta::Path(_)) {
        return Ok(opts);
    }
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("unique") {
            opts.unique = true;
        } else if meta.path.is_ident("ordered") {
            opts.ordered = true;
        } else if meta.path.is_ident("columns") {
            meta.parse_nested_meta(|column| {
                let ident = column.path.require_ident()?.clone();
                opts.columns.push(ident);
                Ok(())
            })?;
        } else {
            return Err(meta.error("expected `unique`, `ordered` or `columns(...)`"));
        }
        Ok(())
    })?;
    Ok(opts)
}

/// `T` if `ty` is written `name<T, ...>` (by its last path segment, so `std::option::Option<T>` too).
fn type_argument<'a>(ty: &'a Type, name: &str) -> Option<&'a Type> {
    if let Type::Path(path) = ty
        && let Some(last) = path.path.segments.last()
        && last.ident == name
        && let PathArguments::AngleBracketed(args) = &last.arguments
        && let Some(GenericArgument::Type(ty)) = args.args.first()
    {
        Some(ty)
    } else {
        None
    }
}

fn parse_references(attr: &syn::Attribute, field: usize, ty: &Type) -> syn::Result<Reference> {
    let mut parent = None;
    let mut on_delete = OnDelete::Restrict;
    let mut on_delete_span = attr.span();
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("on_delete") {
            let value: Ident = meta.value()?.parse()?;
            on_delete_span = value.span();
            on_delete = match value.to_string().as_str() {
                "cascade" => OnDelete::Cascade,
                "restrict" => OnDelete::Restrict,
                "set_null" => OnDelete::SetNull,
                _ => {
                    return Err(syn::Error::new(
                        value.span(),
                        "expected `cascade`, `restrict` or `set_null`",
                    ));
                }
            };
        } else if parent.is_none() {
            parent = Some(meta.path.clone());
        } else {
            return Err(meta.error("expected `on_delete = cascade|restrict|set_null`"));
        }
        Ok(())
    })?;
    let optional = type_argument(ty, "Option").is_some();
    if on_delete == OnDelete::SetNull && !optional {
        return Err(syn::Error::new(
            on_delete_span,
            "`set_null` needs the column to be an `Option`",
        ));
    }
    let parent = parent.ok_or_else(|| {
        syn::Error::new(attr.span(), "expected the referenced relation, e.g. `#[references(User)]`")
    })?;
    Ok(Reference { field, parent, on_delete, optional })
}

fn relation(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let vis = &input.vis;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new(input.generics.span(), "relations can't be generic"));
    }
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "`Relation` can only be derived for structs",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new(data.fields.span(), "`Relation` needs named fields"));
    };

    let fields: Vec<Field> = named
        .named
        .iter()
        .map(|f| Field { ident: f.ident.clone().unwrap(), ty: f.ty.clone(), vis: f.vis.clone() })
        .collect();
    let field_index = |ident: &Ident| -> syn::Result<usize> {
        fields
            .iter()
            .position(|f| f.ident == *ident)
            .ok_or_else(|| syn::Error::new(ident.span(), format!("no field `{ident}`")))
    };

    let mut indexes = Vec::new();
    let mut references = Vec::new();

    for (i, field) in named.named.iter().enumerate() {
        for attr in &field.attrs {
            if attr.path().is_ident("index") {
                let opts = parse_index_options(attr)?;
                if !opts.columns.is_empty() {
                    return Err(syn::Error::new(attr.span(), "`columns(...)` goes on the struct"));
                }
                indexes.push(Index {
                    columns: vec![i],
                    unique: opts.unique,
                    ordered: opts.ordered,
                });
            } else if attr.path().is_ident("references") {
                references.push(parse_references(attr, i, &field.ty)?);
            }
        }
    }

    let mut key_ty: Option<Type> = None;
    let mut columns_mod: Option<Ident> = None;
    for attr in &input.attrs {
        if attr.path().is_ident("index") {
            let opts = parse_index_options(attr)?;
            if opts.columns.len() < 2 {
                return Err(syn::Error::new(
                    attr.span(),
                    "a struct-level index needs `columns(a, b, ...)` with two or more columns; put single-column indexes on the field",
                ));
            }
            let columns = opts.columns.iter().map(field_index).collect::<syn::Result<_>>()?;
            indexes.push(Index { columns, unique: opts.unique, ordered: opts.ordered });
        } else if attr.path().is_ident("relation") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("key") {
                    key_ty = Some(meta.value()?.parse::<Type>()?);
                    Ok(())
                } else if meta.path.is_ident("columns") {
                    columns_mod = Some(meta.value()?.parse::<Ident>()?);
                    Ok(())
                } else {
                    Err(meta.error("expected `key = Type` or `columns = module_name`"))
                }
            })?;
        }
    }
    let key_ty = match key_ty {
        Some(ty) => quote!(#ty),
        None => quote!(::rel::Id<#name>),
    };

    let columns_mod =
        columns_mod.unwrap_or_else(|| format_ident!("{}", snake_case(&name.to_string())));

    // Nulling a column can't break a unique index only if no unique index has the column.
    for reference in &references {
        if reference.on_delete == OnDelete::SetNull
            && indexes.iter().any(|ix| ix.unique && ix.columns.contains(&reference.field))
        {
            return Err(syn::Error::new(
                fields[reference.field].ident.span(),
                "a `set_null` foreign key can't be in a unique index: nulling it on delete could break the index",
            ));
        }
    }

    // Foreign keys need an index on their column for cascades and joins.
    for reference in &references {
        if !indexes.iter().any(|ix| ix.columns == [reference.field]) {
            indexes.push(Index { columns: vec![reference.field], unique: false, ordered: false });
        }
    }

    let index_name = |ix: &Index| -> Ident {
        let columns: Vec<String> =
            ix.columns.iter().map(|&c| fields[c].ident.to_string()).collect();
        format_ident!("{}", columns.join("_and_"))
    };
    let key_type = |ix: &Index| -> TokenStream2 {
        match ix.columns.as_slice() {
            [c] => {
                let ty = &fields[*c].ty;
                quote!(#ty)
            }
            cs => {
                let tys = cs.iter().map(|&c| &fields[c].ty);
                quote!((#(#tys,)*))
            }
        }
    };
    let key_expr = |ix: &Index| -> TokenStream2 {
        match ix.columns.as_slice() {
            [c] => {
                let f = &fields[*c].ident;
                quote!(::std::clone::Clone::clone(&self.#f))
            }
            cs => {
                let fs = cs.iter().map(|&c| &fields[c].ident);
                quote!((#(::std::clone::Clone::clone(&self.#fs),)*))
            }
        }
    };
    let index_type = |ix: &Index| -> TokenStream2 {
        let key = key_type(ix);
        let kind = match (ix.unique, ix.ordered) {
            (false, false) => quote!(HashIndex),
            (true, false) => quote!(UniqueHashIndex),
            (false, true) => quote!(OrdIndex),
            (true, true) => quote!(UniqueOrdIndex),
        };
        quote!(::rel::#kind<#key, <#name as ::rel::Row>::Key>)
    };

    let indexes_struct = format_ident!("{}Indexes", name);
    let queries_name = format_ident!("{}Queries", name);
    let name_str = name.to_string();

    let index_fields = indexes.iter().map(|ix| {
        let ident = index_name(ix);
        let ty = index_type(ix);
        quote!(#ident: #ty)
    });

    let unique_checks = indexes.iter().filter(|ix| ix.unique).map(|ix| {
        let ident = index_name(ix);
        let ident_str = ident.to_string();
        let key = key_expr(ix);
        quote! {
            if indexes.#ident.contains(&#key) {
                return ::std::result::Result::Err(::rel::Error::Unique { relation: #name_str, index: #ident_str });
            }
        }
    });
    let index_inserts = indexes.iter().map(|ix| {
        let ident = index_name(ix);
        let key = key_expr(ix);
        quote!(indexes.#ident.insert(#key, id);)
    });
    let index_removes = indexes.iter().map(|ix| {
        let ident = index_name(ix);
        let key = key_expr(ix);
        quote!(indexes.#ident.remove(&#key, id);)
    });

    // Columns in an index (foreign keys always are) are read-only through `{Name}Mut`;
    // they change through `Database::set` or `Database::update`.
    let is_key = |field: usize| indexes.iter().any(|ix| ix.columns.contains(&field));
    let mut_name = format_ident!("{}Mut", name);
    let mut_fields = fields.iter().enumerate().map(|(i, f)| {
        let (fvis, ident, ty) = (&f.vis, &f.ident, &f.ty);
        if is_key(i) { quote!(#fvis #ident: &'a #ty) } else { quote!(#fvis #ident: &'a mut #ty) }
    });
    let mut_inits = fields.iter().enumerate().map(|(i, f)| {
        let ident = &f.ident;
        if is_key(i) { quote!(#ident: &self.#ident) } else { quote!(#ident: &mut self.#ident) }
    });

    let single_index =
        |field: usize| -> Option<&Index> { indexes.iter().find(|ix| ix.columns == [field]) };

    let reference_items = references.iter().map(|r| {
        let field = &fields[r.field].ident;
        let field_str = field.to_string();
        let parent = &r.parent;
        let parent_str = parent.segments.last().unwrap().ident.to_string();
        let on_delete = match r.on_delete {
            OnDelete::Restrict => quote!(Restrict),
            OnDelete::Cascade => quote!(Cascade),
            OnDelete::SetNull => quote!(SetNull),
        };
        let ix = index_name(single_index(r.field).expect("foreign keys are indexed"));
        let parent_key = quote!(<<#parent as ::rel::Row>::Key as ::rel::Key>::from_raw(parent));
        let lookup_key = if r.optional {
            quote!(::std::option::Option::Some(#parent_key))
        } else {
            parent_key
        };
        let set_null = if r.on_delete == OnDelete::SetNull {
            quote! {
                ::std::option::Option::Some({
                    fn set_null(row: &mut #name) { row.#field = ::std::option::Option::None; }
                    set_null
                })
            }
        } else {
            quote!(::std::option::Option::None)
        };
        quote! {
            ::rel::Reference {
                column: #field_str,
                references: #parent_str,
                parent: { fn parent() -> ::std::any::TypeId { ::std::any::TypeId::of::<#parent>() } parent },
                on_delete: ::rel::OnDelete::#on_delete,
                children: {
                    fn children(indexes: &#indexes_struct, parent: ::rel::RawId, f: &mut dyn ::std::ops::FnMut(::rel::RawId)) {
                        for id in ::rel::LookupIndex::lookup(&indexes.#ix, &#lookup_key) {
                            f(::rel::Key::raw(id));
                        }
                    }
                    children
                },
                set_null: #set_null,
            }
        }
    });

    let unconstrained = (references.is_empty() && !indexes.iter().any(|ix| ix.unique))
        .then(|| quote!(impl ::rel::Unconstrained for #name {}));

    let parents: Vec<&Path> = references.iter().map(|r| &r.parent).collect();
    let reference_checks = references.iter().map(|r| {
        let field = &fields[r.field].ident;
        let field_str = field.to_string();
        let parent = &r.parent;
        let parent_str = parent.segments.last().unwrap().ident.to_string();
        let error = quote! {
            return ::std::result::Result::Err(::rel::Error::ForeignKey {
                relation: #name_str,
                column: #field_str,
                references: #parent_str,
            });
        };
        if r.optional {
            quote! {
                if let ::std::option::Option::Some(key) = self.#field {
                    if !::rel::HasRelation::<#parent>::relation(db).contains(key) {
                        #error
                    }
                }
            }
        } else {
            quote! {
                if !::rel::HasRelation::<#parent>::relation(db).contains(self.#field) {
                    #error
                }
            }
        }
    });

    let columns = fields.iter().enumerate().map(|(i, f)| {
        let ident = &f.ident;
        let ident_str = ident.to_string();
        let ty = &f.ty;
        let fvis = nested_visibility(&f.vis);
        let index = match single_index(i) {
            Some(ix) => {
                let ix = index_name(ix);
                quote! {
                    ::std::option::Option::Some({
                        fn index(indexes: &#indexes_struct) -> &dyn ::rel::LookupIndex<#ty, <#name as ::rel::Row>::Key> { &indexes.#ix }
                        index
                    })
                }
            }
            None => quote!(::std::option::Option::None),
        };
        quote! {
            #fvis const #ident: ::rel::Column<#name, #ty> = ::rel::Column {
                name: #ident_str,
                get: { fn get(row: &#name) -> &#ty { &row.#ident } get },
                get_mut: { fn get_mut(row: &mut #name) -> &mut #ty { &mut row.#ident } get_mut },
                index: #index,
            };
        }
    });

    let mut query_sigs = Vec::new();
    let mut query_impls = Vec::new();
    for ix in &indexes {
        let ident = index_name(ix);
        let method = format_ident!("by_{}", ident);
        let key = key_type(ix);
        let (sig, body) = match (ix.columns.as_slice(), ix.unique, ix.ordered) {
            ([_], true, false) => (
                quote! {
                    fn #method<Q>(&self, key: &Q) -> ::std::option::Option<::rel::Ref<'_, #name>>
                    where #key: ::std::borrow::Borrow<Q>, Q: ::std::hash::Hash + ::std::cmp::Eq + ?Sized
                },
                quote!(self.indexes().#ident.get(key).map(|id| self.indexed(id))),
            ),
            ([_], false, false) => (
                quote! {
                    fn #method<Q>(&self, key: &Q) -> impl ::std::iter::Iterator<Item = ::rel::Ref<'_, #name>>
                    where #key: ::std::borrow::Borrow<Q>, Q: ::std::hash::Hash + ::std::cmp::Eq + ?Sized
                },
                quote!(self.indexes().#ident.get(key).map(|id| self.indexed(id))),
            ),
            ([_], _, true) => (
                quote! {
                    fn #method(&self, range: impl ::std::ops::RangeBounds<#key>)
                        -> impl ::std::iter::Iterator<Item = ::rel::Ref<'_, #name>>
                },
                quote! {
                    let range = (range.start_bound().cloned(), range.end_bound().cloned());
                    self.indexes().#ident.range(range).map(|id| self.indexed(id))
                },
            ),
            (cs, unique, _) => {
                let params: Vec<_> = cs.iter().map(|&c| &fields[c].ident).collect();
                let tys = cs.iter().map(|&c| &fields[c].ty);
                let args = quote!(#(#params: &#tys),*);
                let key_value = quote!((#(::std::clone::Clone::clone(#params),)*));
                if unique {
                    (
                        quote!(fn #method(&self, #args) -> ::std::option::Option<::rel::Ref<'_, #name>>),
                        quote!(self.indexes().#ident.get(&#key_value).map(|id| self.indexed(id))),
                    )
                } else {
                    (
                        quote!(fn #method(&self, #args) -> impl ::std::iter::Iterator<Item = ::rel::Ref<'_, #name>>),
                        quote!(self.indexes().#ident.get(&#key_value).map(|id| self.indexed(id))),
                    )
                }
            }
        };
        query_sigs.push(quote!(#sig;));
        query_impls.push(quote!(#sig { #body }));
    }

    Ok(quote! {
        #[doc(hidden)]
        #[derive(Default)]
        #vis struct #indexes_struct {
            #(#index_fields,)*
        }

        /// Columns of
        #[doc = concat!("[`", #name_str, "`],")]
        /// for queries and `Database::set`.
        #[allow(non_upper_case_globals, unused_imports)]
        #vis mod #columns_mod {
            use super::*;
            #(#columns)*
        }

        /// Mutable access to a row of
        #[doc = concat!("[`", #name_str, "`]")]
        /// with its indexed and foreign key columns read-only.
        #[allow(dead_code)]
        #vis struct #mut_name<'a> {
            #(#mut_fields,)*
        }

        impl ::rel::Row for #name {
            type Key = #key_ty;
            type Indexes = #indexes_struct;
            type Mut<'a> = #mut_name<'a>;

            fn project(&mut self) -> Self::Mut<'_> {
                #mut_name { #(#mut_inits,)* }
            }

            const NAME: &'static str = #name_str;
            const REFERENCES: &'static [::rel::Reference<Self>] = &[#(#reference_items),*];

            fn check_unique(&self, indexes: &Self::Indexes) -> ::rel::Result<()> {
                #(#unique_checks)*
                ::std::result::Result::Ok(())
            }

            fn index_insert(&self, indexes: &mut Self::Indexes, id: Self::Key) {
                #(#index_inserts)*
            }

            fn index_remove(&self, indexes: &mut Self::Indexes, id: Self::Key) {
                #(#index_removes)*
            }
        }

        #unconstrained

        impl<D: ?Sized> ::rel::References<D> for #name
        where
            #(D: ::rel::HasRelation<#parents>,)*
        {
            fn check_references(&self, db: &D) -> ::rel::Result<()> {
                let _ = db;
                #(#reference_checks)*
                ::std::result::Result::Ok(())
            }
        }

        /// Index lookups on a relation of
        #[doc = concat!("[`", #name_str, "`].")]
        #vis trait #queries_name {
            #(#query_sigs)*
        }

        impl #queries_name for ::rel::Relation<#name> {
            #(#query_impls)*
        }
    })
}

fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The visibility a field's column needs inside the generated columns module to be
/// visible wherever the field is: one module further out for private and `super` fields.
fn nested_visibility(vis: &Visibility) -> TokenStream2 {
    match vis {
        Visibility::Inherited => quote!(pub(super)),
        // `self` can't be followed by more segments: a restriction names an ancestor module.
        Visibility::Restricted(r) if r.path.is_ident("self") => quote!(pub(super)),
        Visibility::Restricted(r)
            if r.path.segments.first().is_some_and(|s| s.ident == "super") =>
        {
            let path = &r.path;
            quote!(pub(in super::#path))
        }
        vis @ (Visibility::Public(_) | Visibility::Restricted(_)) => quote!(#vis),
    }
}

fn database(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "`Database` can only be derived for structs",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new(data.fields.span(), "`Database` needs named fields"));
    };

    let mut idents = Vec::new();
    let mut rows = Vec::new();
    // Fields that aren't relations are ordinary state and ignored.
    for field in &named.named {
        if let Some(row) = type_argument(&field.ty, "Relation") {
            idents.push(field.ident.clone().unwrap());
            rows.push(row.clone());
        }
    }

    Ok(quote! {
        // Every relation's foreign keys must point at relations this database has.
        const _: () = {
            fn assert_references<T: ::rel::References<#name>>() {}
            fn assert_all() {
                #(assert_references::<#rows>();)*
            }
        };

        impl ::rel::Database for #name {
            fn visit_children(
                &self,
                parent_type: ::std::any::TypeId,
                parent: ::rel::RawId,
                f: &mut dyn ::std::ops::FnMut(::rel::Child),
            ) {
                #(self.#idents.visit_children(parent_type, parent, f);)*
            }

            fn remove_checked(&mut self, row_type: ::std::any::TypeId, id: ::rel::RawId) {
                #(
                    if row_type == ::std::any::TypeId::of::<#rows>() {
                        self.#idents.remove_checked(id);
                        return;
                    }
                )*
                ::std::unreachable!("deleted rows come from this database's relations")
            }

            fn set_null_checked(&mut self, row_type: ::std::any::TypeId, id: ::rel::RawId, reference: usize) {
                #(
                    if row_type == ::std::any::TypeId::of::<#rows>() {
                        self.#idents.set_null_checked(id, reference);
                        return;
                    }
                )*
                ::std::unreachable!("nulled rows come from this database's relations")
            }
        }

        #(
            impl ::rel::KeyIn<#name> for <#rows as ::rel::Row>::Key {
                type Row = #rows;
            }
        )*

        #(
            impl ::rel::HasRelation<#rows> for #name {
                fn relation(&self) -> &::rel::Relation<#rows> {
                    &self.#idents
                }

                fn relation_mut(&mut self) -> &mut ::rel::Relation<#rows> {
                    &mut self.#idents
                }
            }
        )*
    })
}
