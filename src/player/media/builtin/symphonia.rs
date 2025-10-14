use std::{ffi::OsStr, fs::File};

use intx::{I24, U24};
use regex::Regex;
use symphonia::{
    core::{
        audio::{AudioBufferRef, Channels, Signal},
        codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions},
        errors::Error,
        formats::{FormatOptions, FormatReader, SeekMode, SeekTo},
        io::MediaSourceStream,
        meta::{MetadataOptions, StandardTagKey, Tag, Value, Visual},
        probe::{Hint, ProbeResult},
        units::{Time, TimeBase},
    },
    default::get_codecs,
};

use crate::{
    player::devices::format::ChannelSpec,
    player::media::{
        errors::{
            ChannelRetrievalError, CloseError, FrameDurationError, MetadataError, OpenError,
            PlaybackReadError, PlaybackStartError, PlaybackStopError, SeekError,
            TrackDurationError,
        },
        metadata::Metadata,
        playback::{PlaybackFrame, Samples},
        traits::{MediaPlugin, MediaProvider},
    },
};

#[derive(Default)]
pub struct SymphoniaProvider {
    format: Option<Box<dyn FormatReader>>,
    current_metadata: Metadata,
    current_track: u32,
    current_duration: u64,
    current_length: Option<u64>,
    current_position: u64,
    current_timebase: Option<TimeBase>,
    decoder: Option<Box<dyn Decoder>>,
    pending_metadata_update: bool,
    last_image: Option<Visual>,
}

impl SymphoniaProvider {
    fn break_metadata(&mut self, tags: &[Tag]) {
        let id3_position_in_set_regex = Regex::new(r"(\d+)/(\d+)").unwrap();

        for tag in tags {
            match tag.std_key {
                Some(StandardTagKey::TrackTitle) => {
                    self.current_metadata.name = Some(tag.value.to_string())
                }
                Some(StandardTagKey::Artist) => {
                    self.current_metadata.artist = Some(tag.value.to_string())
                }
                Some(StandardTagKey::AlbumArtist) => {
                    self.current_metadata.album_artist = Some(tag.value.to_string())
                }
                Some(StandardTagKey::OriginalArtist) => {
                    self.current_metadata.original_artist = Some(tag.value.to_string())
                }
                Some(StandardTagKey::Composer) => {
                    self.current_metadata.composer = Some(tag.value.to_string())
                }
                Some(StandardTagKey::Album) => {
                    self.current_metadata.album = Some(tag.value.to_string())
                }
                Some(StandardTagKey::Genre) => {
                    self.current_metadata.genre = Some(tag.value.to_string())
                }
                Some(StandardTagKey::ContentGroup) => {
                    self.current_metadata.grouping = Some(tag.value.to_string())
                }
                Some(StandardTagKey::Bpm) => {
                    self.current_metadata.bpm = match &tag.value {
                        Value::String(v) => v.clone().parse().ok(),
                        Value::UnsignedInt(v) => Some(*v),
                        _ => None,
                    }
                }
                Some(StandardTagKey::Compilation) => {
                    self.current_metadata.compilation = match tag.value {
                        Value::Boolean(v) => v,
                        Value::Flag => true,
                        _ => false,
                    }
                }
                Some(StandardTagKey::Date) => {
                    self.current_metadata.date =
                        Some(dateparser::parse(&tag.value.to_string()).ok()).flatten();
                }
                Some(StandardTagKey::TrackNumber) => match &tag.value {
                    Value::String(v) => {
                        if let Some(captures) = id3_position_in_set_regex.captures(v) {
                            if let Some(track) = captures.get(1) {
                                self.current_metadata.track_current = track.as_str().parse().ok();
                            }
                            if let Some(total) = captures.get(2) {
                                self.current_metadata.track_max = total.as_str().parse().ok();
                            }
                        } else {
                            self.current_metadata.track_current = v.clone().parse().ok();
                        }
                    }
                    Value::UnsignedInt(v) => {
                        self.current_metadata.track_current = Some(*v);
                    }
                    _ => (),
                },
                Some(StandardTagKey::TrackTotal) => {
                    self.current_metadata.track_max = match &tag.value {
                        Value::String(v) => v.clone().parse().ok(),
                        Value::UnsignedInt(v) => Some(*v),
                        _ => None,
                    }
                }
                Some(StandardTagKey::DiscNumber) => match &tag.value {
                    Value::String(v) => {
                        if let Some(captures) = id3_position_in_set_regex.captures(v) {
                            if let Some(disc) = captures.get(1) {
                                self.current_metadata.disc_current = disc.as_str().parse().ok();
                            }
                            if let Some(total) = captures.get(2) {
                                self.current_metadata.disc_max = total.as_str().parse().ok();
                            }
                        } else {
                            self.current_metadata.disc_current = v.clone().parse().ok();
                        }
                    }
                    Value::UnsignedInt(v) => {
                        self.current_metadata.disc_current = Some(*v);
                    }
                    _ => (),
                },
                Some(StandardTagKey::DiscTotal) => {
                    self.current_metadata.disc_max = match &tag.value {
                        Value::String(v) => v.clone().parse().ok(),
                        Value::UnsignedInt(v) => Some(*v),
                        _ => None,
                    }
                }
                Some(StandardTagKey::Label) => {
                    self.current_metadata.label = Some(tag.value.to_string())
                }
                Some(StandardTagKey::IdentCatalogNumber) => {
                    self.current_metadata.catalog = Some(tag.value.to_string())
                }
                Some(StandardTagKey::IdentIsrc) => {
                    self.current_metadata.isrc = Some(tag.value.to_string())
                }
                Some(StandardTagKey::SortAlbum) => {
                    self.current_metadata.sort_album = Some(tag.value.to_string())
                }
                Some(StandardTagKey::SortAlbumArtist) => {
                    self.current_metadata.artist_sort = Some(tag.value.to_string())
                }
                Some(StandardTagKey::MusicBrainzAlbumId) => {
                    self.current_metadata.mbid_album = Some(tag.value.to_string())
                }
                _ => (),
            }
        }
    }

