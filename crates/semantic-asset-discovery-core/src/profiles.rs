use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfilePack {
    pub id: String,
    pub label: String,
    pub version: Option<String>,
    pub standard_iri: String,
    #[serde(default)]
    pub artifact_kinds: Vec<String>,
    #[serde(default)]
    pub link_predicates: Vec<String>,
}

const BUILT_IN_PROFILE_PACKS: &[&str] = &[
    include_str!("../data/profiles/dcat-ap.toml"),
    include_str!("../data/profiles/breg-dcat-ap.toml"),
    include_str!("../data/profiles/ogc-api-records.toml"),
    include_str!("../data/profiles/prof.toml"),
    include_str!("../data/profiles/shacl.toml"),
    include_str!("../data/profiles/json-schema.toml"),
    include_str!("../data/profiles/openapi.toml"),
];

pub fn built_in_profile_packs() -> Result<Vec<ProfilePack>, toml::de::Error> {
    BUILT_IN_PROFILE_PACKS
        .iter()
        .map(|pack| parse_profile_pack(pack))
        .collect()
}

pub fn parse_profile_pack(input: &str) -> Result<ProfilePack, toml::de::Error> {
    toml::from_str(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_profile_packs_are_data_driven() {
        let packs = built_in_profile_packs().expect("profile packs parse");
        let ids: Vec<_> = packs.iter().map(|pack| pack.id.as_str()).collect();
        assert!(ids.contains(&"dcat-ap"));
        assert!(ids.contains(&"breg-dcat-ap"));
        assert!(ids.contains(&"ogc-api-records"));
        assert!(ids.contains(&"prof"));
        assert!(ids.contains(&"shacl"));
        assert!(ids.contains(&"json-schema"));
        assert!(ids.contains(&"openapi"));
    }
}
