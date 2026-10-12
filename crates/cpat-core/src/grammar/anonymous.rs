/// Numbers the anonymous patterns of ONE build: `anonymous-pattern-0`,
/// `-1`, … A build starts its own count, so the same grammar always gets
/// the same names, whatever was built before it (v12 `AnonymousNames`).
#[derive(Debug, Default)]
pub(super) struct AnonymousNames {
    next: usize,
}

/// What every anonymous name starts with. A literal or regex built under
/// such a name is named by its content instead.
pub(super) const PREFIX: &str = "anonymous-pattern-";

impl AnonymousNames {
    pub(super) fn next(&mut self) -> String {
        let name = format!("{PREFIX}{}", self.next);
        self.next += 1;
        name
    }
}

pub(super) fn is_anonymous(name: &str) -> bool {
    name.starts_with(PREFIX)
}