    fn read_base_metadata(&mut self, probed: &mut ProbeResult) {
        self.current_metadata = Metadata::default();
        self.last_image = None;

        if let Some(metadata) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
            self.break_metadata(metadata.tags());
            if !metadata.visuals().is_empty() {
                self.last_image = Some(metadata.visuals()[0].clone());
            }
        }

        if let Some(metadata) = probed.format.metadata().current() {
            self.break_metadata(metadata.tags());
            if !metadata.visuals().is_empty() {
                self.last_image = Some(metadata.visuals()[0].clone());
            }
        }

        self.pending_metadata_update = true;
    }
}

impl MediaProvider for SymphoniaProvider {
    fn open(&mut self, file: File, ext: Option<&OsStr>) -> Result<(), OpenError> {
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let ext_as_str = ext.and_then(|e| e.to_str());
        let mut probed = if let Some(ext) = ext_as_str {
            let mut hint = Hint::new();
            hint.with_extension(ext);

            symphonia::default::get_probe()
                .format(&hint, mss, &fmt_opts, &meta_opts)
                .map_err(|_| OpenError::UnsupportedFormat)?
        } else {
            let hint = Hint::new();

            symphonia::default::get_probe()
                .format(&hint, mss, &fmt_opts, &meta_opts)
                .map_err(|_| OpenError::UnsupportedFormat)?
        };

        self.read_base_metadata(&mut probed);
        self.current_position = 0;
        self.current_length = None;
        self.current_timebase = None;
        self.format = Some(probed.format);

        Ok(())
    }

    fn close(&mut self) -> Result<(), CloseError> {
        self.stop_playback().expect("invalid outcome");
        self.current_metadata = Metadata::default();
        self.format = None;
        Ok(())
    }

