pub fn scale(input: f64, in_min: f64, in_max: f64, out_min: f64, out_max: f64) -> f64 {
    let slope = (out_max - out_min) / (in_max - in_min);
    out_min + slope * (input - in_min)
}
