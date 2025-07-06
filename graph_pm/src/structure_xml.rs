use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename = "network")]
pub struct Network {
    #[serde(rename = "networkStructure")]
    pub(crate) network_structure: NetworkStructure,
    pub(crate) demands: Demands,
}

#[derive(Debug, Deserialize)]
pub struct NetworkStructure {
    pub(crate) nodes: Nodes,
    pub(crate) links: Links,
}

#[derive(Debug, Deserialize)]
pub struct Nodes {
    #[serde(rename = "node")]
    pub(crate) nodes: Vec<Node>,
}

#[derive(Debug, Deserialize)]
pub struct Node {
    #[serde(rename = "id")]
    pub(crate) id: String,
}

#[derive(Debug, Deserialize)]
pub struct Links {
    #[serde(rename = "link")]
    pub(crate) links: Vec<Link>,
}

#[derive(Debug, Deserialize)]
pub struct Link {
    #[serde(rename = "id")]
    id: String,
    pub(crate) source: String,
    pub(crate) target: String,
    #[serde(rename = "preInstalledModule")]
    pub(crate) capacity: Option<PreInstalledModule>,
    #[serde(rename = "additionalModules")]
    pub(crate) modules: Option<AdditionalModules>,
}

#[derive(Debug, Deserialize)]
pub struct PreInstalledModule {
    pub(crate) capacity: f64,
}

#[derive(Debug, Deserialize)]
pub struct Demands {
    #[serde(rename = "demand")]
    pub(crate) demands: Vec<Demand>,
}

#[derive(Debug, Deserialize)]
pub struct Demand {
    #[serde(rename = "id")]
    id: String,
    pub(crate) source: String,
    pub(crate) target: String,
    #[serde(rename = "demandValue")]
    pub(crate) value: f64,
}

#[derive(Debug, Deserialize)]
pub struct AdditionalModules {
    #[serde(rename = "addModule")]
    pub(crate) add_module: Vec<AddModule>,
}

#[derive(Debug, Deserialize)]
pub struct AddModule {
    pub(crate) capacity: f64,
}
