use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::collections::BinaryHeap;

// Добавлен VecDeque для BFS
use crate::structure_xml::Network;

// Предполагается, что эта структура корректно определена

pub type ResultExec = (usize, usize, usize);

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

    // Переопределяем eq для Path, чтобы сравнивать только по ребрам
    fn eq(&self, other: &Self) -> bool {
        self.edges == other.edges
    }
}

// Вспомогательная функция для проверки наличия дубликата пути
pub fn has_duplicate_path(paths: &Vec<Path>, new_path: &Path) -> bool {
    paths.iter().any(|path| path.edges == new_path.edges)
}

// Вспомогательная функция для добавления пути, если он уникален
pub fn add_path_if_unique(commodity: &mut Commodity, new_path: Path) {
    // Проверяем, существует ли уже такой путь
    let path_exists = commodity.paths.iter().any(|p| p.edges == new_path.edges);

    if !path_exists {
        commodity.paths.push(new_path);
    }
}

// Обертка для сравнения f64, необходимая для BinaryHeap
#[derive(Copy, Clone)]
struct OrderedFloat(f64);

impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0.partial_cmp(&other.0) == Some(Ordering::Equal)
    }
}

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

// Структура для приоритетной очереди в алгоритме Дейкстры
#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: OrderedFloat,
    vertex: usize,
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Сравнение по стоимости (обратный порядок для min-heap)
        other.cost.cmp(&self.cost)
            // Затем по индексу вершины для детерминированности
            .then_with(|| self.vertex.cmp(&other.vertex))
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Структура, представляющая граф
pub struct Graph {
    pub(crate) nodes: HashMap<String, usize>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) commodities: Vec<Commodity>,
    pub(crate) count_dejkstra: usize,
    pub(crate) adj_list: Vec<Vec<(usize, usize)>>, // (next_vertex, edge_idx) - Список смежности
}

// Структура, представляющая ребро в графе
pub struct Edge {
    pub(crate) from: usize,
    pub(crate) to: usize,
    pub(crate) capacity: f64,
    pub(crate) flow: f64,
    pub index: usize,  // добавляем индекс ребра
}

impl Edge {
    pub fn new(from: usize, to: usize, capacity: f64, index: usize) -> Self {
        Self {
            from,
            to,
            capacity,
            flow: 0.0,
            index,
        }
    }
}

// Структура, представляющая товар (поток)
pub struct Commodity {
    pub(crate) source: usize,
    pub(crate) target: usize,
    pub(crate) demand: f64,
    pub(crate) paths: Vec<Path>,
}

