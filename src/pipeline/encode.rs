use crate::types::DecodedFrame;
use anyhow::Result;
use bytes::Bytes;
use std::time::Instant;

pub fn encode_jpeg(
    frame: &DecodedFrame,
    _quality: u8,
) -> Result<Bytes> {
    let timer = Instant::now();
    let width = frame.width as usize;
    let height = frame.height as usize;

    let rgb = yuv_to_rgb(
        &frame.y_plane,
        &frame.u_plane,
        &frame.v_plane,
        frame.y_stride as usize,
        frame.u_stride as usize,
        frame.v_stride as usize,
        width,
        height,
    )?;

    let yuv_to_rgb_us = timer.elapsed().as_micros();

    let encode_timer = Instant::now();
    let mut jpeg_bytes = Vec::with_capacity(width * height);

    yuvutils_rs::yuv420_to_rgb(
        &yuvutils_rs::YuvPlanarImage {
            y_plane: &rgb,
            y_stride: frame.y_stride as u32,
            u_plane: &frame.u_plane,
            u_stride: frame.u_stride as u32,
            v_plane: &frame.v_plane,
            v_stride: frame.v_stride as u32,
            width: width as u32,
            height: height as u32,
        },
        &mut jpeg_bytes,
        width as u32 * 3,
        yuvutils_rs::YuvRange::Limited,
        yuvutils_rs::YuvStandardMatrix::Bt601,
    )?;

    let encode_us = encode_timer.elapsed().as_micros();

    tracing::debug!(
        stream_id = %frame.stream_id,
        frame_number = frame.frame_number,
        yuv_to_rgb_us = yuv_to_rgb_us,
        jpeg_encode_us = encode_us,
        jpeg_size_bytes = jpeg_bytes.len(),
        "Frame encoded"
    );

    Ok(Bytes::from(jpeg_bytes))
}

fn yuv_to_rgb(
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    stride_y: usize,
    stride_u: usize,
    stride_v: usize,
    width: usize,
    height: usize,
) -> Result<Vec<u8>> {
    let mut rgb = vec![0u8; width * height * 3];
    let rgb_stride = width * 3;

    yuvutils_rs::yuv420_to_rgb(
        &yuvutils_rs::YuvPlanarImage {
            y_plane,
            y_stride: stride_y as u32,
            u_plane,
            u_stride: stride_u as u32,
            v_plane,
            v_stride: stride_v as u32,
            width: width as u32,
            height: height as u32,
        },
        &mut rgb,
        rgb_stride as u32,
        yuvutils_rs::YuvRange::Limited,
        yuvutils_rs::YuvStandardMatrix::Bt601,
    )?;

    Ok(rgb)
}
