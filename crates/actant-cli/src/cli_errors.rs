use actant_core::ActantError;

pub(crate) fn invalid_input(detail: impl Into<String>) -> ActantError {
    ActantError::InvalidInput(detail.into())
}

pub(crate) fn not_found(detail: impl Into<String>) -> ActantError {
    ActantError::NotFound(detail.into())
}

pub(crate) fn conflict(detail: impl Into<String>) -> ActantError {
    ActantError::Conflict(detail.into())
}

pub(crate) fn storage(action: &str, err: impl std::fmt::Display) -> ActantError {
    ActantError::Storage(format!("{action}: {err}"))
}

pub(crate) fn internal(action: &str, err: impl std::fmt::Display) -> ActantError {
    ActantError::Internal(format!("{action}: {err}"))
}

pub(crate) fn print_public_error(err: &anyhow::Error) {
    if let Some(e) = err.downcast_ref::<ActantError>() {
        print_actant_error(e);
        return;
    }
    let fallback = ActantError::Internal(err.to_string());
    print_actant_error(&fallback);
}

pub(crate) fn print_actant_error(e: &ActantError) {
    eprintln!("error: {}", e.code());
    eprintln!("detail: {e}");
    eprintln!("fix: {}", e.fix().unwrap_or_else(|| e.hint()));
}
