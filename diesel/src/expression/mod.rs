//! AST types representing various typed SQL expressions. Almost all types
//! implement either [`Expression`](/diesel/expression/trait.Expression.html) or
//! [`AsExpression`](/diesel/expression/trait.AsExpression.html).
//!
//! The most common expression to work with is a
//! [`Column`](../query_source/trait.Column.html). There are various methods
//! that you can call on these, found in
//! [`expression_methods`](expression_methods/index.html). You can also call
//! numeric operators on types which have been passed to
//! [`operator_allowed!`](../macro.operator_allowed.html) or
//! [`numeric_expr!`](../macro.numeric_expr.html).
//!
//! Any primitive which implements [`ToSql`](../types/trait.ToSql.html) will
//! also implement [`AsExpression`](trait.AsExpression.html), allowing it to be
//! used as an argument to any of the methods described here.
#[macro_use]
#[doc(hidden)]
pub mod ops;

#[doc(hidden)]
pub mod array_comparison;
#[doc(hidden)]
pub mod bound;
#[doc(hidden)]
pub mod coerce;
#[doc(hidden)]
pub mod count;
#[doc(hidden)]
pub mod exists;
#[doc(hidden)]
#[macro_use]
pub mod functions;
#[doc(hidden)]
pub mod grouped;
#[macro_use]
pub mod helper_types;
mod not;
#[doc(hidden)]
pub mod nullable;
#[doc(hidden)]
#[macro_use]
pub mod operators;
#[doc(hidden)]
pub mod sql_literal;

#[doc(hidden)]
pub mod dsl {
    #[doc(inline)]
    pub use super::count::{count, count_star};
    #[doc(inline)]
    pub use super::exists::exists;
    #[doc(inline)]
    pub use super::functions::aggregate_folding::*;
    #[doc(inline)]
    pub use super::functions::aggregate_ordering::*;
    #[doc(inline)]
    pub use super::functions::date_and_time::*;
    #[doc(inline)]
    pub use super::not::not;
    #[doc(inline)]
    pub use super::sql_literal::sql;

    #[cfg(feature = "postgres")]
    pub use pg::expression::dsl::*;
}

#[doc(inline)]
pub use self::sql_literal::SqlLiteral;

use backend::Backend;
use dsl::AsExprOf;

/// Represents a typed fragment of SQL.
///
/// Apps should not need to implement this type directly, but it may be common
/// to use this in where clauses. Libraries should consider using
/// [`diesel_infix_operator!`](../macro.diesel_infix_operator.html) or
/// [`diesel_postfix_operator!`](../macro.diesel_postfix_operator.html) instead of
/// implementing this directly.
pub trait Expression {
    type SqlType;
}

impl<T: Expression + ?Sized> Expression for Box<T> {
    type SqlType = T::SqlType;
}

impl<'a, T: Expression + ?Sized> Expression for &'a T {
    type SqlType = T::SqlType;
}

/// Converts a type to its representation for use in Diesel's query builder.
///
/// Implementations of this trait will generally do one of 3 things:
///
/// - Return `self` for types which are already parts of Diesel's query builder
/// - Perform some implicit coercion (for example, allowing [`now`] to be used as
///   both [`Timestamp`] and [`Timestamptz`].
/// - Indicate that the type has data which will be sent separately from the
///   query. This is generally referred as a "bind parameter". Types which
///   implement [`ToSql`] will generally implement `AsExpression` this way.
pub trait AsExpression<T> {
    type Expression: Expression<SqlType = T>;

    fn as_expression(self) -> Self::Expression;
}

impl<T: Expression> AsExpression<T::SqlType> for T {
    type Expression = Self;

    fn as_expression(self) -> Self {
        self
    }
}

/// Converts a type to its representation for use in Diesel's query builder.
///
/// This trait only exists to make usage of `AsExpression` more ergonomic when
/// the `SqlType` cannot be inferred. It is generally used when you need to use
/// a Rust value as the left hand side of an expression, or when you want to
/// select a constant value.
///
/// # Example
///
/// ```rust
/// # #[macro_use] extern crate diesel;
/// # include!("../doctest_setup.rs");
/// #
/// # table! {
/// #     users {
/// #         id -> Integer,
/// #         name -> VarChar,
/// #     }
/// # }
/// #
/// # fn main() {
/// use diesel::types::Text;
/// #   let conn = establish_connection();
/// let names = users::table
///     .select("The Amazing ".into_sql::<Text>().concat(users::name))
///     .load(&conn);
/// let expected_names = vec![
///     "The Amazing Sean".to_string(),
///     "The Amazing Tess".to_string(),
/// ];
/// assert_eq!(Ok(expected_names), names);
/// # }
/// ```
pub trait IntoSql {
    /// Convert `self` to an expression for Diesel's query builder.
    ///
    /// There is no difference in behavior between `x.into_sql::<Y>()` and
    /// `AsExpression::<Y>::as_expression(x)`.
    fn into_sql<T>(self) -> AsExprOf<Self, T>
    where
        Self: AsExpression<T> + Sized,
    {
        self.as_expression()
    }

