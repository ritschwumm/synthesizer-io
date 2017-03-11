// Copyright 2017 Google Inc. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! A simple module that makes a sine wave.

use std::f32::consts;

use module::{Module, Buffer};

pub struct Sin {
    phase: f32,
    freq: f32,
}

impl Sin {
    /// Frequency is specified in cycles per sample. Note: we'll move to freq as
    /// a control input.
    pub fn new(freq: f32) -> Sin {
        Sin {
            phase: 0.0,
            freq: freq,
        }
    }
}

fn mod_1(x: f32) -> f32 {
    x - x.floor()
}

impl Module for Sin {
    fn n_bufs_out(&self) -> usize { 1 }

    fn process(&mut self, _control_in: &[f32], _control_out: &mut [f32],
        _buf_in: &[&Buffer], buf_out: &mut [Buffer])
    {
        let out = buf_out[0].get_mut();
        let mut phase = self.phase;
        for i in 0..out.len() {
            out[i] = (phase * 2.0 * consts::PI).sin();
            phase += self.freq;
        }
        self.phase = mod_1(phase);
    }
}