use crate::{Audio, AudioSource, Decodable};
use bevy_asset::{Asset, Assets};
use bevy_ecs::world::World;
use bevy_reflect::TypeUuid;
use bevy_utils::tracing::warn;
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};
use std::marker::PhantomData;

/// Used internally to play audio on the current "audio device"
pub struct AudioOutput<Source = AudioSource>
where
    Source: Decodable,
{
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    phantom: PhantomData<Source>,
}

impl<Source> Default for AudioOutput<Source>
where
    Source: Decodable,
{
    fn default() -> Self {
        if let Ok((stream, stream_handle)) = OutputStream::try_default() {
            Self {
                _stream: Some(stream),
                stream_handle: Some(stream_handle),
                phantom: PhantomData,
            }
        } else {
            warn!("No audio device found.");
            Self {
                _stream: None,
                stream_handle: None,
                phantom: PhantomData,
            }
        }
    }
}

impl<Source> AudioOutput<Source>
where
    Source: Asset + Decodable,
{
    fn play_source(&self, audio_source: &Source, repeat: bool) -> Option<Sink> {
        if let Some(stream_handle) = &self.stream_handle {
            let sink = Sink::try_new(stream_handle).unwrap();
            if repeat {
                sink.append(audio_source.decoder().repeat_infinite());
            } else {
                sink.append(audio_source.decoder());
            }
            Some(sink)
        } else {
            None
        }
    }

    fn try_play_queued(
        &self,
        audio_sources: &Assets<Source>,
        audio: &mut Audio<Source>,
        sinks: &mut Assets<AudioSink>,
    ) {
        let mut queue = audio.queue.write();
        let len = queue.len();
        let mut i = 0;
        while i < len {
            let config = queue.pop_front().unwrap();
            if let Some(audio_source) = audio_sources.get(&config.source_handle) {
                if let Some(sink) = self.play_source(audio_source, config.repeat) {
                    // don't keep the strong handle. there is no way to return it to the user here as it is async
                    let _ = sinks.set(config.sink_handle, AudioSink { sink: Some(sink) });
                }
            } else {
                // audio source hasn't loaded yet. add it back to the queue
                queue.push_back(config);
            }
            i += 1;
        }
    }
}

/// Plays audio currently queued in the [`Audio`] resource through the [`AudioOutput`] resource
pub fn play_queued_audio_system<Source: Asset>(world: &mut World)
where
    Source: Decodable,
{
    let world = world.cell();
    let audio_output = world.get_non_send::<AudioOutput<Source>>().unwrap();
    let mut audio = world.get_resource_mut::<Audio<Source>>().unwrap();
    let mut sinks = world.get_resource_mut::<Assets<AudioSink>>().unwrap();

    if let Some(audio_sources) = world.get_resource::<Assets<Source>>() {
        audio_output.try_play_queued(&*audio_sources, &mut *audio, &mut *sinks);
    };
}

/// Asset controlling the playback of a sound
///
/// ```
/// # use bevy_ecs::system::{Local, Res};
/// # use bevy_asset::{Assets, Handle};
/// # use bevy_audio::AudioSink;
/// // Execution of this system should be controlled by a state or input,
/// // otherwise it would just toggle between play and pause every frame.
/// fn pause(
///     audio_sinks: Res<Assets<AudioSink>>,
///     music_controller: Local<Handle<AudioSink>>,
/// ) {
///     if let Some(sink) = audio_sinks.get(&*music_controller) {
///         if sink.is_paused() {
///             sink.play()
///         } else {
///             sink.pause()
///         }
///     }
/// }
/// ```
///
#[derive(TypeUuid)]
#[uuid = "8BEE570C-57C2-4FC0-8CFB-983A22F7D981"]
pub struct AudioSink {
    // This field is an Option in order to allow us to have a safe drop that will detach the sink.
    // It will never be None during its life
    sink: Option<Sink>,
}

impl Drop for AudioSink {
    fn drop(&mut self) {
        self.sink.take().unwrap().detach();
    }
}

impl AudioSink {
    /// Gets the volume of the sound.
    ///
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0`
    /// will multiply each sample by this value.
    pub fn volume(&self) -> f32 {
        self.sink.as_ref().unwrap().volume()
    }

    /// Changes the volume of the sound.
    ///
    /// The value `1.0` is the "normal" volume (unfiltered input). Any value other than `1.0`
    /// will multiply each sample by this value.
    pub fn set_volume(&self, volume: f32) {
        self.sink.as_ref().unwrap().set_volume(volume);
    }

    /// Gets the speed of the sound.
    ///
    /// The value `1.0` is the "normal" speed (unfiltered input). Any value other than `1.0`
    /// will change the play speed of the sound.
    pub fn speed(&self) -> f32 {
        self.sink.as_ref().unwrap().speed()
    }

    /// Changes the speed of the sound.
    ///
    /// The value `1.0` is the "normal" speed (unfiltered input). Any value other than `1.0`
    /// will change the play speed of the sound.
    pub fn set_speed(&self, speed: f32) {
        self.sink.as_ref().unwrap().set_speed(speed);
    }

    /// Resumes playback of a paused sink.
    ///
    /// No effect if not paused.
    pub fn play(&self) {
        self.sink.as_ref().unwrap().play();
    }

    /// Pauses playback of this sink.
    ///
    /// No effect if already paused.
    /// A paused sink can be resumed with [`play`](Self::play).
    pub fn pause(&self) {
        self.sink.as_ref().unwrap().pause();
    }

    /// Is this sink paused?
    ///
    /// Sinks can be paused and resumed using [`pause`](Self::pause) and [`play`](Self::play).
    pub fn is_paused(&self) -> bool {
        self.sink.as_ref().unwrap().is_paused()
    }
}
