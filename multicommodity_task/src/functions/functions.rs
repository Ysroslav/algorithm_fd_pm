type Coeffs = (f64, f64, f64);

fn psi_coeffs(c: f64, rho: f64) -> Coeffs {
    let x0 = rho * c;
    // значение, 1‑я и 2‑я производные исходной части
    let f0 = x0 / (c - x0);                         // f(x0)
    let f1 = c / (c - x0).powi(2);                  // f'(x0)
    let f2 = 2.0 * c / (c - x0).powi(3);            // f''(x0)

    let a = f2 / 2.0;
    let b = f1 - 2.0 * a * x0;
    let d = f0 - a * x0 * x0 - b * x0;
    (a, b, d)
}


pub fn modified_delay_function(flow: f64, capacity: f64, rho: f64) -> f64 {
    if flow <= rho * capacity  {
        flow / (capacity - flow)                                 // исходная часть
    } else {
        let (a, b, d) = psi_coeffs(capacity, rho);                // квадратичное продолжение
        a * flow * flow + b * flow + d
    }
}

pub fn modified_delay_gradient(flow: f64, capacity: f64, rho: f64) -> f64 {
    if flow <= rho * capacity {
        capacity / (capacity  - flow).powi(2)                           // производная исходной части
    } else {
        let (a, b, _) = psi_coeffs(capacity, rho);
        2.0 * a * flow + b                               // производная ψ
    }
}

pub fn modified_two_function(flow: f64, cost_a: f64, cost_b: f64) -> f64 {
    cost_a * flow.powi(2) + cost_b * flow
}

pub fn modified_two_gradient(flow: f64, cost_a: f64, cost_b: f64) -> f64 {
    cost_a * flow + cost_b
}