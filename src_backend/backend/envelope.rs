//! This is a rust version of https://github.com/Signalsmith-Audio/dsp/blob/024128874c1c24fc27ba163f465c26e30198aebd/envelopes.h#L370
//! The key idea is explained in the following articles
//! - https://signalsmith-audio.co.uk/writing/2022/cascaded-box-filter-smoothing/
//! - https://signalsmith-audio.co.uk/writing/2022/constant-time-peak-hold/

use ndarray::prelude::*;
use num_traits::{AsPrimitive, Float, FromPrimitive, NumAssignOps, NumOps};

#[derive(Clone)]
pub struct BoxSum<A: Float + NumOps + NumAssignOps + AsPrimitive<f64>>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    buffer: Vec<A>,
    index: usize,
    sum: A,
    wrap_jump: A,
}

impl<A: Float + NumOps + NumAssignOps + AsPrimitive<f64>> BoxSum<A>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    pub fn new(max_length: usize) -> Self {
        let mut out = BoxSum {
            buffer: Vec::new(),
            index: 0,
            sum: A::zero(),
            wrap_jump: A::zero(),
        };
        out.resize(max_length);
        out
    }

    pub fn resize(&mut self, max_length: usize) {
        let buf_length = max_length + 1;
        if buf_length > self.buffer.capacity() {
            self.buffer
                .reserve_exact(buf_length - self.buffer.capacity());
        } else {
            self.buffer.truncate(buf_length);
            self.buffer.shrink_to_fit();
        }
        self.reset_default();
    }

    pub fn reset(&mut self, value: A) {
        self.index = 0;
        self.sum = A::zero();
        let buf_length = self.buffer.capacity();
        self.wrap_jump = std::iter::repeat(value)
            .take(buf_length)
            .fold(A::zero(), |sum, x| {
                self.buffer.push(sum);
                sum + x
            });
    }

    pub fn reset_default(&mut self) {
        self.reset(A::zero());
    }

    pub fn read(&self, width: usize) -> A {
        if self.index >= width {
            (self.sum.as_() - self.buffer[self.index - width].as_()).as_()
        } else {
            (self.sum.as_() + self.wrap_jump.as_()
                - self.buffer[self.index + self.buffer.len() - width].as_())
            .as_()
        }
    }

    pub fn write(&mut self, value: A) {
        self.index += 1;
        if self.index == self.buffer.len() {
            self.index = 0;
            self.wrap_jump = self.sum;
            self.sum = A::zero();
        }
        self.sum += value;
        self.buffer[self.index] = self.sum;
    }

    pub fn step(&mut self, value: A, width: usize) -> A {
        self.write(value);
        self.read(width)
    }
}

#[derive(Clone)]
pub struct BoxFilter<A: Float + NumOps + NumAssignOps + AsPrimitive<f64>>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    box_sum: BoxSum<A>,
    length: usize,
    max_length: usize,
    multiplier: A,
}

impl<A: Float + NumOps + NumAssignOps + AsPrimitive<f64>> BoxFilter<A>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    pub fn new(max_length: usize) -> Self {
        BoxFilter {
            box_sum: BoxSum::new(max_length),
            length: max_length,
            max_length,
            multiplier: A::one() / max_length.as_(),
        }
    }

    pub fn resize(&mut self, max_length: usize) {
        self.box_sum.resize(max_length);
        self.max_length = max_length;
        self.set(max_length);
    }

    pub fn set(&mut self, length: usize) {
        self.length = length;
        self.multiplier = A::one() / length.as_();
        if length > self.max_length {
            self.resize(length);
        }
    }

    pub fn reset(&mut self, fill: A) {
        self.box_sum.reset(fill);
    }

    pub fn reset_default(&mut self) {
        self.box_sum.reset_default();
    }

    pub fn step(&mut self, value: A) -> A {
        self.box_sum.step(value, self.length) * self.multiplier
    }
}

#[derive(Clone)]
struct BoxFilterLayer<A: Float + NumOps + NumAssignOps + AsPrimitive<f64>>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    filter: BoxFilter<A>,
    length: usize,
    ratio: f64,
    length_err: f64,
}

impl<A: Float + NumOps + NumAssignOps + AsPrimitive<f64> + FromPrimitive> BoxFilterLayer<A>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    fn with_ratio(ratio: f64) -> Self {
        BoxFilterLayer {
            ratio,
            ..Default::default()
        }
    }
}

