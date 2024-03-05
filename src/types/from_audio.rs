use super::{Audio, InputAudio, TypeError};

impl TryFrom<Audio> for compositor_pipeline::audio_mixer::types::AudioMixingParams {
    type Error = TypeError;

    fn try_from(value: Audio) -> Result<Self, Self::Error> {
        let mut inputs = Vec::with_capacity(value.inputs.len());
        for input in value.inputs {
            inputs.push(input.try_into()?);
        }

        Ok(Self { inputs })
    }
}

impl TryFrom<InputAudio> for compositor_pipeline::audio_mixer::types::InputParams {
    type Error = TypeError;

    fn try_from(value: InputAudio) -> Result<Self, Self::Error> {
        if let Some(volume) = value.volume {
            if !(0.0..=1.0).contains(&volume) {
                return Err(TypeError::new("Input volume has to be in [0, 1] range."));
            }
        }
        Ok(Self {
            input_id: value.input_id.into(),
            volume: value.volume.unwrap_or(1.0),
        })
    }
}