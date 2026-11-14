use std::collections::HashSet;

// tags as people type them
//
// a tag is a single word, so any whitespace or common punctuation between words separates
// tags: "beach, summer|trip; #family" is {beach, summer, trip, family}.  the backend gets a
// set (the api carries HashSet<String>), and folding it into its own stored form -- the |
// separated string in mariadb -- stays the backend's business

// what separates tags in the inputs
fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, ',' | ';' | '|' | '/' | '\\')
}

pub fn parse_tags(input: &str) -> HashSet<String> {
    input
        .split(is_separator)
        .map(|t| t.trim_start_matches('#'))
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

// the tags of a record for display or for prefilling an input, sorted so that the text is
// stable; parse_tags reads it back as the same set
pub fn format_tags(tags: &HashSet<String>) -> String {
    let mut tags: Vec<&str> = tags.iter().map(String::as_str).collect();
    tags.sort_unstable();
    tags.join(", ")
}

pub const TAG_PLACEHOLDER: &str = "e.g. beach, summer_trip, family";

pub const TAG_HELP: &str = "Separate tags with spaces, commas or other punctuation. \
     A tag can't contain spaces, so use underscores for multi-word tags (summer_trip).";

#[cfg(test)]
mod tests {
    use super::*;

    fn set(tags: &[&str]) -> HashSet<String> {
        tags.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn any_separator_works() {
        assert_eq!(
            parse_tags("beach, summer|trip; #family"),
            set(&["beach", "summer", "trip", "family"])
        );
        assert_eq!(
            parse_tags("a b\tc\nd/e\\f"),
            set(&["a", "b", "c", "d", "e", "f"])
        );
        assert_eq!(parse_tags("  ,, | ; "), set(&[]));
        assert_eq!(parse_tags("a,a, a"), set(&["a"]));
    }

    #[test]
    fn tag_characters_are_kept() {
        assert_eq!(
            parse_tags("summer_trip ct-mod7 v1.2"),
            set(&["summer_trip", "ct-mod7", "v1.2"])
        );
    }

    #[test]
    fn format_round_trips() {
        let tags = set(&["zebra", "apple", "summer_trip"]);
        assert_eq!(format_tags(&tags), "apple, summer_trip, zebra");
        assert_eq!(parse_tags(&format_tags(&tags)), tags);
        assert_eq!(format_tags(&set(&[])), "");
    }
}
