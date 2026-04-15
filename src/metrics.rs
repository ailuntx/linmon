use serde::Serialize;

#[derive(Debug, Default, Serialize, Clone)]
pub struct TempMetrics {
    pub cpu_temp: Option<f32>,
    pub gpu_temp: Option<f32>,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct PowerMetrics {
    pub cpu_power: Option<f32>,
    pub gpu_power: Option<f32>,
    pub sys_power: Option<f32>,
    pub tracked_power: Option<f32>,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct MemMetrics {
    pub ram_total: u64,
    pub ram_usage: u64,
    pub swap_total: u64,
    pub swap_usage: u64,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct Metrics {
    pub temp: TempMetrics,
    pub power: PowerMetrics,
    pub memory: MemMetrics,
    pub cpu_usage: (u32, f32),
    pub gpu_usage: (u32, f32),
}

pub fn zero_div<T: core::ops::Div<Output = T> + Default + PartialEq>(a: T, b: T) -> T {
    let zero: T = Default::default();
    if b == zero {
        zero
    } else {
        a / b
    }
}