impl Graph {
    // Создание графа из XML-строки
    pub(crate) fn from_xml(xml: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let network: Network = serde_xml_rs::from_str(xml)?;

        // Создаем отображение имен узлов в индексы
        let mut nodes = HashMap::new();
        for (idx, node) in network.network_structure.nodes.nodes.iter().enumerate() {
            nodes.insert(node.id.clone(), idx);
        }

        // Создаем ребра графа
        let mut edges = Vec::new();
        // Временный список смежности для построения
        let mut temp_adj_list: Vec<Vec<(usize, usize)>> = vec![Vec::new(); nodes.len()];

        for (idx, link) in network.network_structure.links.links.iter().enumerate() {
            let from = nodes[&link.source];
            let to = nodes[&link.target];
            let capacity = match &link.capacity {
                Some(value) => value.capacity,
                None => {
                    match &link.modules {
                        Some(module) => module.add_module[0].capacity,
                        None => return Err("Capacity not found for link".into()), // Возвращаем ошибку
                    }
                }
            };

            // Добавляем индексы при создании рёбер
            // Каждое логическое ребро из XML может быть представлено двумя направленными ребрами
            let edge_idx_forward = idx * 2;
            let edge_idx_backward = idx * 2 + 1;

            edges.push(Edge::new(from, to, capacity, edge_idx_forward));
            edges.push(Edge::new(to, from, capacity, edge_idx_backward));

            // Заполняем список смежности
            temp_adj_list[from].push((to, edge_idx_forward));
            temp_adj_list[to].push((from, edge_idx_backward));
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
            count_dejkstra: 0usize,
            adj_list: temp_adj_list, // Инициализируем adj_list один раз
        })
    }

    // Основной метод проекции
    pub fn projection_method(&mut self, max_iterations: usize, tolerance: f64) -> ResultExec {
        // Инициализация начальных потоков (можно добавить более сложную инициализацию)
        for k in 0..self.commodities.len() {
            if let Some(initial_path) = self.find_initial_path( // Используем find_initial_path для первого пути
                                                                self.commodities[k].source,
                                                                self.commodities[k].target,
            ) {
                // Добавляем начальный путь и устанавливаем весь спрос на него
                self.commodities[k].paths.push(initial_path);
                self.commodities[k].paths[0].flow = self.commodities[k].demand;

                // Обновляем потоки на ребрах
                for &edge_idx in &self.commodities[k].paths[0].edges {
                    self.edges[edge_idx].flow += self.commodities[k].demand;
                }
            } else {
                println!("Не удалось найти начальный путь для товара от {} до {}",
                         self.commodities[k].source, self.commodities[k].target);
            }
        }


        let mut i = 0;
        let mut path_count = 0usize;
        for t in 0..max_iterations {
            let mut max_change: f64 = 0.0;

            // Для каждого товара
            for k in 0..self.commodities.len() {
                // 1. Находим кратчайший путь с учетом текущих задержек (используем производные)
                let shortest_path = self.find_shortest_path_with_derivatives(
                    self.commodities[k].source,
                    self.commodities[k].target,
                );

                if let Some(path) = shortest_path {
                    // Добавляем новый путь, если он еще не существует
                    if !self.commodities[k].paths.contains(&path) {
                        self.commodities[k].paths.push(path);
                    }

                    // 2. Обновляем потоки на путях
                    let (change, _) = self.update_path_flows(k, t);
                    max_change = max_change.max(change);
                } else {
                    println!("Не удалось найти кратчайший путь для товара {} от {} до {}",
                             k, self.commodities[k].source, self.commodities[k].target);
                }
            }

            let path_count_i = self.commodities.iter().flat_map(|c| c.paths.iter()).filter(|p| p.flow > 0.0).count();
            if path_count < path_count_i {
                path_count = path_count_i;
            }

            println!("Итерация {}; Максимальное изменение потока: {}", t, max_change);

            // Проверяем сходимость
            if max_change < tolerance {
                println!("Сходимость достигнута на итерации {}. Максимальное изменение потока: {}", t, max_change);
                break;
            }
            i += 1;
        }
        println!("Количество вызовов метода Дейкстры: {}", self.count_dejkstra);
        (i as usize, self.count_dejkstra, path_count)
    }

    // Поиск кратчайшего пути с использованием функции задержки (для инициализации)
    pub(crate) fn find_shortest_path(&mut self, source: usize, target: usize) -> Option<Path> {
        self.count_dejkstra += 1;
        let n = self.nodes.len();
        let mut distances = vec![f64::INFINITY; n];
        let mut previous = vec![None; n]; // (previous_vertex, edge_index_to_current)
        let mut heap = BinaryHeap::new();

        // Инициализация начальной вершины
        distances[source] = 0.0;
        heap.push(State {
            cost: OrderedFloat(0.0),
            vertex: source,
        });

        while let Some(State { cost, vertex }) = heap.pop() {
            // Если достигли целевой вершины
            if vertex == target {
                break;
            }

            // Если нашли худший путь до этой вершины, пропускаем
            if cost.0 > distances[vertex] {
                continue;
            }

            // Просматриваем соседей, используя предрассчитанный adj_list
            for &(next_vertex, edge_idx) in &self.adj_list[vertex] {
                let edge = &self.edges[edge_idx];

                // Проверяем, что ребро имеет достаточную пропускную способность (если это важно для инициализации)
                // Для инициализации, возможно, не нужно проверять flow < capacity,
                // но для общего случая это важно.
                /*if edge.flow >= edge.capacity - f64::EPSILON { // Используем EPSILON для сравнения с плавающей точкой
                    continue;
                }*/

                let next_cost = cost.0 + self.delay_function(edge_idx);

                if next_cost < distances[next_vertex] {
                    distances[next_vertex] = next_cost;
                    heap.push(State {
                        cost: OrderedFloat(next_cost),
                        vertex: next_vertex,
                    });
                    previous[next_vertex] = Some((vertex, edge_idx));
                }
            }
        }

        // Если путь до целевой вершины не найден
        if distances[target] == f64::INFINITY {
            return None;
        }

        // Восстанавливаем путь
        let mut path_edges = Vec::new();
        let mut current = target;
        while let Some((prev, edge_idx)) = previous[current] {
            path_edges.push(edge_idx);
            current = prev;
        }

        path_edges.reverse();
        Some(Path { edges: path_edges, flow: 0.0 })
    }

    // Поиск кратчайшего пути с использованием производных функции задержки
    pub fn find_shortest_path_with_derivatives(&mut self, source: usize, target: usize) -> Option<Path> {
        self.count_dejkstra += 1;
        let n = self.nodes.len();
        let mut distances = vec![f64::INFINITY; n];
        let mut previous = vec![None; n]; // (previous_vertex, edge_index_to_current)
        let mut heap = BinaryHeap::new();

        distances[source] = 0.0;
        heap.push(State {
            cost: OrderedFloat(0.0),
            vertex: source,
        });

        while let Some(State { cost, vertex }) = heap.pop() {
            // Прерываем, если достигли целевой вершины
            if vertex == target {
                break;
            }

            // Пропускаем, если найден более дешёвый путь (РАСКОММЕНТИРОВАНО)
            if cost.0 > distances[vertex] {
                continue;
            }

            // Просматриваем все соседние вершины, используя предрассчитанный adj_list
            for &(next_vertex, edge_idx) in &self.adj_list[vertex] {
                let edge = &self.edges[edge_idx];

                // Если ребро полностью загружено, его нельзя использовать
                /*if edge.flow >= edge.capacity - f64::EPSILON {
                    continue;
                }*/

                // Вычисляем производную функции задержки как стоимость ребра
                let derivative_cost = self.derivative_delay(edge_idx);

                // Вычисляем новую стоимость пути
                let next_cost = cost.0 + derivative_cost;

                // Обновляем стоимость, если нашли более дешевый путь
                if next_cost < distances[next_vertex] {
                    distances[next_vertex] = next_cost;
                    heap.push(State {
                        cost: OrderedFloat(next_cost),
                        vertex: next_vertex,
                    });
                    previous[next_vertex] = Some((vertex, edge_idx));
                }
            }
        }

        // Если путь не найден, возвращаем None
        if distances[target] == f64::INFINITY {
            return None;
        }

        // Восстанавливаем путь
        let mut path_edges = Vec::new();
        let mut current = target;
        while let Some((prev, edge_idx)) = previous[current] {
            path_edges.push(edge_idx);
            current = prev;
        }

        path_edges.reverse();
        Some(Path::new(path_edges))
    }

    // Поиск начального пути (например, по минимальному количеству ребер)
    pub fn find_initial_path(&mut self, source: usize, target: usize) -> Option<Path> {
        self.count_dejkstra += 1;
        let n = self.nodes.len();
        let mut distances = vec![f64::INFINITY; n];
        let mut previous = vec![None; n];
        let mut queue = VecDeque::new(); // Используем BFS для поиска кратчайшего пути по количеству ребер

        distances[source] = 0.0;
        queue.push_back(source);

        while let Some(vertex) = queue.pop_front() {
            if vertex == target {
                break;
            }

            for &(next_vertex, edge_idx) in &self.adj_list[vertex] {
                let edge = &self.edges[edge_idx];
                // Для начального пути, возможно, не нужно учитывать поток, только пропускную способность
                /*if edge.capacity <= f64::EPSILON { // Ребро не имеет пропускной способности
                    continue;
                }*/

                // Используем постоянный вес для всех ребер (1.0)
                let next_cost = distances[vertex] + 1.0;

                if next_cost < distances[next_vertex] {
                    distances[next_vertex] = next_cost;
                    queue.push_back(next_vertex);
                    previous[next_vertex] = Some((vertex, edge_idx));
                }
            }
        }

        if distances[target] == f64::INFINITY {
            return None;
        }

        let mut path_edges = Vec::new();
        let mut current = target;
        while let Some((prev, edge_idx)) = previous[current] {
            path_edges.push(edge_idx);
            current = prev;
        }

        path_edges.reverse();
        Some(Path::new(path_edges))
    }


    // Обновление потоков по путям для одного товара
    fn update_path_flows(&mut self, k: usize, t: usize) -> (f64, f64) {
        let alpha_t = self.calculate_step_size(t);
        let commodity_demand = self.commodities[k].demand;

        // Временно забираем пути, чтобы избежать проблем с заимствованием
        let mut paths_for_commodity = std::mem::take(&mut self.commodities[k].paths);
        let num_paths = paths_for_commodity.len();

        if num_paths == 0 {
            // Если путей нет, нечего обновлять. Возвращаем 0 изменение.
            self.commodities[k].paths = paths_for_commodity; // Возвращаем пустые пути
            return (0.0, alpha_t);
        }

        // Находим кратчайший путь среди *активных* путей
        let mut min_path_length = f64::INFINITY;
        let mut shortest_path_idx_in_active_paths = 0;

        for p_idx in 0..num_paths {
            let path_length = self.calculate_path_length(k, p_idx, &paths_for_commodity);
            if path_length < min_path_length {
                min_path_length = path_length;
                shortest_path_idx_in_active_paths = p_idx;
            }
        }
        let d_kp_bar = min_path_length; // Длина кратчайшего пути среди активных

        let mut path_updates_info = Vec::new(); // (path_idx, new_flow)
        let mut total_flow_before_correction = 0.0;

        // Обновляем потоки для всех путей, кроме того, который будет корректировать общий спрос
        // (обычно это кратчайший путь)
        for p_idx in 0..num_paths {
            let d_kp = self.calculate_path_length(k, p_idx, &paths_for_commodity);
            let h_kp = self.calculate_h_kp(k, p_idx, &paths_for_commodity);

            // Избегаем деления на ноль, если h_kp очень мало или равно нулю
            let step_term = if h_kp.abs() < f64::EPSILON {
                0.0 // Если вторая производная нулевая, шаг не корректируется этим членом
            } else {
                alpha_t / h_kp * (d_kp - d_kp_bar)
            };

            let old_flow = paths_for_commodity[p_idx].flow;
            let new_flow = (old_flow - step_term).max(0.0); // Поток не может быть отрицательным
            path_updates_info.push((p_idx, new_flow));
            total_flow_before_correction += new_flow;
        }

        let mut max_change: f64 = 0.0;

        // Откатываем изменения потоков на ребрах, чтобы применить новые
        // Это необходимо, так как мы будем пересчитывать потоки на ребрах
        // на основе новых потоков по путям.
        for p_idx in 0..num_paths {
            let path = &paths_for_commodity[p_idx];
            for &edge_idx in &path.edges {
                self.edges[edge_idx].flow -= path.flow;
            }
        }

        // Применяем новые потоки по путям (без коррекции спроса)
        for (p_idx, new_flow) in path_updates_info {
            let old_flow = paths_for_commodity[p_idx].flow;
            let flow_change = new_flow - old_flow;
            max_change = max_change.max(flow_change.abs());
            paths_for_commodity[p_idx].flow = new_flow;
        }

        // Корректируем поток по кратчайшему пути для соблюдения спроса
        let current_total_flow = paths_for_commodity.iter().map(|p| p.flow).sum::<f64>();
        let flow_deficit = commodity_demand - current_total_flow;

        let shortest_path_mut = &mut paths_for_commodity[shortest_path_idx_in_active_paths];
        let old_shortest_flow = shortest_path_mut.flow;
        let new_shortest_flow = (old_shortest_flow + flow_deficit).max(0.0); // Убеждаемся, что поток неотрицателен

        let shortest_flow_change = new_shortest_flow - old_shortest_flow;
        max_change = max_change.max(shortest_flow_change.abs());

        shortest_path_mut.flow = new_shortest_flow;

        // Теперь обновляем потоки на ребрах на основе всех новых потоков по путям
        for path in paths_for_commodity.iter() {
            for &edge_idx in &path.edges {
                self.edges[edge_idx].flow += path.flow;
            }
        }

        // Возвращаем обновленные пути обратно в структуру Graph
        self.commodities[k].paths = paths_for_commodity;

        (max_change, alpha_t)
    }

    // Вычисление длины пути (использует первую производную)
    fn calculate_path_length(&self, k: usize, p: usize, paths: &Vec<Path>) -> f64 {
        let path = &paths[p];
        path.edges.iter()
            .map(|&edge_idx| self.derivative_delay(edge_idx))
            .sum()
    }

    // Вычисление H_kp (использует вторую производную)
    fn calculate_h_kp(&self, k: usize, p: usize, paths: &Vec<Path>) -> f64 {
        let path = &paths[p];
        let shortest_path = &paths[paths.len() - 1]; // Предполагаем, что последний путь - кратчайший

        // Находим симметрическую разность путей
        let mut l_kp = HashSet::new();
        for &edge_idx in path.edges.iter() {
            l_kp.insert(edge_idx);
        }
        for &edge_idx in shortest_path.edges.iter() {
            l_kp.insert(edge_idx);
        }

        let intersection: HashSet<_> = path.edges.iter()
            .filter(|e| shortest_path.edges.contains(e))
            .collect();

        l_kp.retain(|e| !intersection.contains(e));

        // Суммируем вторые производные
        l_kp.iter()
            .map(|&edge_idx| self.derivative_second_delay(edge_idx))
            .sum()
    }

    // Вычисление размера шага (убывающий шаг)
    fn calculate_step_size(&self, t: usize) -> f64 {
        // Простой убывающий размер шага, например, 1 / (t + 1)
        1f64 / (t + 1) as f64
        //1f64
    }

    // Функция задержки (с линейной аппроксимацией для стабильности)
    fn delay_function(&self, edge_idx: usize) -> f64 {
        let edge = &self.edges[edge_idx];
        let cap = edge.capacity;
        let flow = edge.flow;
        let rho: f64 = 0.99; // Параметр для линейной аппроксимации

        if flow >= rho * cap {
            // Линейная аппроксимация для потоков, близких к пропускной способности
            let x_rho = rho * cap;
            let f_x_rho = x_rho / (cap - x_rho);
            let f_prime_x_rho = cap / ((cap - x_rho).powi(2));
            // Используем только первую производную для линейной аппроксимации
            f_x_rho + f_prime_x_rho * (flow - x_rho)
        } else {
            // Оригинальная функция задержки Клейнрока
            flow / (cap - flow)
        }
    }

    // Первая производная функции задержки (с линейной аппроксимацией)
    fn derivative_delay(&self, edge_idx: usize) -> f64 {
        let edge = &self.edges[edge_idx];
        let cap = edge.capacity;
        let flow = edge.flow;
        let rho: f64 = 0.99; // Параметр для линейной аппроксимации

        if flow >= rho * cap {
            // Производная линейной аппроксимации - константа
            let x_rho = rho * cap;
            cap / ((cap - x_rho).powi(2))
        } else {
            // Оригинальная первая производная
            cap / ((cap - flow).powi(2))
        }
    }

    // Вторая производная функции задержки (с линейной аппроксимацией)
    fn derivative_second_delay(&self, edge_idx: usize) -> f64 {
        let edge = &self.edges[edge_idx];
        let cap = edge.capacity;
        let flow = edge.flow;
        let rho: f64 = 0.99; // Параметр для линейной аппроксимации

        if flow >= rho * cap {
            // Вторая производная линейной аппроксимации - 0
            // Для сохранения выпуклости можно использовать константу или более сложную сглаживающую функцию
            // В данном случае, для линейной аппроксимации, вторая производная будет 0.
            // Если бы использовалась квадратичная аппроксимация, то была бы константа.
            // Для стабильности, если вторая производная должна быть положительной,
            // можно вернуть небольшое положительное число.
            // Согласно статье, для квадратичной аппроксимации:
            // f_double_prime_x = 2.0 * edge.capacity / ((edge.capacity - x).powi(3));
            2.0 * cap / ((cap - rho * cap).powi(3)) // Возвращаем значение в точке rho*cap
        } else {
            // Оригинальная вторая производная
            2f64 * cap / ((cap - flow).powi(3))
        }
    }
}
