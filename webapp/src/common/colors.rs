// First, let's define the color types that will be used across components
// Add this to a new file: webapp/src/common/colors.rs

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CollectionColor {
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Gray,
    Teal,
    Indigo,
}

impl CollectionColor {
    pub fn all() -> Vec<Self> {
        vec![
            Self::Red,
            Self::Orange,
            Self::Yellow,
            Self::Green,
            Self::Blue,
            Self::Purple,
            Self::Pink,
            Self::Gray,
            Self::Teal,
            Self::Indigo,
        ]
    }

    pub fn to_css_color(self) -> &'static str {
        match self {
            Self::Red => "#EF4444",
            Self::Orange => "#F97316",
            Self::Yellow => "#EAB308",
            Self::Green => "#22C55E",
            Self::Blue => "#3B82F6",
            Self::Purple => "#8B5CF6",
            Self::Pink => "#EC4899",
            Self::Gray => "#6B7280",
            Self::Teal => "#14B8A6",
            Self::Indigo => "#6366F1",
        }
    }

    pub fn to_light_css_color(self) -> &'static str {
        match self {
            Self::Red => "#FEF2F2",
            Self::Orange => "#FFF7ED",
            Self::Yellow => "#FEFCE8",
            Self::Green => "#F0FDF4",
            Self::Blue => "#EFF6FF",
            Self::Purple => "#F5F3FF",
            Self::Pink => "#FDF2F8",
            Self::Gray => "#F9FAFB",
            Self::Teal => "#F0FDFA",
            Self::Indigo => "#EEF2FF",
        }
    }
}

impl fmt::Display for CollectionColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Yellow => "Yellow",
            Self::Green => "Green",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
            Self::Gray => "Gray",
            Self::Teal => "Teal",
            Self::Indigo => "Indigo",
        };
        write!(f, "{}", name)
    }
}

impl From<String> for CollectionColor {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Red" => Self::Red,
            "Orange" => Self::Orange,
            "Yellow" => Self::Yellow,
            "Green" => Self::Green,
            "Blue" => Self::Blue,
            "Purple" => Self::Purple,
            "Pink" => Self::Pink,
            "Teal" => Self::Teal,
            "Indigo" => Self::Indigo,
            _ => Self::Gray,
        }
    }
}
