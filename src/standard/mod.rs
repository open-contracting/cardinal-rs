use std::collections::HashSet;
use std::sync::LazyLock;

pub static BID_STATUS: LazyLock<HashSet<&str>> =
    LazyLock::new(|| HashSet::from(["invited", "pending", "valid", "disqualified", "withdrawn"]));
