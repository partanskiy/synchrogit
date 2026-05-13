use once_cell::sync::OnceCell;

static HOSTNAME: OnceCell<String> = OnceCell::new();

pub fn hostname() -> &'static str {
    HOSTNAME.get_or_init(|| gethostname::gethostname().to_string_lossy().into_owned())
}
