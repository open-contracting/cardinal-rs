use std::collections::HashSet;
use std::sync::LazyLock;

// https://extensions.open-contracting.org/en/extensions/bids/master/codelists/#bidStatus.csv
pub static BID_STATUS: LazyLock<HashSet<&str>> =
    LazyLock::new(|| HashSet::from(["invited", "pending", "valid", "disqualified", "withdrawn"]));

// https://standard.open-contracting.org/latest/en/schema/codelists/#award-status
pub static AWARD_STATUS: LazyLock<HashSet<&str>> =
    LazyLock::new(|| HashSet::from(["pending", "active", "cancelled", "unsuccessful"]));
