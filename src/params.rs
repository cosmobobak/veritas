

pub struct Params {
    pub c_puct: f64,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            c_puct: 1.41,
        }
    }
}