use std::time::Duration;
use ffmpeg_next::{self as ffmpeg, format, media::Type, Rational, Dictionary};

const RTSP_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub struct DemuxedStream {
    pub ictx: format::context::Input,
    pub video_stream_index: usize,
    pub time_base: Rational,
    pub decoder: ffmpeg::codec::decoder::Video,
    pub width: u32,
    pub height: u32,
}

fn open_input_with_timeout(
    url: &str,
    rtsp_transport: &str,
    analyzeduration: &str,
    probesize: &str,
    stimeout: &str,
) -> Result<format::context::Input, anyhow::Error> {
    let (tx, rx) = std::sync::mpsc::channel();
    let url_owned = url.to_string();
    let rtsp_transport = rtsp_transport.to_string();
    let analyzeduration = analyzeduration.to_string();
    let probesize = probesize.to_string();
    let stimeout = stimeout.to_string();

    std::thread::spawn(move || {
        let mut opts = Dictionary::new();
        opts.set("rtsp_transport", &rtsp_transport);
        opts.set("analyzeduration", &analyzeduration);
        opts.set("probesize", &probesize);
        opts.set("stimeout", &stimeout);
        let result = format::input_with_dictionary(&url_owned, opts);
        let _ = tx.send(result);
    });

    match rx.recv_timeout(RTSP_CONNECT_TIMEOUT) {
        Ok(Ok(ictx)) => Ok(ictx),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err(anyhow::anyhow!(
            "RTSP connection to '{}' timed out after {}s",
            url, RTSP_CONNECT_TIMEOUT.as_secs()
        )),
    }
}

pub fn open_video_source(
    url: &str,
    source_type: &str,
    rtsp_transport: &str,
    ffmpeg_threads: i32,
) -> Result<DemuxedStream, anyhow::Error> {
    let ictx = if source_type == "rtsp" {
        open_input_with_timeout(url, rtsp_transport, "10000000", "5000000", "10000000")?
    } else {
        let mut opts = Dictionary::new();
        opts.set("analyzeduration", "10000000");
        opts.set("probesize", "5000000");
        opts.set("stimeout", "10000000");
        format::input_with_dictionary(url, opts)?
    };

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
    {
        let mut tc = ffmpeg::codec::threading::Config::kind(ffmpeg::codec::threading::Type::Frame);
        tc.count = ffmpeg_threads as usize;
        decoder_ctx.set_threading(tc);
    }

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
