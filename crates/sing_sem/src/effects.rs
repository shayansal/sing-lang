use std::collections::HashSet;

pub(crate) const ALL: &[&str] = &[
    "h", "d", "g", "z", "fr", "fw", "ir", "iw", "nr", "nw", "dr", "dw", "tm", "rn", "th", "at",
    "ff", "bk", "sy",
];

const SYSTEM_FAMILY: &[&str] = &[
    "fr", "fw", "ir", "iw", "nr", "nw", "dr", "dw", "tm", "rn", "th", "ff", "bk", "sy",
];

pub(crate) fn is_known(effect: &str) -> bool {
    ALL.contains(&effect)
}

pub(crate) fn is_allowed(declared: &HashSet<String>, used: &str) -> bool {
    declared
        .iter()
        .any(|declared| covers(declared.as_str(), used))
}

pub(crate) fn covers(declared: &str, used: &str) -> bool {
    declared == used || (declared == "sy" && SYSTEM_FAMILY.contains(&used))
}
