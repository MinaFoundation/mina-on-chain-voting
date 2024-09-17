use anyhow::Result;
use axum::{
  extract::Json,
  http::StatusCode,
  response::{IntoResponse, Response},
};
use serde::Serialize;

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

impl<T: Serialize, E: ToString> Wrapper<Result<T, E>> {
  pub fn wrapper_into_response(self) -> Response {
    match self.0 {
      Ok(v) => Json(v).into_response(),
      Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
  }
}
