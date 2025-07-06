use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::collections::HashMap;

use ordered_float::OrderedFloat;

use crate::functions::functions::{modified_delay_function, modified_delay_gradient};
use crate::structure::structure::Network;

#[derive(Clone, Eq, PartialEq)]
struct State {
    cost: OrderedFloat<f64>,
    node: usize,
}

// Реализация для BinaryHeap с использованием OrderedFloat
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cost.cmp(&other.cost).reverse()
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub(crate) edges: Vec<usize>,
    pub(crate) flow: f64,
}

impl Path {
    pub fn new(edges: Vec<usize>) -> Self {
        Self {
            edges,
            flow: 0.0,  // начальный поток равен 0
        }
    }

    // Добавим метод для создания пути с заданным потоком
    pub fn with_flow(edges: Vec<usize>, flow: f64) -> Self {
        Self {
            edges,
            flow,
        }
    }

    fn eq(&self, other: &Self) -> bool {
        self.edges == other.edges
    }
}

pub fn has_duplicate_path(paths: &Vec<Path>, new_path: &Path) -> bool {
    paths.iter().any(|path| path.edges == new_path.edges)
}

pub fn add_path_if_unique(commodity: &mut Commodity, new_path: Path) {
    // Проверяем, существует ли уже такой путь
    let path_exists = commodity.paths.iter().any(|p| p.edges == new_path.edges);

    if !path_exists {
        commodity.paths.push(new_path);
    }
}


pub struct Graph {
    pub(crate) nodes: HashMap<String, usize>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) commodities: Vec<Commodity>,
    call_dejkstra: usize,
}

pub struct Edge {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) capacity: f64,
    pub(crate) flow: f64,
    pub index: usize,  // добавляем индекс ребра
    pub cost_a: f64, // расходы для квадратичной функции, для квадрата потока
    pub cost_b: f64, // расходы для квадратичной функции, для значения потока
}

impl Edge {
    pub fn new(from: usize, to: usize, capacity: f64, index: usize, cost_a: f64, cost_b: f64) -> Self {
        Self {
            from,
            to,
            capacity,
            flow: 0.0,
            index,
            cost_a,
            cost_b,
        }
    }
}

pub struct Commodity {
    pub(crate) source: usize,
    pub(crate) target: usize,
    pub(crate) demand: f64,
    pub(crate) paths: Vec<Path>,
}

impl Graph {
    pub(crate) fn from_xml(xml: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let network: Network = serde_xml_rs::from_str(xml)?;

        // Создаем отображение имен узлов в индексы
        let mut nodes = HashMap::new();
        for (idx, node) in network.network_structure.nodes.nodes.iter().enumerate() {
            nodes.insert(node.id.clone(), idx);
        }

        // Создаем ребра графа
        let mut edges = Vec::new();
        for (idx, link) in network.network_structure.links.links.iter().enumerate() {
            let from = nodes[&link.source];
            let to = nodes[&link.target];
            let capacity = match &link.capacity {
                Some(value) => value.capacity,
                None => {
                    match &link.modules {
                        Some(module) => module.add_module[0].capacity,
                        None => 0f64,
                    }
                }
            };

            let cost_b = match &link.routing_cost {
                Some(value) => *value,
                None => {
                    match &link.capacity {
                        Some(module) => module.cost,
                        None => 0f64,
                    }
                }
            };

            let cost_a = match &link.modules {
                Some(module) => module.add_module[0].cost,
                None => 0f64,
            };

            // Добавляем индексы при создании рёбер
            edges.push(Edge::new(from, to, capacity, idx * 2, cost_a, cost_b));
            edges.push(Edge::new(to, from, capacity, idx * 2 + 1, cost_a, cost_b));
        }

        // Создаем список товаров (commodities)
        let mut commodities = Vec::new();
        for demand in network.demands.demands.iter() {
            commodities.push(Commodity {
                source: nodes[&demand.source],
                target: nodes[&demand.target],
                demand: demand.value,
                paths: Vec::new(),
            });
        }

        Ok(Graph {
            nodes,
            edges,
            commodities,
            call_dejkstra: 0usize,
        })
    }

