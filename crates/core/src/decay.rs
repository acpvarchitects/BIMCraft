//! The words for how a material has decayed: one list, shared by Heritage Parts and Survey Areas.

use serde::{Deserialize, Serialize};

/// How a part's material has decayed: one of the words conservation uses for the forms stone, plaster and masonry
/// decay in (the words alone: what each means is the surveyor's to know), or the project's own.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decay {
    Alveolization,
    BiologicalColonization,
    Blistering,
    Crack,
    Crust,
    Deformation,
    Delamination,
    Deposit,
    Detachment,
    Discolouration,
    Disintegration,
    Efflorescence,
    Encrustation,
    Erosion,
    Exfoliation,
    Fragmentation,
    Graffiti,
    MissingPart,
    MoistArea,
    Patina,
    Peeling,
    Pitting,
    Scaling,
    Soiling,
    Staining,
    Vegetation,
    /// One the list has no word for, in the project's own words.
    Custom(String),
}

impl Decay {
    /// Every one the list names, in its order.
    pub const NAMED: [Decay; 26] = [
        Decay::Alveolization,
        Decay::BiologicalColonization,
        Decay::Blistering,
        Decay::Crack,
        Decay::Crust,
        Decay::Deformation,
        Decay::Delamination,
        Decay::Deposit,
        Decay::Detachment,
        Decay::Discolouration,
        Decay::Disintegration,
        Decay::Efflorescence,
        Decay::Encrustation,
        Decay::Erosion,
        Decay::Exfoliation,
        Decay::Fragmentation,
        Decay::Graffiti,
        Decay::MissingPart,
        Decay::MoistArea,
        Decay::Patina,
        Decay::Peeling,
        Decay::Pitting,
        Decay::Scaling,
        Decay::Soiling,
        Decay::Staining,
        Decay::Vegetation,
    ];

    /// As listings, schedules and IFC show it: `Biological colonization`; a custom one, its own text.
    pub fn label(&self) -> &str {
        match self {
            Decay::Alveolization => "Alveolization",
            Decay::BiologicalColonization => "Biological colonization",
            Decay::Blistering => "Blistering",
            Decay::Crack => "Crack",
            Decay::Crust => "Crust",
            Decay::Deformation => "Deformation",
            Decay::Delamination => "Delamination",
            Decay::Deposit => "Deposit",
            Decay::Detachment => "Detachment",
            Decay::Discolouration => "Discolouration",
            Decay::Disintegration => "Disintegration",
            Decay::Efflorescence => "Efflorescence",
            Decay::Encrustation => "Encrustation",
            Decay::Erosion => "Erosion",
            Decay::Exfoliation => "Exfoliation",
            Decay::Fragmentation => "Fragmentation",
            Decay::Graffiti => "Graffiti",
            Decay::MissingPart => "Missing part",
            Decay::MoistArea => "Moist area",
            Decay::Patina => "Patina",
            Decay::Peeling => "Peeling",
            Decay::Pitting => "Pitting",
            Decay::Scaling => "Scaling",
            Decay::Soiling => "Soiling",
            Decay::Staining => "Staining",
            Decay::Vegetation => "Vegetation",
            Decay::Custom(text) => text,
        }
    }

    /// One the list names, by its label or its name in files, whatever its case, spaces, hyphens or underscores
    /// (`Missing part`, `missing-part`, `MISSING_PART`), or as American English spells it (`discoloration`).
    pub fn named(text: &str) -> Option<Decay> {
        let key = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
        let wanted = match key(text).as_str() {
            "discoloration" => "discolouration".to_string(),
            "alveolisation" => "alveolization".to_string(),
            "biologicalcolonisation" => "biologicalcolonization".to_string(),
            other => other.to_string(),
        };
        Decay::NAMED.into_iter().find(|d| key(d.label()) == wanted)
    }
}
