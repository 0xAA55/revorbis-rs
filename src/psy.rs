#![allow(dead_code)]
use std::{
    cmp::{min, max},
    mem,
    fmt::{self, Debug, Formatter},
    rc::Rc,
};

use crate::*;
use scales::*;
use envelope::*;
use psy_masking::*;

#[derive(Default, Clone, Copy, PartialEq)]
#[allow(non_snake_case)]
pub struct VorbisInfoPsyGlobal {
    pub eighth_octave_lines: i32,

    /* for block long/short tuning; encode only */
    pub preecho_thresh: [f32; VE_BANDS],
    pub postecho_thresh: [f32; VE_BANDS],
    pub stretch_penalty: f32,
    pub preecho_minenergy: f32,

    pub ampmax_att_per_sec: f32,

    /* channel coupling config */
    pub coupling_pkHz: [i32; PACKETBLOBS],
    pub coupling_pointlimit: [[i32; PACKETBLOBS]; 2],
    pub coupling_prepointamp: [i32; PACKETBLOBS],
    pub coupling_postpointamp: [i32; PACKETBLOBS],
    pub sliding_lowpass: [[i32; PACKETBLOBS]; 2],
}

impl Debug for VorbisInfoPsyGlobal {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("VorbisInfoPsyGlobal")
        .field("eighth_octave_lines", &self.eighth_octave_lines)
        .field("preecho_thresh", &format_args!("[{}]", format_array!(self.preecho_thresh)))
        .field("postecho_thresh", &format_args!("[{}]", format_array!(self.postecho_thresh)))
        .field("stretch_penalty", &self.stretch_penalty)
        .field("preecho_minenergy", &self.preecho_minenergy)
        .field("ampmax_att_per_sec", &self.ampmax_att_per_sec)
        .field("coupling_pkHz", &format_args!("[{}]", format_array!(self.coupling_pkHz)))
        .field("coupling_pointlimit", &format_args!("[{}, {}]",
            format_array!(self.coupling_pointlimit[0]),
            format_array!(self.coupling_pointlimit[1]),
        ))
        .field("coupling_prepointamp", &format_args!("[{}]", format_array!(self.coupling_prepointamp)))
        .field("coupling_postpointamp", &format_args!("[{}]", format_array!(self.coupling_postpointamp)))
        .field("sliding_lowpass", &format_args!("[{}, {}]",
            format_array!(self.sliding_lowpass[0]),
            format_array!(self.sliding_lowpass[1]),
        ))
        .finish()
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct VorbisLookPsyGlobal {
    ampmax: f32,
    channels: i32,
    info_psy_global: Rc<VorbisInfoPsyGlobal>,
    coupling_pointlimit: [[i32; P_NOISECURVES]; 2],
}

impl VorbisLookPsyGlobal {
    pub fn new(ampmax: f32, channels: i32, info_psy_global: Rc<VorbisInfoPsyGlobal>) -> Self {
        Self {
            ampmax,
            channels,
            info_psy_global: info_psy_global.clone(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
#[allow(non_snake_case)]
pub struct VorbisInfoPsy {
    pub block_flag: i32,

    pub ath_adjatt: f32,
    pub ath_maxatt: f32,

    pub tone_masteratt: [f32; P_NOISECURVES],
    pub tone_centerboost: f32,
    pub tone_decay: f32,
    pub tone_abs_limit: f32,
    pub toneatt: [f32; P_BANDS],

    pub noisemaskp: i32,
    pub noisemaxsupp: f32,
    pub noisewindowlo: f32,
    pub noisewindowhi: f32,
    pub noisewindowlomin: i32,
    pub noisewindowhimin: i32,
    pub noisewindowfixed: i32,
    pub noiseoff: [[f32; P_BANDS]; P_NOISECURVES],
    pub noisecompand: [f32; NOISE_COMPAND_LEVELS],

    pub max_curve_dB: f32,

    pub normal_p: i32,
    pub normal_start: i32,
    pub normal_partition: i32,
    pub normal_thresh: f64,
}

impl Debug for VorbisInfoPsy {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("VorbisInfoPsy")
        .field("block_flag", &self.block_flag)
        .field("ath_adjatt", &self.ath_adjatt)
        .field("ath_maxatt", &self.ath_maxatt)
        .field("tone_masteratt", &format_args!("[{}]", format_array!(self.tone_masteratt)))
        .field("tone_centerboost", &self.tone_centerboost)
        .field("tone_abs_limit", &self.tone_abs_limit)
        .field("toneatt", &format_args!("[{}]", format_array!(self.toneatt)))
        .field("noisemaskp", &self.noisemaskp)
        .field("noisemaxsupp", &self.noisemaxsupp)
        .field("noisewindowlo", &self.noisewindowlo)
        .field("noisewindowhi", &self.noisewindowhi)
        .field("noisewindowlomin", &self.noisewindowlomin)
        .field("noisewindowhimin", &self.noisewindowhimin)
        .field("noisewindowfixed", &self.noisewindowfixed)
        .field("noiseoff", &format_args!("[{}]", (0..P_NOISECURVES).map(|i|format!("[{}]", format_array!(self.noiseoff[i]))).collect::<Vec<_>>().join(", ")))
        .field("noisecompand", &format_args!("[{}]", format_array!(self.noisecompand)))
        .field("max_curve_dB", &self.max_curve_dB)
        .field("normal_p", &self.normal_p)
        .field("normal_start", &self.normal_start)
        .field("normal_partition", &self.normal_partition)
        .field("normal_thresh", &self.normal_thresh)
        .finish()
    }
}

impl Default for VorbisInfoPsy {
    fn default() -> Self {
        unsafe {mem::MaybeUninit::<Self>::zeroed().assume_init()}
    }
}

fn min_curve(c: &mut [f32], c2: &[f32]) {
    for i in 0..EHMER_MAX {
        c[i] = c[i].min(c2[i]);
    }
}

fn max_curve(c: &mut [f32], c2: &[f32]) {
    for i in 0..EHMER_MAX {
        c[i] = c[i].max(c2[i]);
    }
}

fn attenuate_curve(c: &mut [f32], att: f32) {
    for i in 0..EHMER_MAX {
        c[i] *= att;
    }
}

#[allow(non_snake_case)]
fn setup_tone_curves(
    curveatt_dB: &[f32; P_BANDS],
    binHz: f32,
    n: usize,
    center_boost: f32,
    center_decay_rate: f32,
) -> Vec<Vec<Vec<f32>>> {
    let mut ath = [0.0; EHMER_MAX];
    let mut workc = [[[0.0; EHMER_MAX]; P_LEVELS]; P_BANDS];
    let mut athc = [[0.0; EHMER_MAX]; P_LEVELS];
    let mut ret: Vec<Vec<Vec<f32>>> = Vec::default();
    ret.resize(P_BANDS, Vec::default());

    for i in 0..P_BANDS {
        /* we add back in the ATH to avoid low level curves falling off to
           -infinity and unnecessarily cutting off high level curves in the
           curve limiting (last step). */

        /* A half-band's settings must be valid over the whole band, and
           it's better to mask too little than too much */
        let ath_offset = i * 4;
        for j in 0..EHMER_MAX {
            let mut min = 999.0_f32;
            for k in 0..4 {
                if j + k + ath_offset < MAX_ATH {
                    min = min.min(ATH[j + k + ath_offset]);
                } else {
                    min = min.min(ATH[MAX_ATH - 1]);
                }
            }
            ath[j] = min;
        }

        /* copy curves into working space, replicate the 50dB curve to 30
           and 40, replicate the 100dB curve to 110 */
        for j in 0..6 {
            workc[i][j + 2] = TONEMASKS[i][j];
        }
        workc[i][0] = TONEMASKS[i][0];
        workc[i][1] = TONEMASKS[i][0];

        /* apply centered curve boost/decay */
        for j in 0..P_LEVELS {
            for k in 0..EHMER_MAX {
                let mut adj = center_boost + (EHMER_OFFSET as f32 - k as f32).abs() * center_decay_rate;
                if adj * center_boost < 0.0 {
                    adj = 0.0;
                }
                workc[i][j][k] += adj;
            }
        }

        /* normalize curves so the driving amplitude is 0dB */
        /* make temp curves with the ATH overlayed */
        for j in 0..P_LEVELS {
            attenuate_curve(&mut workc[i][j], curveatt_dB[i] + 100.0 - max(2, j) as f32 * 10.0 - P_LEVEL_0);
            athc[j] = ath;
            attenuate_curve(&mut athc[j], 100.0 - j as f32 * 10.0 - P_LEVEL_0);
            max_curve(&mut athc[j], &workc[i][j]);
        }

        /* Now limit the louder curves.

           the idea is this: We don't know what the playback attenuation
           will be; 0dB SL moves every time the user twiddles the volume
           knob. So that means we have to use a single 'most pessimal' curve
           for all masking amplitudes, right?  Wrong.  The *loudest* sound
           can be in (we assume) a range of ...+100dB] SL.  However, sounds
           20dB down will be in a range ...+80], 40dB down is from ...+60],
           etc... */

        for j in 1..P_LEVELS {
            let &athc_j_m_1 = &athc[j - 1];
            min_curve(&mut athc[j], &athc_j_m_1);
            min_curve(&mut workc[i][j], &athc[j]);
        }
    }

    for i in 0..P_BANDS {
        let ret_i = &mut ret[i];
        ret_i.resize(P_LEVELS, Vec::default());
        /* low frequency curves are measured with greater resolution than
           the MDCT/FFT will actually give us; we want the curve applied
           to the tone data to be pessimistic and thus apply the minimum
           masking possible for a given bin.  That means that a single bin
           could span more than one octave and that the curve will be a
           composite of multiple octaves.  It also may mean that a single
           bin may span > an eighth of an octave and that the eighth
           octave values may also be composited. */

        /* which octave curves will we be compositing? */
        let bin = (fromOC!(i as f32 * 0.5) / binHz).floor();
        let lo_curve = ((toOC!(bin * binHz + 1.0) * 2.0).ceil() as usize).clamp(0, i);
        let hi_curve = min((toOC!((bin + 1.0) * binHz) * 2.0).floor() as usize, P_BANDS);

        for m in 0..P_LEVELS {
            let ret_i_m = &mut ret_i[m];
            *ret_i_m = vec![0.0; EHMER_MAX + 2];

            let mut brute_buffer = vec![999.0_f32; n];

            /* render the curve into bins, then pull values back into curve.
               The point is that any inherent subsampling aliasing results in
               a safe minimum */
            let process_curve = |k: usize, brute_buffer: &mut [f32]| {
                let mut l = 0usize;

                for j in 0..EHMER_MAX {
                    let lo_bin = ((fromOC!(j as f32 * 0.125 + k as f32 * 0.5 - 2.0625) / binHz) as usize + 0).clamp(0, n);
                    let hi_bin = ((fromOC!(j as f32 * 0.125 + k as f32 * 0.5 - 1.9375) / binHz) as usize + 1).clamp(0, n);
                    l = min(l, lo_bin);

                    while l < hi_bin && l < n {
                        brute_buffer[l] = brute_buffer[l].min(workc[k][m][j]);
                        l += 1;
                    }
                }

                while l < n {
                    brute_buffer[l] = brute_buffer[l].min(workc[k][m][EHMER_MAX - 1]);
                    l += 1;
                }
            };
            for k in lo_curve..hi_curve {
                process_curve(k, &mut brute_buffer);
            }

            /* be equally paranoid about being valid up to next half ocatve */
            if i + 1 < P_BANDS {
                let k = i + 1;
                process_curve(k, &mut brute_buffer);
            }

            for j in 0..EHMER_MAX {
                let bin = (fromOC!(j as f32 * 0.125 + i as f32 * 0.5 - 2.0) / binHz) as isize;
                ret_i_m[j + 2] = if bin < 0 {
                    -999.0
                } else if bin as usize >= n {
                    -999.0
                } else {
                    brute_buffer[bin as usize]
                };
            }

            /* add fenceposts */
            let mut j = 0;
            while j < EHMER_OFFSET {
                if ret_i_m[j + 2] > -200.0 {
                    break;
                }
                j += 1;
            }
            ret_i_m[0] = j as f32;

            j = EHMER_MAX - 1;
            while j > EHMER_OFFSET + 1 {
                if ret_i_m[j + 2] > -200.0 {
                    break;
                }
                j -= 1;
            }
            ret_i_m[1] = j as f32;
        }
    }

    ret
}

fn setup_noise_offset(rate: u32, n: usize, vi: &VorbisInfoPsy) -> Vec<Vec<f32>> {
    let mut ret = vecvec![[0.0; n]; P_NOISECURVES];

    for i in 0..n {
        let halfoc = (toOC!((i as f32 + 0.5) * rate as f32 / (2.0 * n as f32)) * 2.0).clamp(0.0, (P_BANDS - 2) as f32);
        let inthalfoc = halfoc as i32;
        let del = halfoc - inthalfoc as f32;

        for j in 0..P_NOISECURVES {
            let inthalfoc = inthalfoc as usize;
            let ret_j = &mut ret[j];
            let src_j = &vi.noiseoff[j];
            ret_j[i] =
                src_j[inthalfoc] * (1.0 - del) +
                src_j[inthalfoc + 1] * del;
        }
    }

    ret
}


#[derive(Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct VorbisLookPsy {
    pub n: usize,
    pub vorbis_info_phy: Rc<VorbisInfoPsy>,

    pub tonecurves: Vec<Vec<Vec<f32>>>,
    pub noiseoffset: Vec<Vec<f32>>,

    pub ath: Vec<f32>,

    /// in n.ocshift format
    pub octave: Vec<i32>,
    pub bark: Vec<i32>,

    pub firstoc: i32,
    pub shiftoc: i32,
    pub eighth_octave_lines: i32,
    pub total_octave_lines: i32,
    pub rate: u32,

    /// Masking compensation value
    pub m_val: f32,
}

impl Default for VorbisLookPsy {
    #[allow(invalid_value)]
    fn default() -> Self {
        unsafe {mem::MaybeUninit::zeroed().assume_init()}
    }
}

impl VorbisLookPsy {
    pub fn new(
        vorbis_info_phy: Rc<VorbisInfoPsy>,
        vorbis_info_psy_global: &VorbisInfoPsyGlobal,
        n: usize,
        rate: u32,
    ) -> Self {
        let eighth_octave_lines = vorbis_info_psy_global.eighth_octave_lines;
        let shiftoc = toOC!(rint!((vorbis_info_psy_global.eighth_octave_lines as f32 * 8.0).log2())) as i32 - 1;
        let firstoc = (toOC!(0.25 * rate as f32 * 0.5 / n as f32) as i32 * (1 << (shiftoc + 1))) as i32 - eighth_octave_lines;
        let maxoc = (toOC!((n as f32 + 0.25) * rate as f32 * 0.5 / n as f32) * (1 << (shiftoc + 1)) as f32 + 0.5) as i32;
        let total_octave_lines = maxoc - firstoc + 1;
        let mut ath = vec![0.0; n];
        let mut octave = vec![0; n];
        let mut bark = vec![0; n];

        // AoTuV HF weighting
        let m_val = if rate < 26000 {
            0.0
        } else if rate < 38000 {
            0.94 // 32kHz
        } else if rate > 46000 {
            1.275 // 48kHz
        } else {
            1.0
        };

        // set up the lookups for a given blocksize and sample rate
        let mut j = 0;
        for i in 0..(MAX_ATH - 1) {
            let endpos = rint!(fromOC!((i + 1) as f32 * 0.125 - 2.0) * 2.0 * n as f32 / rate as f32) as usize;
            let mut base = ATH[i];
            if j < endpos {
                let delta = (ATH[i + 1] - base) / (endpos - j) as f32;
                while j < endpos && j < n {
                    ath[j] = base + 100.0;
                    base += delta;
                    j += 1;
                }
            }
        }

        while j < n {
            ath[j] = ath[j - 1];
            j += 1;
        }

        let mut lo = -99;
        let mut hi = 1;
        let noisewindowlomin = vorbis_info_phy.noisewindowlomin;
        let noisewindowhimin = vorbis_info_phy.noisewindowhimin;
        let noisewindowlo = vorbis_info_phy.noisewindowlo;
        let noisewindowhi = vorbis_info_phy.noisewindowhi;
        for i in 0..n as i32 {
            let n = n as i32;
            let rate = rate as i32;
            let bark_i = toBARK!(rate / (2 * n) * i);

            while lo + noisewindowlomin < i as i32 &&
                toBARK!((rate / (2 * n)) * lo) < bark_i - noisewindowlo {
                lo += 1;
            }

            while hi <= n && (hi < i + noisewindowhimin ||
                toBARK!(rate / (2 * n) * hi) < (bark_i + noisewindowhi)) {
                hi += 1;
            }

            bark[i as usize] = ((lo - 1) << 16) + (hi - 1);
        }

        for i in 0..n {
            let rate = rate as f32;
            let n = n as f32;
            octave[i] = (toOC!((i as f32 + 0.25) * 0.5 * rate / n) * (1 << (shiftoc + 1)) as f32 + 0.5) as i32;
        }

        Self {
            eighth_octave_lines,
            shiftoc,
            firstoc,
            total_octave_lines,
            ath,
            octave,
            bark,
            vorbis_info_phy: vorbis_info_phy.clone(),
            n,
            rate,
            m_val,
            tonecurves: setup_tone_curves(
                &vorbis_info_phy.toneatt,
                rate as f32 * 0.5 / n as f32,
                n,
                vorbis_info_phy.tone_centerboost,
                vorbis_info_phy.tone_decay
            ),
            noiseoffset: setup_noise_offset(rate, n, &*vorbis_info_phy),
        }
    }
}

impl Debug for VorbisLookPsy {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("VorbisLookPsy")
        .field("n", &self.n)
        .field("vorbis_info_phy", &self.vorbis_info_phy)
        .field("tonecurves", &NestVecFormatter::new_level2(&self.tonecurves))
        .field("noiseoffset", &NestVecFormatter::new_level1(&self.noiseoffset))
        .field("ath", &NestVecFormatter::new(&self.ath))
        .field("octave", &NestVecFormatter::new(&self.octave))
        .field("bark", &NestVecFormatter::new(&self.bark))
        .field("firstoc", &self.firstoc)
        .field("shiftoc", &self.shiftoc)
        .field("eighth_octave_lines", &self.eighth_octave_lines)
        .field("total_octave_lines", &self.total_octave_lines)
        .field("rate", &self.rate)
        .field("m_val", &self.m_val)
        .finish()
    }
}

// Psychic ready.
// Makes sense.
// Understandable.
// We could use one of those!
