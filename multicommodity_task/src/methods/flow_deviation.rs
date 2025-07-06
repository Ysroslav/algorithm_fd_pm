use std::collections::HashMap;

use crate::functions::functions::{modified_delay_function, modified_delay_gradient};
use crate::structure::graph::{add_path_if_unique, Graph, Path};

pub type ResultExec = (usize, usize, usize);


pub struct FlowDeviation {
    graph: Graph,
    epsilon: f64,
    rho: f64,  // обычно 0.99
    armijo_alpha_init: f64, // Initial step size (e.g., 1.0)
    armijo_beta: f64,       // Contraction factor (e.g., 0.5)
    armijo_c: f64,          // Armijo condition constant (e.g., 1e-4)
}

impl FlowDeviation {
    pub(crate) fn new(graph: Graph, epsilon: f64) -> Self {
        Self {
            graph,
            epsilon,
            rho: 0.99,
            armijo_alpha_init: 1.0, // A common starting point for alpha
            armijo_beta: 0.5,       // Common contraction factor
            armijo_c: 1e-4,         // Common Armijo constant
        }
    }

    pub(crate) fn solve(&mut self) -> Result<(ResultExec), Box<dyn std::error::Error>> {
        // Шаг 1: Инициализация
        self.initialize()?;
        let mut t = 0;

        let mut lb = 0f64;

        for i in 0..1000000 {
            let current_total_edge_flows = self.calculate_total_edge_flows();

            let x_current = self.evaluate_current_objective();

            let y_0_paths_map = self.solve_subproblem()?;

            let mut y_0_total_edge_flows = vec![0.0; self.graph.edges.len()];
            for (k, path) in &y_0_paths_map {
                let demand = self.graph.commodities[*k].demand;
                for &edge_idx in &path.edges {
                    y_0_total_edge_flows[edge_idx] += demand;
                }
            }


            let (f_x_t, gradient_term_dot_product) = self.compute_lower_bound(&y_0_paths_map);


            let alpha = self.armijo_line_search(
                &current_total_edge_flows,
                &y_0_total_edge_flows,
                f_x_t,
                gradient_term_dot_product,
            )?;


            // Update lower bound (based on x^t)
            // This is the lower bound for the *current* iteration, not necessarily an overall lower bound.
            // Often, it's `lb = lb.max(f_x_t + gradient_term_dot_product);` as you have.
            // But the duality gap check often uses the value from this iteration.
            lb = lb.max(f_x_t + gradient_term_dot_product);


            // Update flows to get x^(t+1)
            self.update_flows(alpha, &y_0_paths_map)?;

            // Evaluate objective for x^(t+1)
            let updated_objective_value = self.evaluate_current_objective();

            println!("lb {} grad: {} alpha {}", (x_current - updated_objective_value).abs(), gradient_term_dot_product, alpha);

            if (x_current - updated_objective_value).abs() <= self.epsilon {
                println!("Converged after {i}");
                break;
            }

            t += 1;
        }
        self.graph.get_call_dejkstra();
        Ok(((t as usize, self.graph.get_call_dejkstra(), self.graph.commodities.iter().flat_map(|c| c.paths.iter()).count())))
    }

    fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for edge in &mut self.graph.edges {
            edge.flow = 0.0;
        }

        for k in 0..self.graph.commodities.len() {
            let source = self.graph.commodities[k].source;
            let target = self.graph.commodities[k].target;
            let demand = self.graph.commodities[k].demand;

            let derivatives = self.compute_gradient(); // This will use edge.flow = 0.0 from above

            if let Some(path) = self.graph.find_shortest_path_with_derivatives_weights(source, target, &derivatives) {
                let edges = path.edges.clone();
                let path_with_flow = Path::with_flow(edges.clone(), demand);
                self.graph.commodities[k].paths.push(path_with_flow);
            } else {
                return Err(format!(
                    "Не найден начальный путь для commodity {}", k
                ).into());
            }
        }