    fn start_playback(&mut self) -> Result<(), PlaybackStartError> {
        let Some(format) = &self.format else {
            return Err(PlaybackStartError::NothingOpen);
        };
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(PlaybackStartError::NothingToPlay)?;

        if let Some(frame_count) = track.codec_params.n_frames {
            if let Some(tb) = track.codec_params.time_base {
                self.current_length = Some(tb.calc_time(frame_count).seconds);
                self.current_timebase = Some(tb);
            }
        }

        self.current_track = track.id;

        let dec_opts: DecoderOptions = Default::default();
        self.decoder = Some(
            get_codecs()
                .make(&track.codec_params, &dec_opts)
                .map_err(|_| PlaybackStartError::Undecodable)?,
        );

        Ok(())
    }

    fn stop_playback(&mut self) -> Result<(), PlaybackStopError> {
        self.current_track = 0;
        self.decoder = None;

        Ok(())
    }

    fn read_samples(&mut self) -> Result<PlaybackFrame, PlaybackReadError> {
        let Some(format) = &mut self.format else {
            return Err(PlaybackReadError::NothingOpen);
        };
        // this has a loop because the next packet may not be from the current track
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(Error::ResetRequired) => return Err(PlaybackReadError::Eof),
                Err(_) => {
                    // TODO: Handle better
                    return Err(PlaybackReadError::Eof);
                }
            };

            while !format.metadata().is_latest() {
                // TODO: handle metadata updates
                format.metadata().pop();
            }

            if packet.track_id() != self.current_track {
                continue;
            }

            let Some(decoder) = &mut self.decoder else {
                return Err(PlaybackReadError::NeverStarted);
            };

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let rate = decoded.spec().rate;
                    let channel_count = decoded.spec().channels.count();
                    self.current_duration = decoded.capacity() as u64;

                    if let Some(tb) = &self.current_timebase {
                        self.current_position = tb.calc_time(packet.ts()).seconds;
                    }

