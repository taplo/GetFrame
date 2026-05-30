use ffmpeg_next::{self as ffmpeg, format, media::Type, Rational, Dictionary};

pub struct DemuxedStream {
    pub ictx: format::context::Input,
    pub video_stream_index: usize,
    pub time_base: Rational,
    pub decoder: ffmpeg::codec::decoder::Video,
    pub width: u32,
    pub height: u32,
}

pub fn open_video_source(
    url: &str,
    source_type: &str,
    rtsp_transport: &str,
    ffmpeg_threads: i32,
) -> Result<DemuxedStream, anyhow::Error> {
    let mut opts = Dictionary::new();
    if source_type == "rtsp" {
        opts.set("rtsp_transport", rtsp_transport);
    }
    opts.set("analyzeduration", "5000000");
    opts.set("probesize", "5000000");

    let ictx = format::input_with_dictionary(url, opts)?;

    let video_stream = ictx.streams()
        .best(Type::Video)
        .ok_or_else(|| anyhow::anyhow!("No video stream found in source: {}", url))?;

    let video_stream_index = video_stream.index();
    let time_base = video_stream.time_base();
    let params = video_stream.parameters();
    let width = unsafe { (*params.as_ptr()).width as u32 };
    let height = unsafe { (*params.as_ptr()).height as u32 };
    let codec_id = params.id();

    anyhow::ensure!(width > 0 && height > 0, "Invalid video resolution: {}x{}", width, height);
    anyhow::ensure!(codec_id != ffmpeg::codec::Id::None, "Unknown codec in video stream");

    let mut decoder_ctx = ffmpeg::codec::Context::from_parameters(video_stream.parameters())?;
    #[allow(clippy::struct_update_has_no_effect)]
    decoder_ctx.set_threading(ffmpeg::codec::threading::Config {
        kind: ffmpeg::codec::threading::Type::Frame,
        count: ffmpeg_threads as usize,
        ..Default::default()
    });

    let codec = ffmpeg::codec::decoder::find(codec_id)
        .ok_or_else(|| anyhow::anyhow!("No decoder found for codec: {:?}", codec_id))?;

    let decoder = decoder_ctx.decoder().open_as(codec)?.video()?;

    Ok(DemuxedStream {
        ictx,
        video_stream_index,
        time_base,
        decoder,
        width,
        height,
    })
}
