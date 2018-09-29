#![deny(rust_2018_idioms)]
#![feature(in_band_lifetimes)]
#![feature(crate_visibility_modifier)]
#![feature(nll)]
#![feature(min_const_fn)]
#![feature(const_fn)]
#![feature(const_let)]
#![feature(try_from)]
#![allow(dead_code)]
#![allow(unused_imports)]

use derive_new::new;
use rustc_hash::FxHashMap;
use std::any::Any;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Write;
use std::hash::Hash;

pub mod memoized;
pub mod runtime;
pub mod transparent;

/// The base trait which your "query context" must implement. Gives
/// access to the salsa runtime, which you must embed into your query
/// context (along with whatever other state you may require).
pub trait QueryContext: QueryContextStorageTypes {
    /// Gives access to the underlying salsa runtime.
    fn salsa_runtime(&self) -> &runtime::Runtime<Self>;
}

/// Defines the `QueryDescriptor` associated type. An impl of this
/// should be generated for your query-context type automatically by
/// the `query_context_storage` macro, so you shouldn't need to mess
/// with this trait directly.
pub trait QueryContextStorageTypes: Sized {
    /// A "query descriptor" packages up all the possible queries and a key.
    /// It is used to store information about (e.g.) the stack.
    ///
    /// At runtime, it can be implemented in various ways: a monster enum
    /// works for a fixed set of queries, but a boxed trait object is good
    /// for a more open-ended option.
    type QueryDescriptor: QueryDescriptor<Self>;

    /// Defines the "storage type", where all the query data is kept.
    /// This type is defined by the `query_context_storage` macro.
    type QueryContextStorage: Default;
}

pub trait QueryDescriptor<QC>: Clone + Debug + Eq + Hash {}

pub trait Query<QC: QueryContext>: Debug + Default + Sized + 'static {
    type Key: Clone + Debug + Hash + Eq + Send;
    type Value: Clone + Debug + Hash + Eq + Send;
    type Storage: QueryStorageOps<QC, Self> + Send + Sync;

    fn execute(query: &QC, key: Self::Key) -> Self::Value;
}

pub trait QueryStorageOps<QC, Q>: Default
where
    QC: QueryContext,
    Q: Query<QC>,
{
    fn try_fetch<'q>(
        &self,
        query: &'q QC,
        key: &Q::Key,
        descriptor: impl FnOnce() -> QC::QueryDescriptor,
    ) -> Result<Q::Value, CycleDetected>;
}

#[derive(new)]
pub struct QueryTable<'me, QC, Q>
where
    QC: QueryContext,
    Q: Query<QC>,
{
    pub query: &'me QC,
    pub storage: &'me Q::Storage,
    pub descriptor_fn: fn(&QC, &Q::Key) -> QC::QueryDescriptor,
}

pub struct CycleDetected;

impl<QC, Q> QueryTable<'me, QC, Q>
where
    QC: QueryContext,
    Q: Query<QC>,
{
    pub fn of(&self, key: Q::Key) -> Q::Value {
        self.storage
            .try_fetch(self.query, &key, || self.descriptor(&key))
            .unwrap_or_else(|CycleDetected| {
                self.query
                    .salsa_runtime()
                    .report_unexpected_cycle(self.descriptor(&key))
            })
    }

    fn descriptor(&self, key: &Q::Key) -> QC::QueryDescriptor {
        (self.descriptor_fn)(self.query, key)
    }
}