    /// Convert `&self` to an expression for Diesel's query builder.
    ///
    /// There is no difference in behavior between `x.as_sql::<Y>()` and
    /// `AsExpression::<Y>::as_expression(&x)`.
    fn as_sql<'a, T>(&'a self) -> AsExprOf<&'a Self, T>
    where
        &'a Self: AsExpression<T>,
    {
        self.as_expression()
    }
}

impl<T> IntoSql for T {}

/// Indicates that all elements of an expression are valid given a from clause.
/// This is used to ensure that `users.filter(posts::id.eq(1))` fails to
/// compile. This constraint is only used in places where the nullability of a
/// SQL type doesn't matter (everything except `select` and `returning`). For
/// places where nullability is important, `SelectableExpression` is used
/// instead.
pub trait AppearsOnTable<QS: ?Sized>: Expression {}

impl<T: ?Sized, QS> AppearsOnTable<QS> for Box<T>
where
    T: AppearsOnTable<QS>,
    Box<T>: Expression,
{
}

impl<'a, T: ?Sized, QS> AppearsOnTable<QS> for &'a T
where
    T: AppearsOnTable<QS>,
    &'a T: Expression,
{
}

/// Indicates that an expression can be selected from a source. Columns will
/// implement this for their table. Certain special types, like `CountStar` and
/// `Bound` will implement this for all sources. Most compound expressions will
/// implement this if each of their parts implement it.
///
/// Notably, columns will not implement this trait for the right side of a left
/// join. To select a column or expression using a column from the right side of
/// a left join, you must call `.nullable()` on it.
pub trait SelectableExpression<QS: ?Sized>: AppearsOnTable<QS> {}

impl<T: ?Sized, QS> SelectableExpression<QS> for Box<T>
where
    T: SelectableExpression<QS>,
    Box<T>: AppearsOnTable<QS>,
{
}

impl<'a, T: ?Sized, QS> SelectableExpression<QS> for &'a T
where
    T: SelectableExpression<QS>,
    &'a T: AppearsOnTable<QS>,
{
}

/// Marker trait to indicate that an expression does not include any aggregate
/// functions. Used to ensure that aggregate expressions aren't mixed with
/// non-aggregate expressions in a select clause, and that they're never
/// included in a where clause.
pub trait NonAggregate {}

impl<T: NonAggregate + ?Sized> NonAggregate for Box<T> {}

impl<'a, T: NonAggregate + ?Sized> NonAggregate for &'a T {}

use query_builder::{QueryFragment, QueryId};

/// Helper trait used when boxing expressions. This exists to work around the
/// fact that Rust will not let us use non-core types as bounds on a trait
/// object (you could not return `Box<Expression+NonAggregate>`)
///
/// # Examples
///
/// ```rust
/// # #[macro_use] extern crate diesel;
/// # use diesel::types;
/// # include!("../doctest_setup.rs");
/// #
/// # table! {
/// #     users {
/// #         id -> Integer,
/// #         name -> VarChar,
/// #     }
/// # }
///
/// # #[derive(PartialEq, Eq, Debug)]
/// #[derive(Queryable)]
/// struct User {
///     id: i32,
///     name: String,
/// }
///
/// fn main() {
///     let conn = establish_connection();
///     let where_clause: Box<BoxableExpression<users::table, _, SqlType=types::Bool>>;
///     let search_by_id = true;
///
///     if search_by_id {
///         where_clause = Box::new(users::id.eq(1))
///     } else {
///         where_clause = Box::new(users::name.eq("Tess".to_string()))
///     }
///
///     // BoxableExpression can be chained
///     let where_clause = where_clause.and(Box::new(users::id.ne(10)));
///
///     let result = users::table.filter(where_clause).load::<User>(&conn);
///     assert_eq!(result, Ok(vec![User { id: 1, name: "Sean".into() }]));
/// }
/// ```
pub trait BoxableExpression<QS, DB>
where
    DB: Backend,
    Self: Expression,
    Self: SelectableExpression<QS>,
    Self: NonAggregate,
    Self: QueryFragment<DB>,
{
}

impl<QS, T, DB> BoxableExpression<QS, DB> for T
where
    DB: Backend,
    T: Expression,
    T: SelectableExpression<QS>,
    T: NonAggregate,
    T: QueryFragment<DB>,
{
}

impl<QS, ST, DB> QueryId for BoxableExpression<QS, DB, SqlType = ST> {
    type QueryId = ();

    const HAS_STATIC_QUERY_ID: bool = false;
}
