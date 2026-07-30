pub mod auth;
pub mod biometric;
pub mod biometry;
pub mod error;
pub mod fido2;
pub mod session;
pub mod software;
pub mod traits;

pub use auth::{AuthManager, AuthStatus};
pub use biometric::{BiometricAuth, BiometricStub};
pub use biometry::BiometricEngine;
pub use biometry::mock::MockEngine;
pub use biometry::tauri::TauriBiometryEngine;
pub use error::AuthError;
pub use fido2::{Fido2Authenticator, Fido2Status, MockFido2Authenticator};
pub use session::{SessionKey, SessionKeyModule, SessionPermissions, SessionTx};
pub use software::SoftwareAuth;
pub use traits::{AuthProvider, AuthResult};