/// A macro that helps in defining the "context trait" of a given
/// module.  This is a trait that defines everything that a block of
/// queries need to execute, as well as defining the queries
/// themselves that are exported for others to use.
///
/// This macro declares the "prototype" for a single query. This will
/// expand into a `fn` item. This prototype specifies name of the
/// method (in the example below, that would be `my_query`) and
/// connects it to query definition type (`MyQuery`, in the example
/// below). These typically have the same name but a distinct
/// capitalization convention. Note that the actual input/output type
/// of the query are given only in the query definition (see the
/// `query_definition` macro for more details).
///
/// ### Examples
///
/// The simplest example is something like this:
///
/// ```ignore
/// trait TypeckQueryContext {
///     query_prototype! {
///         /// Comments or other attributes can go here
///         fn my_query() for MyQuery;
///     }
/// }
/// ```
///
/// This just expands to something like:
///
/// ```ignore
/// fn my_query(&self) -> QueryTable<'_, Self, $query_type>;
/// ```
///
/// This permits us to invoke the query via `query.my_query().of(key)`.
///
/// You can also include more than one query if you prefer:
///
/// ```ignore
/// trait TypeckQueryContext {
///     query_prototype! {
///         fn my_query() for MyQuery;
///         fn my_other_query() for MyOtherQuery;
///     }
/// }
/// ```
#[macro_export]
macro_rules! query_prototype {
    (
        $(
            $(#[$attr:meta])*
            fn $method_name:ident() for $query_type:ty;
        )*
    ) => {
        $(
            $(#[$attr])*
            fn $method_name(&self) -> $crate::QueryTable<'_, Self, $query_type>;
        )*
    }
}

/// Creates a **Query Definition** type. This defines the input (key)
/// of the query, the output key (value), and the query context trait
/// that the query requires.
///
/// Example:
///
/// ```ignore
/// query_definition! {
///     pub MyQuery(query: &impl MyQueryContext, key: MyKey) -> MyValue {
///         ... // fn body specifies what happens when query is invoked
///     }
/// }
/// ```
///
/// Here, the query context trait would be `MyQueryContext` -- this
/// should be a trait containing all the other queries that the
/// definition needs to invoke (as well as any other methods that you
/// may want).
///
/// The `MyKey` type is the **key** to the query -- it must be Clone,
/// Debug, Hash, Eq, and Send, as specified in the `Query` trait.
///
/// The `MyKey` type is the **value** to the query -- it too must be
/// Clone, Debug, Hash, Eq, and Send, as specified in the `Query`
/// trait.
#[macro_export]
macro_rules! query_definition {
    // Step 1. Filtering the attributes to look for the special ones
    // we consume.
    (
        @filter_attrs {
            input { #[storage(memoized)] $($input:tt)* };
            storage { $storage:tt };
            other_attrs { $($other_attrs:tt)* };
        }
    ) => {
        $crate::query_definition! {
            @filter_attrs {
                input { $($input)* };
                storage { memoized };
                other_attrs { $($other_attrs)* };
            }
        }
    };

    (
        @filter_attrs {
            input { #[storage(transparent)] $($input:tt)* };
            storage { $storage:tt };
            other_attrs { $($other_attrs:tt)* };
        }
    ) => {
        $crate::query_definition! {
            @filter_attrs {
                input { $($input)* };
                storage { transparent };
                other_attrs { $($other_attrs)* };
            }
        }
    };

    (
        @filter_attrs {
            input { #[$attr:meta] $($input:tt)* };
            storage { $storage:tt };
            other_attrs { $($other_attrs:tt)* };
        }
    ) => {
        $crate::query_definition! {
            @filter_attrs {
                input { $($input)* };
                storage { $storage };
                other_attrs { $($other_attrs)* #[$attr] };
            }
        }
    };

    (
        @filter_attrs {
            input {
                $v:vis $name:ident(
                    $query:tt : &impl $query_trait:path,
                    $key:tt : $key_ty:ty $(,)*
                ) -> $value_ty:ty {
                    $($body:tt)*
                }
            };
            storage { $storage:tt };
            other_attrs { $($attrs:tt)* };
        }
    ) => {
        #[derive(Default, Debug)]
        $($attrs)*
        $v struct $name;

        impl<QC> $crate::Query<QC> for $name
        where
            QC: $query_trait,
        {
            type Key = $key_ty;
            type Value = $value_ty;
            type Storage = $crate::query_definition! { @storage_ty[QC, Self, $storage] };

            fn execute($query: &QC, $key: $key_ty) -> $value_ty {
                $($body)*
            }
        }
    };

    (
        @storage_ty[$QC:ident, $Self:ident, memoized]
    ) => {
        $crate::memoized::MemoizedStorage<$QC, $Self>
    };

    (
        @storage_ty[$QC:ident, $Self:ident, transparent]
    ) => {
        $crate::transparent::TransparentStorage
    };

    // Various legal start states:
    (
        # $($tokens:tt)*
    ) => {
        $crate::query_definition! {
            @filter_attrs {
                input { # $($tokens)* };
                storage { memoized };
                other_attrs { };
            }
        }
    };
    (
        $v:vis $name:ident $($tokens:tt)*
    ) => {
        $crate::query_definition! {
            @filter_attrs {
                input { $v $name $($tokens)* };
                storage { memoized };
                other_attrs { };
            }
        }
    };
}

/// This macro generates the "query storage" that goes into your query
/// context.
#[macro_export]
macro_rules! query_context_storage {
    (
        $(#[$attr:meta])*
        $v:vis struct $Storage:ident for $QueryContext:ty {
            $(
                impl $TraitName:path {
                    $(
                        fn $query_method:ident() for $QueryType:path;
                    )*
                }
            )*
        }
    ) => {
        #[derive(Default)]
        $(#[$attr])*
        $v struct $Storage {
            $(
                $(
                    $query_method: <$QueryType as $crate::Query<$QueryContext>>::Storage,
                )*
            )*
        }

        /// Identifies a query and its key. You are not meant to name
        /// this type directly or use its fields etc.  It is a
        /// **private query descriptor type generated by salsa** and
        /// its exact structure is subject to change. Sadly, I don't
        /// know any way to hide this with hygiene, so use `__`
        /// instead.
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        $v struct __SalsaQueryDescriptor {
            kind: __SalsaQueryDescriptorKind
        }

        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        enum __SalsaQueryDescriptorKind {
            $(
                $(
                    $query_method(<$QueryType as $crate::Query<$QueryContext>>::Key),
                )*
            )*
        }

        impl $crate::QueryContextStorageTypes for $QueryContext {
            type QueryDescriptor = __SalsaQueryDescriptor;
            type QueryContextStorage = $Storage;
        }

        impl $crate::QueryDescriptor<$QueryContext> for __SalsaQueryDescriptor {
        }

        $(
            impl $TraitName for $QueryContext {
                $(
                    fn $query_method(
                        &self,
                    ) -> $crate::QueryTable<'_, Self, $QueryType> {
                        $crate::QueryTable::new(
                            self,
                            &$crate::QueryContext::salsa_runtime(self)
                                .storage()
                                .$query_method,
                            |_, key| {
                                let key = std::clone::Clone::clone(key);
                                __SalsaQueryDescriptor {
                                    kind: __SalsaQueryDescriptorKind::$query_method(key),
                                }
                            },
                        )
                }
                )*
            }
        )*
    };
}
