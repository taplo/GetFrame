use crate::types::DecodedFrame;
use anyhow::Result;
use bytes::Bytes;
use turbojpeg::{Compressor, YuvPlanesImage, Subsamp};
use std::time::Instant;

pub fn encode_jpeg(
    frame: &DecodedFrame,
    quality: u8,
) -> Result<Bytes> {
    let width = frame.width as usize;
    let height = frame.height as usize;

    let encode_timer = Instant::now();
    let mut compressor = Compressor::new()?;
    compressor.set_quality(quality as i32)?;
    let image = YuvPlanesImage {
        y_plane: &frame.y_plane[..],
        u_plane: &frame.u_plane[..],
        v_plane: &frame.v_plane[..],
        width,
        height,
        y_stride: frame.y_stride as usize,
        u_stride: frame.u_stride as usize,
        v_stride: frame.v_stride as usize,
        subsamp: Subsamp::Sub2x2,
    };
    let jpeg = compressor.compress_yuv_planes_to_vec(&image)?;
    let encode_us = encode_timer.elapsed().as_micros();

    tracing::debug!(
        stream_id = %frame.stream_id,
        frame_number = frame.frame_number,
        jpeg_encode_us = encode_us,
        jpeg_size_bytes = jpeg.len(),
        "Frame encoded"
    );

    Ok(Bytes::from(jpeg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_jpeg_gray() {
        let frame = DecodedFrame {
            stream_id: uuid::Uuid::new_v4(),
            pts: 0,
            time_base: (1, 30),
            width: 320,
            height: 240,
            y_plane: vec![128u8; 320 * 240],
            u_plane: vec![128u8; 320 * 240 / 4],
            v_plane: vec![128u8; 320 * 240 / 4],
            y_stride: 320,
            u_stride: 160,
            v_stride: 160,
            is_keyframe: true,
            frame_number: 0,
            scene_change_score: None,
        };
        let result = encode_jpeg(&frame, 85);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert_eq!(&bytes[..3], &[0xFF, 0xD8, 0xFF]);
        assert!(bytes.len() > 100);
    }

    #[test]
    fn test_encode_jpeg_minimal_size() {
        let frame = DecodedFrame {
            stream_id: uuid::Uuid::new_v4(),
            pts: 0,
            time_base: (1, 30),
            width: 16,
            height: 16,
            y_plane: vec![0u8; 16 * 16],
            u_plane: vec![128u8; 16 * 16 / 4],
            v_plane: vec![128u8; 16 * 16 / 4],
            y_stride: 16,
            u_stride: 8,
            v_stride: 8,
            is_keyframe: true,
            frame_number: 0,
            scene_change_score: None,
        };
        let result = encode_jpeg(&frame, 50);
        assert!(result.is_ok());
        let bytes = result.unwrap();
        assert_eq!(&bytes[..3], &[0xFF, 0xD8, 0xFF]);
    }

    #[test]
    fn test_encode_jpeg_varied_quality() {
        let frame = DecodedFrame {
            stream_id: uuid::Uuid::new_v4(),
            pts: 0,
            time_base: (1, 30),
            width: 64,
            height: 64,
            y_plane: vec![128u8; 64 * 64],
            u_plane: vec![128u8; 64 * 64 / 4],
            v_plane: vec![128u8; 64 * 64 / 4],
            y_stride: 64,
            u_stride: 32,
            v_stride: 32,
            is_keyframe: true,
            frame_number: 0,
            scene_change_score: None,
        };
        let low = encode_jpeg(&frame, 10).unwrap();
        let high = encode_jpeg(&frame, 95).unwrap();
        assert!(low.len() <= high.len(), "Higher quality should produce larger or equal output");
    }
}
