use std::collections::HashSet;

enum SearchMode {
    SubstringCombined {
        filter: HashSet<String>
    },
    Fulltext {
        filter: String
    },

}
