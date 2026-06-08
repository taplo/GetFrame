use crate::pipeline::rule::ComparisonMethod;

pub struct FrameComparator {
    prev_y: Option<Vec<u8>>,
    prev_width: u32,
    prev_height: u32,
    method: ComparisonMethod,
    threshold: f64,
}

impl FrameComparator {
    pub fn new(method: ComparisonMethod, threshold: f64) -> Self {
        Self { prev_y: None, prev_width: 0, prev_height: 0, method, threshold }
    }

    pub fn is_static(&mut self, y_plane: &[u8], width: u32, height: u32) -> Result<bool, &'static str> {
        let expected_len = (width * height) as usize;
        if y_plane.len() < expected_len {
            return Err("Y plane data too short for dimensions");
        }
        let is_first = self.prev_y.is_none();
        if !is_first && (width != self.prev_width || height != self.prev_height) {
            self.reset();
            return Ok(false);
        }
        let result = if is_first {
            false
        } else {
            let prev = self.prev_y.as_ref().unwrap();
            match self.method {
                ComparisonMethod::PixelDiff => {
                    let diff = Self::pixel_diff(prev, y_plane, expected_len);
                    diff <= self.threshold
                }
                ComparisonMethod::PerceptualHash => {
                    let hash_prev = Self::phash(prev, self.prev_width, self.prev_height);
                    let hash_curr = Self::phash(y_plane, width, height);
                    let distance = Self::hamming_distance(hash_prev, hash_curr);
                    (distance as f64 / 63.0) <= self.threshold
                }
                ComparisonMethod::Ssim => {
                    false // placeholder — implemented in Task 5
                }
            }
        };
        self.prev_y = Some(y_plane[..expected_len].to_vec());
        self.prev_width = width;
        self.prev_height = height;
        Ok(if is_first { false } else { result })
    }

    fn pixel_diff(a: &[u8], b: &[u8], len: usize) -> f64 {
        let sum: u64 = a[..len].iter().zip(b[..len].iter())
            .map(|(x, y)| (*x as i16 - *y as i16).unsigned_abs() as u64)
            .sum();
        sum as f64 / (len as f64 * 255.0)
    }

    fn downsample_to_8x8(src: &[u8], src_width: u32, src_height: u32) -> [[f64; 8]; 8] {
        let mut out = [[0.0f64; 8]; 8];
        let x_ratio = src_width as f64 / 8.0;
        let y_ratio = src_height as f64 / 8.0;
        for ty in 0..8 {
            for tx in 0..8 {
                let sx = (tx as f64 * x_ratio) as u32;
                let sy = (ty as f64 * y_ratio) as u32;
                let idx = (sy * src_width + sx) as usize;
                out[ty][tx] = src[idx] as f64;
            }
        }
        out
    }

    fn dct_8x8(block: &[[f64; 8]; 8]) -> [[f64; 8]; 8] {
        let mut out = [[0.0f64; 8]; 8];
        for u in 0..8 {
            for v in 0..8 {
                let mut sum = 0.0;
                for x in 0..8 {
                    for y in 0..8 {
                        let cx = (x as f64 + 0.5) * std::f64::consts::PI * u as f64 / 8.0;
                        let cy = (y as f64 + 0.5) * std::f64::consts::PI * v as f64 / 8.0;
                        sum += block[x][y] * (cx.cos() * cy.cos());
                    }
                }
                let cu = if u == 0 { 1.0 / (8.0_f64).sqrt() } else { 0.5 };
                let cv = if v == 0 { 1.0 / (8.0_f64).sqrt() } else { 0.5 };
                out[u][v] = cu * cv * sum;
            }
        }
        out
    }

    fn phash(y_plane: &[u8], width: u32, height: u32) -> u64 {
        let block = Self::downsample_to_8x8(y_plane, width, height);
        let dct = Self::dct_8x8(&block);
        let mut values = Vec::with_capacity(63);
        for u in 0..8 {
            for v in 0..8 {
                if u == 0 && v == 0 { continue; }
                values.push(dct[u][v]);
            }
        }
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = values[values.len() / 2];
        let mut hash = 0u64;
        for (i, &val) in values.iter().enumerate() {
            if val > median {
                hash |= 1u64 << i;
            }
        }
        hash
    }

    fn hamming_distance(a: u64, b: u64) -> u32 {
        (a ^ b).count_ones()
    }

    fn reset(&mut self) {
        self.prev_y = None;
        self.prev_width = 0;
        self.prev_height = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixdiff_identical_frames() {
        let y = vec![128u8; 320 * 240];
        let mut cmp = FrameComparator::new(ComparisonMethod::PixelDiff, 0.05);
        assert!(!cmp.is_static(&y, 320, 240).unwrap());
        assert!(cmp.is_static(&y, 320, 240).unwrap());
    }

    #[test]
    fn test_pixdiff_different_frames() {
        let y1 = vec![0u8; 320 * 240];
        let y2 = vec![255u8; 320 * 240];
        let mut cmp = FrameComparator::new(ComparisonMethod::PixelDiff, 0.1);
        assert!(!cmp.is_static(&y1, 320, 240).unwrap());
        assert!(!cmp.is_static(&y2, 320, 240).unwrap());
    }

    #[test]
    fn test_pixdiff_first_frame_never_static() {
        let y = vec![128u8; 320 * 240];
        let mut cmp = FrameComparator::new(ComparisonMethod::PixelDiff, 0.0);
        assert!(!cmp.is_static(&y, 320, 240).unwrap());
    }

    #[test]
    fn test_pixdiff_threshold_boundary() {
        let y1 = vec![100u8; 320 * 240];
        let y2 = vec![101u8; 320 * 240];
        let mut cmp = FrameComparator::new(ComparisonMethod::PixelDiff, 0.005);
        assert!(!cmp.is_static(&y1, 320, 240).unwrap());
        assert!(cmp.is_static(&y2, 320, 240).unwrap());
    }

    #[test]
    fn test_resolution_change_resets() {
        let y1 = vec![128u8; 320 * 240];
        let y2 = vec![128u8; 640 * 480];
        let mut cmp = FrameComparator::new(ComparisonMethod::PixelDiff, 0.05);
        assert!(!cmp.is_static(&y1, 320, 240).unwrap());
        assert!(!cmp.is_static(&y2, 640, 480).unwrap());
    }

    #[test]
    fn test_phash_identical_frames() {
        let y = vec![128u8; 320 * 240];
        let mut cmp = FrameComparator::new(ComparisonMethod::PerceptualHash, 0.1);
        assert!(!cmp.is_static(&y, 320, 240).unwrap());
        assert!(cmp.is_static(&y, 320, 240).unwrap());
    }

    #[test]
    fn test_phash_different_frames() {
        // Horizontal gradient vs vertical gradient — different spatial structure
        let y1: Vec<u8> = (0..240).flat_map(|y| (0..320).map(move |x| ((x * 255 / 319) as u8)).collect::<Vec<_>>()).collect();
        let y2: Vec<u8> = (0..240).flat_map(|y| (0..320).map(move |_| ((y * 255 / 239) as u8)).collect::<Vec<_>>()).collect();
        let mut cmp = FrameComparator::new(ComparisonMethod::PerceptualHash, 0.1);
        assert!(!cmp.is_static(&y1, 320, 240).unwrap());
        assert!(!cmp.is_static(&y2, 320, 240).unwrap());
    }

    #[test]
    fn test_phash_threshold_zero() {
        let y = vec![100u8; 320 * 240];
        let mut cmp = FrameComparator::new(ComparisonMethod::PerceptualHash, 0.0);
        assert!(!cmp.is_static(&y, 320, 240).unwrap());
        assert!(cmp.is_static(&y, 320, 240).unwrap());
    }
}
