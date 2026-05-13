use once_cell::sync::OnceCell;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset, format_description::BorrowedFormatItem};

static LOCAL_OFFSET: OnceCell<UtcOffset> = OnceCell::new();

const COMMIT_FMT: &[BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

const SUFFIX_FMT: &[BorrowedFormatItem<'static>] =
    format_description!("[year][month][day]-[hour][minute][second]");

pub const DEFAULT_COMMIT_TEMPLATE: &str = "{ts} ({host})";

/// Determine the local UTC offset once, while the program is still
/// single-threaded. `OffsetDateTime::now_local()` becomes unreliable once a
/// multi-threaded tokio runtime is spinning, so we resolve it eagerly.
pub fn init_local_offset() {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    let _ = LOCAL_OFFSET.set(offset);
}

pub fn now_local() -> OffsetDateTime {
    let offset = LOCAL_OFFSET.get().copied().unwrap_or(UtcOffset::UTC);
    OffsetDateTime::now_utc().to_offset(offset)
}

pub fn render_commit_message(template: &str, now: OffsetDateTime, host: &str) -> String {
    let ts = now.format(COMMIT_FMT).unwrap_or_default();
    template.replace("{ts}", &ts).replace("{host}", host)
}

pub fn conflict_suffix(now: OffsetDateTime) -> String {
    now.format(SUFFIX_FMT).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn default_template_renders() {
        let fixed = datetime!(2026-05-13 14:32:07 UTC);
        let s = render_commit_message(DEFAULT_COMMIT_TEMPLATE, fixed, "sachan");
        assert_eq!(s, "2026-05-13 14:32:07 (sachan)");
    }

    #[test]
    fn custom_template_substitutes_both_placeholders() {
        let fixed = datetime!(2026-05-13 14:32:07 UTC);
        let s = render_commit_message("sync {host} @ {ts}", fixed, "moonbase");
        assert_eq!(s, "sync moonbase @ 2026-05-13 14:32:07");
    }

    #[test]
    fn conflict_suffix_format() {
        let fixed = datetime!(2026-05-13 14:32:07 UTC);
        assert_eq!(conflict_suffix(fixed), "20260513-143207");
    }
}
