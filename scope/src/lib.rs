// Copyright 2018 The Synthesizer IO Authors.
// 
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// 
//     https://www.apache.org/licenses/LICENSE-2.0
// 
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A renderer for a visual waveform display resembling an analog oscilloscope.

/// The box beyond which the gaussian can be clipped, as a multiple of radius.
const CLIP_FACTOR: f32 = 2.5;

pub struct Scope {
    width: usize,
    height: usize,
    glow: Vec<f32>,

    // time constant for fade, in _samples_
    tc: f32,

    // fraction of scope width per sample
    sweep: f32,

    // current horiz position, as fraction of total width
    horiz: f32,

    // gain, where 1.0 is top to bottom of height
    gain: f32,

    xylast: Option<(f32, f32)>,
}

impl Scope {
    // Create a new Scope instance of the given size.
    pub fn new(width: usize, height: usize) -> Scope {
        let glow = vec![0.0; width * height];
        let tc = 1_000.0;
        let sweep = 0.001;
        let horiz = 0.0;
        let gain = 1.0;
        let xylast = None;
        Scope { width, height, glow, tc, sweep, horiz, gain, xylast }
    }

    // Add a dot to the glow.
    pub fn add_dot(&mut self, x: f32, y: f32, r: f32, amp: f32) {
        let r_recip = r.recip();
        let i0 = ((x - CLIP_FACTOR * r).ceil().max(0.0) as usize).min(self.width);
        let i1 = ((x + CLIP_FACTOR * r).ceil().max(0.0) as usize).min(self.width);
        let j0 = ((y - CLIP_FACTOR * r).ceil().max(0.0) as usize).min(self.height);
        let j1 = ((y + CLIP_FACTOR * r).ceil().max(0.0) as usize).min(self.height);
        for j in j0..j1 {
            let zy_amp = gauss_approx(r_recip * (j as f32 - y)) * amp;
            for i in i0..i1 {
                let zx = gauss_approx(r_recip * (i as f32 - x));
                self.glow[j * self.width + i] += zx * zy_amp;
            }
        }
    }

    pub fn add_line_step(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, amp: f32) {
        let n = 20;
        let step = (n as f32).recip();
        let amp = amp / (n as f32);
        for i in 0..n {
            let t = (i as f32 + 0.5) * step;
            self.add_dot(x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, r, amp);
        }
    }

