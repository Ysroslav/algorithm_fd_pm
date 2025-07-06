use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Instant;

use crate::graph::Graph;

mod structure_xml;
mod parser_xml;
mod graph;


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open("output_pm_real.txt")?;

    let nets: [String; 24] = [
        String::from("abilene"),
        String::from("atlanta"),
        String::from("brain"),
        String::from("cost266"),
        String::from("dfn-bwin"),
        String::from("dfn-gwin"),
        String::from("di-yuan"),
        String::from("france"),
        String::from("geant"),
        String::from("germany50"),
        String::from("giul39"),
        String::from("india35"),
        String::from("newyork"),
        String::from("nobel-eu"),
        String::from("nobel-germany"),
        String::from("norway"),
        String::from("pdh"),
        String::from("pioro40"),
        String::from("polska"),
        String::from("sun"),
        String::from("ta1"),
        String::from("ta2"),
        String::from("zib54"),
        String::from("nobel-us"),
    ];
    for str in &nets {
        let xml_content = fs::read_to_string(format!("C:\\Users\\Dell\\mipt\\sndlib_xml\\{}\\{}.xml", str, str))?;
        let mut graph = Graph::from_xml(&xml_content)?;

        let vertex = graph.nodes.len();
        let edges = graph.edges.len();
        let commodities = graph.commodities.iter().len();

        //Запускаем метод проекции
        let start = Instant::now();
        let res = graph.projection_method(100000, 0.0001);
        let duration = start.elapsed();
        println!("Метод выполнился за;{}", duration.as_millis());
        let result = format!("{};{};{};{};{};{}\n", vertex, edges, commodities,
                             res.0, res.1, res.2);
        file.write_all(result.as_bytes())?;
        println!("Метод выполнился для {}", str);
    }

    Ok(())
}

