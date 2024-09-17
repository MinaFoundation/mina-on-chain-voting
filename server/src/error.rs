use axum::{
  http::StatusCode,
  response::{IntoResponse, Response},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
  #[error(transparent)]
  Diesel(#[from] diesel::result::Error),

  #[error(transparent)]
  Anyhow(#[from] anyhow::Error),
}

impl Error {
  fn status_code(&self) -> StatusCode {
    match self {
      Self::Diesel(ref error) => match error {
        diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
      },

      Self::Anyhow(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
  }
}

impl IntoResponse for Error {
  fn into_response(self) -> Response {
    match self {
      Self::Diesel(ref error) => tracing::error!("Error: {error}"),
      Self::Anyhow(ref error) => tracing::error!("Error: {error}"),
    };
    (self.status_code(), self.to_string()).into_response()
  }
}
