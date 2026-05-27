use ffmpeg_next as ffmpeg;

pub struct SceneDetectFilter {
    graph: ffmpeg::filter::Graph,
}

impl SceneDetectFilter {
    pub fn new(
        width: u32,
        height: u32,
        pixel_format: ffmpeg::format::Pixel,
        time_base: ffmpeg::Rational,
        threshold: f64,
    ) -> Result<Self, ffmpeg::Error> {
        let threshold = threshold.clamp(0.001, 0.999);
        let mut graph = ffmpeg::filter::Graph::new();

        let pix_desc = pixel_format.descriptor();
        let pix_name = pix_desc.map(|d| d.name()).unwrap_or("yuv420p");
        let args = format!(
            "video_size={}x{}:pix_fmt={}:time_base={}/{}",
            width,
            height,
            pix_name,
            time_base.numerator(),
            time_base.denominator(),
        );

        let buffer_filter = ffmpeg::filter::find("buffer").expect("buffer filter not found");
        let scdet_filter = ffmpeg::filter::find("scdet").expect("scdet filter not found");
        let sink_filter = ffmpeg::filter::find("buffersink").expect("buffersink filter not found");

        let scdet_args = format!("threshold={:.6}", threshold);

        let mut buffer_ctx = graph.add(&buffer_filter, "in", &args)?;
        let mut scdet_ctx = graph.add(&scdet_filter, "scdet", &scdet_args)?;
        let mut sink_ctx = graph.add(&sink_filter, "out", "")?;

        buffer_ctx.link(0, &mut scdet_ctx, 0);
        scdet_ctx.link(0, &mut sink_ctx, 0);

        graph.validate()?;

        Ok(Self { graph })
    }

    pub fn filter(&mut self, frame: &ffmpeg::frame::Video) -> Result<f64, ffmpeg::Error> {
        let mut buffer_ctx = self.graph.get("in").unwrap();
        buffer_ctx.source().add(frame)?;

        let mut out = ffmpeg::frame::Video::empty();
        let mut sink_ctx = self.graph.get("out").unwrap();
        sink_ctx.sink().frame(&mut out)?;

        let score = read_scdet_score(&out);
        Ok(score)
    }
}

fn read_scdet_score(frame: &ffmpeg::frame::Video) -> f64 {
    let metadata = frame.metadata();
    metadata
        .get("lavfi.scd.score")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}
