use gomokugen::board::Board;



pub struct Params {
    pub c_puct: f64,
    pub valuator: Box<dyn Fn(&Board<15>) -> f64>,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            c_puct: 1.41,
            valuator: Box::new(|b| 0.0),
        }
    }
}