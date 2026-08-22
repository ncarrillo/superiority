mod credentials;
mod file;

pub use credentials::{AuthenticationOutcome, CredentialStore, authenticate_cached};
pub use file::FileCredentialStore;
