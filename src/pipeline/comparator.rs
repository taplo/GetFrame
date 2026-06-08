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
                ComparisonMethod::PerceptualHash | ComparisonMethod::Ssim => {
                    false // placeholder — implemented in Tasks 4, 5
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
}
