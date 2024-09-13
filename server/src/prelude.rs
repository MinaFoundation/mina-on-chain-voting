pub use crate::error::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

pub use std::format as f;

/// Wrapper around a type `T` that can be used to implement external traits for
/// `T`.
///
/// # Examples
///
/// ```
/// use std::convert::From;
///
/// struct Wrapper<T>(pub T);
///
/// impl<T> From<T> for Wrapper<T> {
///     fn from(t: T) -> Self {
///         Wrapper(t)
///     }
/// }
///
///
/// let value = Wrapper::from(42);
/// assert_eq!(value.0, 42);
/// ```
pub struct Wrapper<T>(pub T);