        Ok(())
    }
    fn solve_subproblem(&mut self) -> Result<HashMap<usize, Path>, Box<dyn std::error::Error>> {
        let mut y_0 = HashMap::new();

        // Вычисляем текущие производные для всех ребер
        let derivatives = self.compute_gradient();

        for k in 0..self.graph.commodities.len() {
            let source = self.graph.commodities[k].source;
            let target = self.graph.commodities[k].target;
            let demand = self.graph.commodities[k].demand;

            match self.graph.find_shortest_path_with_derivatives_weights(
                source,
                target,
                &derivatives,
            ) {
                Some(path) => {
                    let path_with_flow = Path::with_flow(path.edges.clone(), demand);
                    y_0.insert(k, path_with_flow);
                }
                None => {
                    return Err(format!(
                        "Не найден путь в подзадаче для commodity {} (source={}, target={})",
                        k, source, target
                    ).into());
                }
            }
        }

        Ok(y_0)
    }

    fn calculate_total_edge_flows(&self) -> Vec<f64> {
        let mut total_edge_flows = vec![0.0; self.graph.edges.len()];
        for commodity in &self.graph.commodities {
            for path in &commodity.paths {
                for &edge_idx in &path.edges {
                    total_edge_flows[edge_idx] += path.flow;
                }
            }
        }
        total_edge_flows
    }

    fn compute_cost_for_total_flow(&self, total_flow_vector: &[f64]) -> f64 {
        let mut total_cost = 0.0;
        for j in 0..self.graph.edges.len() {
            let edge_flow = total_flow_vector[j];
            // Ensure flow doesn't exceed capacity for delay function
            if edge_flow >= self.graph.edges[j].capacity * self.rho { // Using rho for the "safe" capacity
                // Handle capacity violation for the cost function (as per the paper's f_bar)
                // This part requires the quadratic/linear function described in [cite: 112]
                // For simplicity, let's assume it's handled by modified_delay_function
                total_cost += modified_delay_function(edge_flow, self.graph.edges[j].capacity, self.rho);
            } else {
                total_cost += modified_delay_function(edge_flow, self.graph.edges[j].capacity, self.rho);
            }
        }
        total_cost
    }

    // Метод поиска шага, минимизирующий целевую функцию
    fn step_length_search(&self, current_total_edge_flows: &[f64], y_0_total_edge_flows: &[f64]) -> Result<f64, Box<dyn std::error::Error>> {
        let mut alpha_left = 0.0001;
        let mut alpha_right = 0.999;
        let num_steps = 1000;

        let mut best_alpha = alpha_left;
        let mut best_cost = f64::INFINITY;

        for i in 0..=num_steps {
            let alpha = alpha_left + (alpha_right - alpha_left) * (i as f64 / num_steps as f64);
            let mut trial_total_flow = vec![0.0; self.graph.edges.len()];
            for j in 0..self.graph.edges.len() {
                trial_total_flow[j] = (1.0 - alpha) * current_total_edge_flows[j] + alpha * y_0_total_edge_flows[j];
            }

            let current_cost = self.compute_cost_for_total_flow(&trial_total_flow);

            if current_cost < best_cost {
                best_cost = current_cost;
                best_alpha = alpha;
            }
        }
        Ok(best_alpha)
    }

    fn evaluate_objective_for_step(&self, flow: &[f64]) -> Result<f64, Box<dyn std::error::Error>> {
        // Рассчитываем стоимость на основе потока, например, как сумму задержек
        let mut total_cost = 0.0;
        for (i, &f) in flow.iter().enumerate() {
            // Получаем пропускную способность для каждого ребра (из self.graph.edges)
            let edges: Vec<usize> = self.graph.commodities[i].paths.iter().flat_map(|p| p.edges.iter().cloned()).collect();
            for edge_index in edges {
                let capacity = self.graph.edges[edge_index].capacity;
                total_cost += modified_delay_function(f, capacity, self.rho);
            }
        }
        Ok(total_cost)
    }

    fn update_flows(&mut self, alpha: f64, y_0_paths_map: &HashMap<usize, Path>) -> Result<(), Box<dyn std::error::Error>> {
        // First, get the current total edge flows before any updates
        let current_total_edge_flows = self.calculate_total_edge_flows();
        let mut y_0_total_edge_flows = vec![0.0; self.graph.edges.len()];

        // Calculate y_0_total_edge_flows from y_0_paths_map
        for (k, path) in y_0_paths_map {
            let demand = self.graph.commodities[*k].demand;
            for &edge_idx in &path.edges {
                y_0_total_edge_flows[edge_idx] += demand;
            }
        }
        for j in 0..self.graph.edges.len() {
            self.graph.edges[j].flow = (1.0 - alpha) * current_total_edge_flows[j] + alpha * y_0_total_edge_flows[j];
        }

        for k in 0..self.graph.commodities.len() {
            for path_idx in 0..self.graph.commodities[k].paths.len() {
                self.graph.commodities[k].paths[path_idx].flow *= (1.0 - alpha);
            }
        }

        // Step 2: Add the y_0 contribution
        for (k, new_path) in y_0_paths_map {
            // Check if this new_path (by edges) already exists for commodity k
            let mut found = false;
            let commodity = &mut self.graph.commodities[*k]; // Mutable borrow for the commodity

            for path_idx in 0..commodity.paths.len() {
                if commodity.paths[path_idx].edges == new_path.edges {
                    commodity.paths[path_idx].flow += alpha * commodity.demand;
                    found = true;
                    break;
                }
            }
            if !found {
                // Add the new path with flow `alpha * demand`
                let mut path_to_add = new_path.clone(); // Clone the path from y_0
                path_to_add.flow = alpha * commodity.demand;
                add_path_if_unique(commodity, path_to_add);
            }
        }

        // Step 3: Recalculate total edge flows from updated path flows
        // Clear all edge flows first

        Ok(())
    }

    fn check_termination(&self, lb: f64) -> bool {
        // Проверяем условие остановки f(x^(t+1)) <= (1+epsilon)*LB
        let current_objective = self.evaluate_current_objective();
        current_objective <= (1.0 + self.epsilon) * lb
    }

    fn evaluate_objective(&self, alpha: f64, y_0: &[Path]) -> f64 {
        // Оцениваем значение целевой функции для данного alpha
        // Используем модифицированную функцию задержки
        let mut total_cost = 0.0;

        for edge in &self.graph.edges {
            let new_flow = edge.flow * (1.0 - alpha) +
                y_0.iter()
                    .flat_map(|p| p.edges.iter().filter(|&&e| e == edge.index).map(move |_| p)) // Здесь p передается
                    .map(|p| p.flow)  // Теперь p доступен для получения flow
                    .sum::<f64>() * alpha;

            total_cost += modified_delay_function(new_flow, edge.capacity, self.rho);
        }

        total_cost
    }

    fn compute_lower_bound(&self, y_0_paths_map: &HashMap<usize, Path>) -> (f64, f64) {
        // Calculate current objective (f(x^t_0)) based on current total edge flows
        let current_total_edge_flows = self.calculate_total_edge_flows();
        let current_objective = self.compute_cost_for_total_flow(&current_total_edge_flows);

        // Calculate y_0_total_edge_flows from y_0_paths_map
        let mut y_0_total_edge_flows = vec![0.0; self.graph.edges.len()];
        for (k, path) in y_0_paths_map {
            let demand = self.graph.commodities[*k].demand;
            for &edge_idx in &path.edges {
                y_0_total_edge_flows[edge_idx] += demand;
            }
        }

        // Get the gradient vector (derivatives for each edge)
        let gradient_map = self.compute_gradient(); // HashMap<usize, f64> where key is edge index

        let mut gradient_term_dot_product = 0.0;
        //println!("DEBUG: Lower Bound Detail for Iteration:");
        for j in 0..self.graph.edges.len() {
            let edge_index = self.graph.edges[j].index;
            let grad_j = *gradient_map.get(&edge_index)
                .expect("Gradient not found for edge index");
            let diff_j = y_0_total_edge_flows[j] - current_total_edge_flows[j];
            let term = grad_j * diff_j;
            gradient_term_dot_product += term;
        }

        // Return (f(x^t_0), gradient_term_dot_product)
        (current_objective, gradient_term_dot_product)
    }

    fn evaluate_current_objective(&self) -> f64 {
        let mut total_cost = 0.0;
        for (i, edge) in self.graph.edges.iter().enumerate() {
            total_cost += modified_delay_function(edge.flow, edge.capacity, self.rho);
        }
        total_cost
    }

    fn compute_gradient(&self) -> HashMap<usize, f64> {
        let mut gradient = HashMap::new();

        for (i, edge) in self.graph.edges.iter().enumerate() {
            let val_grad = modified_delay_gradient(edge.flow, edge.capacity, self.rho);
            // Вычисляем производную функции задержки
            gradient.insert(edge.index, val_grad);
        }

        gradient
    }


    fn golden_section_line_search(
        &self,
        n: &usize,
        x_0: &[f64],
        y_0: &[f64],
    ) -> Result<f64, Box<dyn std::error::Error>> {
        // Константа золотого сечения
        let tau: f64 = (5.0_f64.sqrt() + 1.0) / 2.0;
        let epsilon: f64 = 1e-3;     // точность поиска

        // Начальные границы интервала
        let mut a: f64 = 0.0;
        let mut b: f64 = 1.0;

        // Вычисляем начальные точки по формулам из Python-версии
        let mut y: f64 = a + (b - a) / (tau * tau);
        let mut z: f64 = a + (b - a) / tau;

        // Вычисляем значения функции в точках y и z
        let mut f_y = self.evaluate_objective_for_step_1(
            &x_0.iter()
                .zip(y_0.iter())
                .map(|(&x, &y_val)| x + y * (y_val - x))
                .collect::<Vec<f64>>()
        )?;

        let mut f_z = self.evaluate_objective_for_step_1(
            &x_0.iter()
                .zip(y_0.iter())
                .map(|(&x, &y_val)| x + z * (y_val - x))
                .collect::<Vec<f64>>()
        )?;

        // Основной цикл поиска
        while (b - a).abs() > epsilon {
            if f_y <= f_z {
                b = z;
                z = y;
                f_z = f_y;
                y = a + (b - a) / (tau * tau);
                f_y = self.evaluate_objective_for_step_1(
                    &x_0.iter()
                        .zip(y_0.iter())
                        .map(|(&x, &y_val)| x + y * (y_val - x))
                        .collect::<Vec<f64>>()
                )?;
            } else {
                a = y;
                y = z;
                f_y = f_z;
                z = a + (b - a) / tau;
                f_z = self.evaluate_objective_for_step_1(
                    &x_0.iter()
                        .zip(y_0.iter())
                        .map(|(&x, &y_val)| x + z * (y_val - x))
                        .collect::<Vec<f64>>()
                )?;
            }
        }

        // Возвращаем середину финального интервала
        let alpha = (a + b) / 2.0;

        // Предотвращаем слишком маленький шаг
        if alpha < 1e-10 {
            Ok(1.0 / (*n as f64 + 1.0).sqrt())
        } else {
            Ok(alpha)
        }
        //Ok(alpha)
    }

    fn evaluate_objective_for_step_1(&self, flow: &[f64]) -> Result<f64, Box<dyn std::error::Error>> {
        let mut total_cost = 0.0;

        // Вычисляем суммарный поток на каждом ребре
        let mut edge_flows = vec![0.0; self.graph.edges.len()];

        for (i, &f) in flow.iter().enumerate() {
            let edges: Vec<usize> = self.graph.commodities[i].paths.iter()
                .flat_map(|p| p.edges.iter().cloned())
                .collect();

            for &edge_index in &edges {
                edge_flows[edge_index] += f;
            }
        }

        // Вычисляем стоимость для каждого ребра
        for (edge_idx, total_flow) in edge_flows.iter().enumerate() {
            let edge = &self.graph.edges[edge_idx];
            total_cost += modified_delay_function(*total_flow, edge.capacity, self.rho);
        }

        Ok(total_cost)
    }

    fn armijo_line_search(
        &self,
        current_total_edge_flows: &[f64],
        y_0_total_edge_flows: &[f64],
        f_x_t: f64, // f(x^t)
        gradient_term_dot_product: f64, // ∇f(x^t)^T (y^t - x^t)
    ) -> Result<f64, Box<dyn std::error::Error>> {
        let mut alpha = self.armijo_alpha_init;
        let c = self.armijo_c;
        let beta = self.armijo_beta;
        let max_iterations = 100; // Prevent infinite loop

        for _ in 0..max_iterations {
            let mut trial_total_flow = vec![0.0; self.graph.edges.len()];
            for j in 0..self.graph.edges.len() {
                trial_total_flow[j] = (1.0 - alpha) * current_total_edge_flows[j] + alpha * y_0_total_edge_flows[j];
            }

            let f_x_alpha = self.compute_cost_for_total_flow(&trial_total_flow);

            // Armijo condition: f(x_k + alpha * p_k) <= f(x_k) + c * alpha * gradient_f(x_k)^T * p_k
            // Here, p_k is (y_0_total_edge_flows - current_total_edge_flows)
            if f_x_alpha <= f_x_t + c * alpha * gradient_term_dot_product {
                return Ok(alpha);
            }

            alpha *= beta; // Reduce step size
            if alpha < 1e-10 { // Prevent alpha from becoming too small
                return Ok(alpha);
            }
        }
        // If Armijo condition is not met after max_iterations, return the last alpha found
        // or a default small value. This indicates a potential issue or very flat landscape.
        Ok(alpha)
    }
}