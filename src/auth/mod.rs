//! Authentication: password hashing, JWT handling and the `AuthUser` extractor.

pub mod extractor;
pub mod jwt;
pub mod password;

pub use extractor::AuthUser;