impl<A: Float + NumOps + NumAssignOps + AsPrimitive<f64> + FromPrimitive> Default
    for BoxFilterLayer<A>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    fn default() -> Self {
        BoxFilterLayer {
            filter: BoxFilter::new(0),
            length: 0,
            ratio: 0.,
            length_err: 0.,
        }
    }
}

#[derive(Clone)]
pub struct BoxStackFilter<A: Float + NumOps + NumAssignOps + AsPrimitive<f64>>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    layers: Vec<BoxFilterLayer<A>>,
    size: Option<usize>,
}

impl<A: Float + NumOps + NumAssignOps + AsPrimitive<f64> + FromPrimitive> BoxStackFilter<A>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    #[rustfmt::skip]
    const HARDCODED_RATIOS: [f64; 21] = [
        1.            , 0.582241861690, 0.417758138310, 0.404078562416, 0.334851475794,
        0.261069961789, 0.307944914938, 0.273699452340, 0.229132636010, 0.189222996712,
        0.248329349789, 0.229253789144, 0.201191468123, 0.173033035122, 0.148192357821,
        0.205275202874, 0.198413552119, 0.178256637764, 0.157821404506, 0.138663023387,
        0.121570179349, /* 0.178479592135, 0.171760666359, 0.158434068954, 0.143107825806,
        0.125907148711, 0.118539468950, 0.103771229086, 0.155427880834, 0.1530631528480,
        0.142803459422, 0.131358358458, 0.104157805178, 0.119338029601, 0.0901675284678,
        0.103683785192, 0.143949349747, 0.139813248378, 0.132051305252, 0.1222167761520,
        0.112888320989, 0.102534988632, 0.0928386714364, 0.0719750997699, 0.08173223964280,
        0.130587011572, 0.127244563184, 0.121228748787, 0.113509941974, 0.1050002722880,
        0.0961938290157, 0.0880639725438, 0.0738389766046, 0.0746781936619, 0.06965449036820,
        */
    ];

    pub fn new(max_size: usize) -> Self {
        BoxStackFilter::with_num_layers(max_size, 4)
    }

    pub fn with_num_layers(max_size: usize, num_layers: usize) -> Self {
        let mut out = BoxStackFilter {
            layers: Vec::new(),
            size: None,
        };
        out.resize(max_size, Self::optimal_ratios(num_layers));
        out
    }

    pub fn resize(&mut self, max_size: usize, ratios: Array1<f64>) {
        self.setup_layers(ratios);
        for layer in &mut self.layers {
            layer.filter.resize(0); // .set() will expand it later
        }
        self.size = None;
        self.set(max_size);
        self.reset_default();
    }

    /// Sets the impulse response length (does not reset if `size` ≤ `maxSize`)
    pub fn set(&mut self, size: usize) {
        if self.layers.is_empty() {
            return;
        }

        if self.size.is_some_and(|x| x == size) {
            return;
        }
        let order = size - 1;
        let order_f64 = order as f64;
        let mut total_order = 0;
        for layer in self.layers.iter_mut() {
            let layer_order_fractional = layer.ratio * order_f64;
            let layer_order = layer_order_fractional as usize;
            layer.length = layer_order + 1;
            layer.length_err = layer_order as f64 - layer_order_fractional;
            total_order += layer_order;
        }
        for _ in total_order..order {
            let (i_min, _) = self.layers.iter().enumerate().fold(
                (0, f64::INFINITY),
                |(i_min, min), (i, layer)| {
                    if layer.length_err < min {
                        (i, layer.length_err)
                    } else {
                        (i_min, min)
                    }
                },
            );
            self.layers[i_min].length += 1;
            self.layers[i_min].length_err += 1.;
        }
        self.layers
            .iter_mut()
            .for_each(|layer| layer.filter.set(layer.length));
    }

    pub fn reset(&mut self, fill: A) {
        self.layers
            .iter_mut()
            .for_each(|layer| layer.filter.reset(fill));
    }

    pub fn reset_default(&mut self) {
        self.reset(A::zero());
    }

    pub fn step(&mut self, value: A) -> A {
        self.layers
            .iter_mut()
            .fold(value, |value, layer| layer.filter.step(value))
    }

    fn setup_layers(&mut self, mut ratios: Array1<f64>) {
        ratios /= ratios.sum();
        self.layers = ratios.into_iter().map(BoxFilterLayer::with_ratio).collect();
    }

    /// Returns an optimal set of length ratios (heuristic for larger depths)
    fn optimal_ratios(num_layers: usize) -> Array1<f64> {
        // Coefficients up to 6, found through numerical search
        if num_layers == 0 {
            Array1::zeros(0)
        } else if num_layers <= 6 {
            let i_start = num_layers * (num_layers - 1) / 2;
            Self::HARDCODED_RATIOS[i_start..(i_start + num_layers)]
                .iter()
                .cloned()
                .collect()
        } else {
            let num_layers_f64 = num_layers as f64;
            let inv_n = 1. / num_layers_f64;
            let sqrt_n = num_layers_f64.sqrt();
            let p = 1. - inv_n;
            let k = 1. + 4.5 / sqrt_n + 0.08 * sqrt_n;

            let mut result: Array1<_> = (0..num_layers)
                .map(|i| {
                    let x = i as f64 * inv_n;
                    let power = -x * (1. - p * (-x * k).exp());
                    2.0f64.powf(power)
                })
                .collect();
            result /= result.sum();
            result
        }
    }

    /// Approximate (optimal) bandwidth for a given number of layers
    fn layers_to_bandwidth(num_layers: usize) -> f64 {
        1.58 * (num_layers as f64 + 0.1)
    }

    /// Approximate (optimal) peak in the stop-band
    #[allow(non_snake_case)]
    fn layers_to_peak_dB(num_layers: usize) -> f64 {
        (5 - (num_layers as isize) * 18) as f64
    }
}

