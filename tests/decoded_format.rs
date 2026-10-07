//! The registry decoder reports the size and pixel layout of the image it
//! last returned (oxideav-core `Decoder::output_video_dimensions` /
//! `output_pixel_format`): an odd-width palette picture, then a 24-bit
//! picture of another size. Each packet is a whole `FORM`, so the second
//! packet is a size change.
//!
//! The pictures come from the crate's own registry encoder.

#![cfg(feature = "registry")]

use oxideav_core::{
    CodecId, CodecParameters, Decoder, Frame, Packet, PixelFormat, TimeBase, VideoFrame, VideoPlane,
};

/// Encodes one `w`×`h` picture of `format` (`Pal8` or `Rgb24`) to a
/// `FORM ILBM`.
fn ilbm(w: u32, h: u32, format: PixelFormat) -> Vec<u8> {
    let bytes = if format == PixelFormat::Pal8 { 1 } else { 3 };
    let stride = w as usize * bytes;
    let mut frame = VideoFrame {
        pts: Some(0),
        planes: vec![VideoPlane {
            stride,
            data: (0..stride * h as usize).map(|i| (i % 7) as u8).collect(),
        }],
    };
    if format == PixelFormat::Pal8 {
        frame.set_palette((0..8 * 3).map(|i| (i * 31) as u8).collect());
    }
    let mut params = CodecParameters::video(CodecId::new("ilbm"));
    params.width = Some(w);
    params.height = Some(h);
    params.pixel_format = Some(format);
    let mut enc = oxideav_iff::registry::make_encoder(&params).expect("encoder");
    enc.send_frame(&Frame::Video(frame)).expect("encode");
    enc.receive_packet().expect("packet").data
}

fn report(dec: &dyn Decoder) -> Option<(u32, u32, PixelFormat)> {
    let (w, h) = dec.output_video_dimensions()?;
    Some((w, h, dec.output_pixel_format()?))
}

#[test]
fn each_frame_reports_its_own_size_and_layout() {
    let pictures = [(33, 17, PixelFormat::Pal8), (34, 18, PixelFormat::Rgb24)];
    let mut dec =
        oxideav_iff::registry::make_decoder(&CodecParameters::video(CodecId::new("ilbm")))
            .expect("decoder");
    let mut last = None;
    for (at, (w, h, format)) in pictures.into_iter().enumerate() {
        dec.send_packet(&Packet::new(0, TimeBase::new(1, 1), ilbm(w, h, format)))
            .expect("send");
        // A new picture changes nothing until it is returned; before the
        // first frame, the pending picture.
        let before = last.or(Some((w, h, format)));
        assert_eq!(report(&*dec), before, "picture {at}: before it is returned");
        let frame = match dec.receive_frame() {
            Ok(Frame::Video(frame)) => frame,
            other => panic!("picture {at}: {other:?}"),
        };
        assert_eq!(report(&*dec), Some((w, h, format)), "picture {at}");
        let bytes = if format == PixelFormat::Pal8 { 1 } else { 3 };
        let plane = &frame.image_planes()[0];
        assert!(plane.stride >= w as usize * bytes, "picture {at}: stride");
        assert_eq!(
            plane.data.len(),
            plane.stride * h as usize,
            "picture {at}: rows"
        );
        last = Some((w, h, format));
    }
}