    pub fn add_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, amp: f32) {
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len2 = dx * dx + dy * dy;
        if len2 < 1.0 {
            self.add_dot((x0 + x1) * 0.5, (y0 + y1) * 0.5, r, amp);
            return;
        }
        // Also, for medium-small lengths, add_line_step with 2 steps might win.
        let uvscale = 1.0 / (r * len2.sqrt());
        let vx = -dy * uvscale;
        let vy = dx * uvscale;
        // scale of u relative to v
        let uscale = 2.0 / ::std::f32::consts::PI.sqrt();
        let ux = vy * uscale;
        let uy = -vx * uscale;
        let u0 = -x0 * ux - y0 * uy;
        let v0 = -x0 * vx - y0 * vy;
        let ustep = dx * ux + dy * uy;
        let amp = r / uscale * amp / len2.sqrt();
        let i0 = ((x0.min(x1) - CLIP_FACTOR * r).ceil().max(0.0) as usize).min(self.width);
        let i1 = ((x0.max(x1) + CLIP_FACTOR * r).ceil().max(0.0) as usize).min(self.width);
        let j0 = ((y0.min(y1) - CLIP_FACTOR * r).ceil().max(0.0) as usize).min(self.height);
        let j1 = ((y0.max(y1) + CLIP_FACTOR * r).ceil().max(0.0) as usize).min(self.height);
        // TODO: (i1-i0).min(j1-j0) is a measure of wastefulness of drawing the whole rect.
        // If this is high, compute horiz bounds per scan line.
        for j in j0..j1 {
            for i in i0..i1 {
                let u = ux * (i as f32) + uy * (j as f32) + u0;
                let v = vx * (i as f32) + vy * (j as f32) + v0;
                let z = amp * gauss_approx(v) * (erf_approx(u) - erf_approx(u - ustep));
                self.glow[j * self.width + i] += z;
            }
        }
    }

    pub fn as_rgba(&self) -> Vec<u8> {
        let n = self.width * self.height;
        let mut im = vec![255; n * 4];
        for i in 0..n {
            let r = (64.0 * self.glow[i].min(1.0).sqrt()) as u8;
            let g = (255.0 * (self.glow[i] + 0.03).min(1.0).sqrt()) as u8;
            let b = (224.0 * (self.glow[i] + 0.1).min(1.0).sqrt()) as u8;
            im[i * 4 + 0] = r;
            im[i * 4 + 1] = g;
            im[i * 4 + 2] = b;
        }
        self.render_grid_lines(&mut im);
        im
    }

    pub fn fade(&mut self, factor: f32) {
        for x in &mut self.glow {
            *x *= factor;
        }
    }

    pub fn provide_samples(&mut self, samples: &[f32]) {
        let factor = (-(samples.len() as f32) / self.tc).exp();
        self.fade(factor);
        let y0 = self.height as f32 * 0.5;
        let yscale = y0 * self.gain;
        for sample in samples {
            let x = self.horiz * (self.width as f32);
            let y = y0 - yscale * sample;
            if let Some((xlast, ylast)) = self.xylast {
                self.add_line(xlast, ylast, x, y, 1.0, 2.0);
            }
            self.xylast = Some((x, y));
            self.horiz += self.sweep;
            if self.horiz > 1.0 {
                self.horiz -= 1.0;
                self.xylast = None;
            }
        }
    }

    fn render_grid_lines(&self, im: &mut [u8]) {
        let x2 = self.width / 2;
        let y2 = self.height / 2;
        let grid_sp = 60;
        let tick_sp = 12;
        let tick_len = 6;
        self.render_hline(0, self.width, y2, im);
        self.render_vline(x2, 0, self.height, im);
        for i in 1..((y2 + grid_sp - 1) / grid_sp) {
            self.render_hline(0, self.width, y2 + i * grid_sp, im);
            self.render_hline(0, self.width, y2 - i * grid_sp, im);
        }
        for i in 1..((x2 + grid_sp - 1) / grid_sp) {
            self.render_vline(x2 + i * grid_sp, 0, self.height, im);
            self.render_vline(x2 - i * grid_sp, 0, self.height, im);
        }
        for i in 1..((y2 + tick_sp - 1) / tick_sp) {
            self.render_hline(x2 - tick_len, x2 + tick_len, y2 - i * tick_sp, im);
            self.render_hline(x2 - tick_len, x2 + tick_len, y2 + i * tick_sp, im);
        }
        for i in 1..((x2 + tick_sp - 1) / tick_sp) {
            self.render_vline(x2 + i * tick_sp, y2 - tick_len, y2 + tick_len, im);
            self.render_vline(x2 - i * tick_sp, y2 - tick_len, y2 + tick_len, im);
        }
    }

    fn render_hline(&self, x0: usize, x1: usize, y: usize, im: &mut [u8]) {
        for i in (y * self.width + x0)..(y * self.width + x1) {
            im[i * 4 + 0] >>= 1;
            im[i * 4 + 1] >>= 1;
            im[i * 4 + 2] >>= 1;
        }
    }

    fn render_vline(&self, x: usize, y0: usize, y1: usize, im: &mut [u8]) {
        for j in y0..y1 {
            let i = j * self.width + x;
            im[i * 4 + 0] >>= 1;
            im[i * 4 + 1] >>= 1;
            im[i * 4 + 2] >>= 1;
        }
    }
}

// Approximate exp(-x*x) in a SIMD-friendly way; approx 3.2e-3 error.
pub fn gauss_approx(x: f32) -> f32 {
    let xx = x * x;
    let y = x + (0.215 + 0.0952 * xx) * (x * xx);
    (1.0 + y * y).recip()
}

// Approximate erf(x * sqrt(pi) / 2); approx 1.6e-3 error
pub fn erf_approx(x: f32) -> f32 {
    let xx = x * x;
    let x = x + (0.217 + 0.072 * xx) * (x * xx);
    x / (1.0 + x * x).sqrt()
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
