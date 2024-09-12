mod info;
mod proposal;

use axum::Router;

pub(crate) trait Build {
  fn build() -> Router;
}

impl Build for Router {
  fn build() -> Router {
    proposal::router().merge(info::router())
  }
}
