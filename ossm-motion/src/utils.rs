pub fn scale(
    input: f64,
    input_start: f64,
    input_end: f64,
    output_start: f64,
    output_end: f64,
) -> f64 {
    let slope = (output_end - output_start) / (input_end - input_start);
    output_start + slope * (input - input_start)
}

pub fn saturate_range(input: f64, min: f64, max: f64) -> f64 {
    let mut output = input;

    if output < min {
        output = min;
    }

    if output > max {
        output = max;
    }

    output
}
