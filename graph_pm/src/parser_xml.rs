use std::fs;

use serde_xml_rs;

use crate::structure_xml::Network;

pub fn parse_xml_to_structure(path: &str) -> Network {
    let xml_data = fs::read_to_string(path).expect("");
    let network: Network = serde_xml_rs::from_str(&xml_data).expect("Ошибка при разборе XML");
    network
}