#[derive(Clone)]
pub struct PeakHold<A: Float + NumOps + NumAssignOps + AsPrimitive<f64>>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    buffer: Vec<A>,
    buf_mask: isize,
    /// the start idx of back region, which saves the reverse-cumulative-max values
    i_back: isize,
    /// middle_start ~ i_working -1 : input values waiting for reverse-cumulative-max
    i_mid_start: isize,
    i_working: isize,
    /// i_working ~ middle_end : recently calculated reverse-cumulative-max values
    i_mid_end: isize,
    /// the end idx of front region, which saves the recent input values
    i_front: isize,
    /// max of the interval middle_end ~ i_front
    front_max: A,
    /// max of the interval i_working ~ middle_end
    working_max: A,
    /// max of the interval middle_start ~ middle_end
    middle_max: A,
}

impl<A: Float + NumOps + NumAssignOps + AsPrimitive<f64> + FromPrimitive> PeakHold<A>
where
    f64: AsPrimitive<A>,
    usize: AsPrimitive<A>,
{
    pub fn new(sr: u32, hold_ms: f64) -> Self {
        let mut out = PeakHold {
            buffer: Vec::new(),
            buf_mask: 0,
            i_back: 0,
            i_mid_start: 0,
            i_working: 0,
            i_mid_end: 0,
            i_front: 0,
            front_max: A::neg_infinity(),
            working_max: A::neg_infinity(),
            middle_max: A::neg_infinity(),
        };
        out.resize(sr, hold_ms);
        out
    }

    pub fn resize(&mut self, sr: u32, hold_ms: f64) {
        assert!(hold_ms > 0.);
        let hold_length = (sr as f64 * hold_ms / 1000.).round() as usize;
        let buf_length = hold_length.next_power_of_two();
        if buf_length > self.buffer.capacity() {
            self.buffer
                .reserve_exact(buf_length - self.buffer.capacity());
        } else {
            self.buffer.truncate(buf_length);
            self.buffer.shrink_to_fit();
        }
        // because buffer_length is a power of two,
        // index can be calculated by i & buffer_mask, which is cheaper than i % buffer_length
        self.buf_mask = buf_length as isize - 1;
        self.i_front = self.i_back + hold_length as isize;
        self.reset_default();
    }

    pub fn reset(&mut self, fill: A) {
        let hold_length = self.hold_length() as isize;
        self.buffer.fill(fill);
        self.buffer.resize(self.buffer.capacity(), fill);
        self.i_back = 0;
        self.i_mid_start = hold_length / 2;
        self.i_working = hold_length;
        self.i_mid_end = hold_length;
        self.i_front = hold_length;

        self.front_max = A::neg_infinity();
        self.working_max = A::neg_infinity();
        self.middle_max = A::neg_infinity();
    }

    pub fn reset_default(&mut self) {
        self.reset(A::neg_infinity());
    }

    pub fn step(&mut self, value: A) -> A {
        self.push(value);
        self.pop();
        self.read()
    }

    pub fn push(&mut self, value: A) {
        // push to front region
        self.buffer[(self.i_front & self.buf_mask) as usize] = value;
        self.i_front += 1;
        self.front_max = self.front_max.max(value);
    }

    pub fn pop(&mut self) {
        if self.i_back == self.i_mid_start {
            self.swap_regions();
        }
        self.i_back += 1;
        if self.i_working != self.i_mid_start {
            // gradually work (==calculate reverse-cumulative-max) for the middle region
            self.i_working -= 1;
            let working_item = &mut self.buffer[(self.i_working & self.buf_mask) as usize];
            self.working_max = self.working_max.max(*working_item);
            *working_item = self.working_max;
        }
    }

    pub fn read(&self) -> A {
        self.buffer[(self.i_back & self.buf_mask) as usize]
            .max(self.middle_max)
            .max(self.front_max)
    }

    pub fn hold_length(&self) -> usize {
        (self.i_front - self.i_back) as usize
    }

    fn swap_regions(&mut self) {
        // Move along the maximums
        self.working_max = A::neg_infinity();
        self.middle_max = self.front_max;
        self.front_max = A::neg_infinity();

        let prev_front_len = self.i_front - self.i_mid_end;
        let prev_mid_len = self.i_mid_end - self.i_mid_start;
        if prev_front_len <= prev_mid_len + 1 {
            // Swap over simply
            self.i_mid_start = self.i_mid_end;
            self.i_mid_end = self.i_front;
            self.i_working = self.i_mid_end;
        } else {
            // The front is longer than the middle - only happens if unbalanced
            // We don't move *all* of the front over, keeping half the surplus in the front
            let mid_len = (self.i_front - self.i_mid_start) / 2;
            self.i_mid_start = self.i_mid_end;
            self.i_mid_end += mid_len;

            // Working index is close enough that it will be finished by the time the back is empty
            let back_len = self.i_mid_start - self.i_back;
            let working_len = back_len.min(self.i_mid_end - self.i_mid_start);
            self.i_working = self.i_mid_start + working_len;

            // Since the front was not completely consumed, we re-calculate the front's maximum
            self.front_max = self.front_max.max(
                (self.i_mid_end..self.i_front)
                    .map(|i| self.buffer[(i & self.buf_mask) as usize])
                    .fold(A::neg_infinity(), A::max),
            );

            // The index might not start at the end of the working block - compute the last bit immediately
            for i in (self.i_working..self.i_mid_end).rev() {
                let working_item = &mut self.buffer[(i & self.buf_mask) as usize];
                self.working_max = self.working_max.max(*working_item);
                *working_item = self.working_max;
            }
        }

        // Is the new back (previous middle) empty? Only happens if unbalanced
        if self.i_back == self.i_mid_start {
            // swap over again (front is empty, no change)
            self.working_max = A::neg_infinity();
            self.middle_max = self.front_max;
            self.front_max = A::neg_infinity();
            self.i_working = self.i_mid_end;
            self.i_mid_start = self.i_mid_end;
            if self.i_back == self.i_mid_start {
                // Only happens if pop from an empty list - fail nicely
                self.i_back -= 1;
            }
        }

        // In case of length 0, when everything points at this value
        self.buffer[(self.i_front & self.buf_mask) as usize] = A::neg_infinity();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_hold_works() {
        let audio = [
            0., 0.1, 0.2, 1., 0.9, 0.4, 0., -0.5, -0.9, -1., -0.4, 1., 0.7,
        ];
        let target = [0., 0.1, 0.2, 1., 1., 1., 0.9, 0.4, 0., -0.5, -0.4, 1., 1.];
        let sr = 24000;
        let hold_ms = 3. / (sr as f64) * 1000.;
        let mut peakhold = PeakHold::new(sr, hold_ms);
        let peakhold_envlop: Vec<_> = audio.into_iter().map(|x| peakhold.step(x)).collect();
        // dbg!(&peakhold_envlop);
        assert_eq!(peakhold_envlop, target);
    }

    #[test]
    fn boxstack_works() {
        let input = [1., 1., 1., 1., 0., 0., 0., 0., 0., 0.];
        let target = [0.25, 0.75, 1., 1., 0.75, 0.25, 0., 0., 0., 0.];
        let mut boxstack = BoxStackFilter::with_num_layers(3, 3);
        boxstack.reset(0.);
        let output: Vec<_> = input.into_iter().map(|x| boxstack.step(x)).collect();
        assert_eq!(output, target);
    }
}