    // Дополнительно: функция для проверки существования пути
    fn path_exists(&self, source: usize, target: usize) -> bool {
        let n = self.nodes.len();
        let mut visited = vec![false; n];
        let mut queue = std::collections::VecDeque::new();

        queue.push_back(source);
        visited[source] = true;

        while let Some(vertex) = queue.pop_front() {
            if vertex == target {
                return true;
            }

            for (edge_idx, edge) in self.edges.iter().enumerate() {
                if edge.from == vertex && !visited[edge.to] && edge.flow < edge.capacity {
                    visited[edge.to] = true;
                    queue.push_back(edge.to);
                }
            }
        }

        false
    }

    fn calculate_path_length(&self, k: usize, p: usize) -> f64 {
        let path = &self.commodities[k].paths[p];
        path.edges.iter()
            .map(|&edge_idx| self.delay_function(edge_idx))
            .sum()
    }


    fn calculate_step_size(&self, t: usize) -> f64 {
        // Реализация правила Армихо
        let beta: f64 = 0.5;
        let sigma = 0.1;
        let initial_step: f64 = 1.0;

        let mut step = initial_step;

        // Можно добавить более сложную логику выбора шага
        step * beta.powi(t as i32)
    }

    fn delay_function(&self, edge_idx: usize) -> f64 {
        let edge = &self.edges[edge_idx];
        let rho_val = 0.99; // Consider making this dynamic, e.g., from FlowDeviation struct
        modified_delay_function(edge.flow, edge.capacity, rho_val)
    }

    fn derivative_delay(&self, edge_idx: usize) -> f64 {
        let edge = &self.edges[edge_idx];
        let rho_val = 0.99; // Consider making this dynamic
        modified_delay_gradient(edge.flow, edge.capacity, rho_val)
    }


    fn calculate_shortest_path_length(&self, k: usize) -> f64 {
        let commodity = &self.commodities[k];

        // Кратчайший путь - последний в списке путей
        if let Some(shortest_path) = commodity.paths.last() {
            // Суммируем задержки по всем ребрам кратчайшего пути
            shortest_path.edges.iter()
                .map(|&edge_idx| self.delay_function(edge_idx))
                .sum()
        } else {
            f64::INFINITY // Если путей еще нет
        }
    }

    pub fn find_shortest_path_with_derivatives_weights(&mut self, source: usize, target: usize, derivatives: &HashMap<usize, f64>) -> Option<Path> {
        self.call_dejkstra += 1;

        let n = self.nodes.len();
        let mut dist: Vec<OrderedFloat<f64>> = vec![OrderedFloat(f64::INFINITY); n];
        let mut prev: Vec<Option<usize>> = vec![None; n];
        let mut prev_edge: Vec<Option<usize>> = vec![None; n];
        let mut visited = vec![false; n];
        let mut heap = BinaryHeap::new();

        // Инициализация расстояния для исходной вершины
        dist[source] = OrderedFloat(0.0);
        heap.push(State {
            cost: OrderedFloat(0.0),
            node: source,
        });

        while let Some(State { node, cost }) = heap.pop() {
            if node == target {
                break;
            }

            if visited[node] {
                continue;
            }
            visited[node] = true;

            // Проверяем все исходящие ребра из текущей вершины
            for (edge_idx, edge) in self.edges.iter().enumerate() {
                if edge.from != node {
                    continue;
                }

                let v = edge.to;
                if visited[v] {
                    continue;
                }

                // Используем производную функции задержки как вес ребра
                let weight = OrderedFloat(*derivatives.get(&edge_idx).unwrap());
                let new_cost = cost + weight;

                if new_cost < dist[v] {
                    dist[v] = new_cost;
                    prev[v] = Some(node);
                    prev_edge[v] = Some(edge_idx);
                    heap.push(State {
                        cost: new_cost,
                        node: v,
                    });
                }
            }
        }

        // Если целевая вершина недостижима
        if dist[target] == OrderedFloat(f64::INFINITY) {
            return None;
        }

        // Восстанавливаем путь
        let mut path_edges = Vec::new();
        let mut current = target;
        while let Some(previous) = prev[current] {
            if let Some(edge_idx) = prev_edge[current] {
                path_edges.push(edge_idx);
            }
            current = previous;
        }

        // Разворачиваем путь для получения правильного порядка (от источника к цели)
        path_edges.reverse();

        Some(Path::new(path_edges))
    }

    pub fn get_call_dejkstra(&self) -> usize {
        self.call_dejkstra
    }
}