                    match decoded {
                        AudioBufferRef::U8(v) => {
                            let mut samples: Vec<Vec<u8>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(*sample);
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Unsigned8(samples),
                            });
                        }
                        AudioBufferRef::U16(v) => {
                            let mut samples: Vec<Vec<u16>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(*sample);
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Unsigned16(samples),
                            });
                        }
                        AudioBufferRef::U24(v) => {
                            let mut samples: Vec<Vec<U24>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(
                                        U24::try_from(sample.0)
                                            .expect("24bit number is not 24bits long"),
                                    );
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Unsigned24(samples),
                            });
                        }
                        AudioBufferRef::U32(v) => {
                            let mut samples: Vec<Vec<u32>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(*sample);
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Unsigned32(samples),
                            });
                        }
                        AudioBufferRef::S8(v) => {
                            let mut samples: Vec<Vec<i8>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(*sample);
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Signed8(samples),
                            });
                        }
                        AudioBufferRef::S16(v) => {
                            let mut samples: Vec<Vec<i16>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(*sample);
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Signed16(samples),
                            });
                        }
                        AudioBufferRef::S24(v) => {
                            let mut samples: Vec<Vec<I24>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(
                                        I24::try_from(sample.0)
                                            .expect("24bit number is not 24bits long"),
                                    );
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Signed24(samples),
                            });
                        }
                        AudioBufferRef::S32(v) => {
                            let mut samples: Vec<Vec<i32>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(*sample);
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Signed32(samples),
                            });
                        }
                        AudioBufferRef::F32(v) => {
                            let mut samples: Vec<Vec<f32>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(*sample);
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Float32(samples),
                            });
                        }
                        AudioBufferRef::F64(v) => {
                            let mut samples: Vec<Vec<f64>> =
                                Vec::with_capacity(v.spec().channels.count());

                            for i in 0..channel_count {
                                samples.push(Vec::with_capacity(v.capacity()));
                                for sample in v.chan(i) {
                                    samples[i].push(*sample);
                                }
                            }

                            return Ok(PlaybackFrame {
                                rate,
                                samples: Samples::Float64(samples),
                            });
                        }
                    }
                }
                Err(Error::IoError(_)) | Err(Error::DecodeError(_)) => {
                    continue;
                }
                Err(e) => {
                    return Err(PlaybackReadError::DecodeFatal(e.to_string()));
                }
            }
        }
    }

    fn frame_duration(&self) -> Result<u64, FrameDurationError> {
        if self.decoder.is_none() {
            Err(FrameDurationError::NothingOpen)
        } else if self.current_duration == 0 {
            Err(FrameDurationError::NeverDecoded)
        } else {
            Ok(self.current_duration)
        }
    }

    fn read_metadata(&mut self) -> Result<&Metadata, MetadataError> {
        self.pending_metadata_update = false;

        if self.format.is_some() {
            Ok(&self.current_metadata)
        } else {
            Err(MetadataError::NothingOpen)
        }
    }

    fn metadata_updated(&self) -> bool {
        self.pending_metadata_update
    }

    fn read_image(&mut self) -> Result<Option<Box<[u8]>>, MetadataError> {
        if self.format.is_some() {
            if let Some(visual) = &self.last_image {
                let data = Ok(Some(visual.data.clone()));
                self.last_image = None;
                data
            } else {
                Ok(None)
            }
        } else {
            Err(MetadataError::NothingOpen)
        }
    }

    fn duration_secs(&self) -> Result<u64, TrackDurationError> {
        if self.decoder.is_none() {
            Err(TrackDurationError::NothingOpen)
        } else if self.current_length.is_none() {
            Err(TrackDurationError::NeverStarted)
        } else {
            Ok(self.current_length.unwrap_or_default())
        }
    }

    fn position_secs(&self) -> Result<u64, TrackDurationError> {
        if self.decoder.is_none() {
            Err(TrackDurationError::NothingOpen)
        } else if self.current_length.is_none() {
            Err(TrackDurationError::NeverStarted)
        } else {
            Ok(self.current_position)
        }
    }

    fn seek(&mut self, time: f64) -> Result<(), SeekError> {
        let timebase = self.current_timebase;
        let Some(format) = &mut self.format else {
            return Err(SeekError::NothingOpen);
        };
        let seek = format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time: Time {
                        seconds: time.trunc() as u64,
                        frac: time.fract(),
                    },
                    track_id: None,
                },
            )
            .map_err(|e| SeekError::Unknown(e.to_string()))?;

        if let Some(timebase) = timebase {
            self.current_position = timebase.calc_time(seek.actual_ts).seconds;
        }

        Ok(())
    }

    fn channels(&self) -> Result<ChannelSpec, ChannelRetrievalError> {
        let Some(format) = &self.format else {
            return Err(ChannelRetrievalError::NothingOpen);
        };

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or(ChannelRetrievalError::NothingToPlay)?;

        // HACK: if the channel count isn't in the codec parameters pretend that it's stereo
        // this "fixes" m4a container files but obviously poorly
        //
        // upstream issue: https://github.com/pdeljanov/Symphonia/issues/289
        Ok(ChannelSpec::Count(
            track
                .codec_params
                .channels
                .map(Channels::count)
                .unwrap_or(2) as u16,
        ))
    }
}

impl MediaPlugin for SymphoniaProvider {
    const NAME: &'static str = "Symphonia";

    const VERSION: &'static str = "0.1.0";

    const SUPPORTED_MIMETYPES: &'static [&'static str] = &[
        "audio/ogg",
        "audio/aac",
        "audio/x-flac",
        "audio/x-wav",
        "audio/mpeg",
        "audio/m4a",
        "audio/x-aiff",
    ];

    const PROVIDES_DECODING: bool = true;
    const PROVIDES_METADATA: bool = true;
    const ALWAYS_CHECK_METADATA: bool = false;

    const SUPPORTED_EXTENSIONS: &'static [&'static str] =
        &["ogg", "aac", "flac", "wav", "mp3", "m4a", "aiff"];
    const INDEXING_SUPPORTED: bool = true;
}
